//! nova-link: Object file linker for the MacroCore-X toolchain.
//!
//! Handles:
//! - Symbol resolution across multiple object files
//! - Section merging (.text, .data, .bss, .rodata)
//! - Output formats: ELF (PC/Workstation), flat binary (MCU)
//! - Relocation processing
//!
//! # Object File Format
//!
//! Each object file is a simple binary with:
//! ```text
//! [magic: 4 bytes "NOVA"]
//! [header: 32 bytes]
//! [section table]
//! [section data...]
//! [symbol table]
//! [relocation table]
//! ```

use std::collections::HashMap;

/// Magic bytes for Nova object files.
pub const OBJ_MAGIC: &[u8; 4] = b"NOVA";

/// Object file header.
#[derive(Debug, Clone)]
pub struct ObjectHeader {
    /// Number of sections.
    pub num_sections: u32,
    /// Number of symbols.
    pub num_symbols: u32,
    /// Number of relocations.
    pub num_relocations: u32,
    /// Target triple length.
    pub target_len: u32,
    /// Reserved.
    pub reserved: [u32; 4],
}

/// Section in an object file.
#[derive(Debug, Clone)]
pub struct Section {
    /// Section name.
    pub name: String,
    /// Section data.
    pub data: Vec<u8>,
    /// Section flags (1=alloc, 2=exec, 4=write).
    pub flags: u32,
    /// Alignment requirement.
    pub alignment: u32,
}

/// Symbol entry.
#[derive(Debug, Clone)]
pub struct Symbol {
    /// Symbol name.
    pub name: String,
    /// Section index (0 = undefined).
    pub section_index: u32,
    /// Offset within section.
    pub offset: u32,
    /// Size of the symbol.
    pub size: u32,
    /// Symbol type (0=notype, 1=func, 2=object).
    pub sym_type: u8,
    /// Binding (0=local, 1=global, 2=weak).
    pub binding: u8,
}

/// Relocation entry.
#[derive(Debug, Clone)]
pub struct Relocation {
    /// Offset within the section to patch.
    pub offset: u32,
    /// Symbol index to resolve.
    pub symbol_index: u32,
    /// Relocation type.
    pub reloc_type: RelocType,
    /// Addend.
    pub addend: i64,
}

/// Relocation types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocType {
    /// 32-bit absolute address.
    Abs32,
    /// 64-bit absolute address.
    Abs64,
    /// PC-relative 12-bit branch.
    PcRel12,
    /// PC-relative 20-bit jump.
    PcRel20,
    /// 32-bit relative.
    Rel32,
    /// 64-bit relative.
    Rel64,
}

/// An object file in memory.
#[derive(Debug, Clone)]
pub struct ObjectFile {
    /// Source file name.
    pub name: String,
    /// Target triple.
    pub target: String,
    /// Sections.
    pub sections: Vec<Section>,
    /// Symbols.
    pub symbols: Vec<Symbol>,
    /// Relocations.
    pub relocations: Vec<Relocation>,
}

/// Output format for the linker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Flat binary (for MCU).
    Binary,
    /// ELF executable (for PC/Workstation).
    Elf,
}

/// Linker configuration.
#[derive(Debug, Clone)]
pub struct LinkConfig {
    /// Output format.
    pub format: OutputFormat,
    /// Base address for code section.
    pub text_base: u64,
    /// Base address for data section.
    pub data_base: u64,
    /// Entry point symbol name.
    pub entry: String,
    /// Output file name.
    pub output: String,
}

impl Default for LinkConfig {
    fn default() -> Self {
        LinkConfig {
            format: OutputFormat::Binary,
            text_base: 0x1000,
            data_base: 0x10000,
            entry: "_start".to_string(),
            output: "a.out".to_string(),
        }
    }
}

/// The linker.
pub struct Linker {
    /// Object files to link.
    objects: Vec<ObjectFile>,
    /// Configuration.
    config: LinkConfig,
    /// Merged sections.
    merged_sections: HashMap<String, Vec<u8>>,
    /// Resolved symbol addresses.
    symbol_addresses: HashMap<String, u64>,
    /// Base address of each section.
    section_base: HashMap<String, u64>,
}

impl Linker {
    /// Create a new linker with the given configuration.
    pub fn new(config: LinkConfig) -> Self {
        Linker {
            objects: Vec::new(),
            config,
            merged_sections: HashMap::new(),
            symbol_addresses: HashMap::new(),
            section_base: HashMap::new(),
        }
    }

    /// Add an object file to the linker.
    pub fn add_object(&mut self, obj: ObjectFile) {
        self.objects.push(obj);
    }

    /// Link all object files and produce the output binary.
    pub fn link(&mut self) -> Result<Vec<u8>, LinkError> {
        if self.objects.is_empty() {
            return Err(LinkError::NoObjects);
        }

        // Phase 1: Merge sections
        self.merge_sections()?;

        // Phase 2: Resolve symbols
        self.resolve_symbols()?;

        // Phase 3: Apply relocations
        self.apply_relocations()?;

        // Phase 4: Generate output
        match self.config.format {
            OutputFormat::Binary => self.generate_binary(),
            OutputFormat::Elf => self.generate_elf(),
        }
    }

    /// Link and write the output to a file.
    pub fn link_to_file(&mut self, path: &str) -> Result<(), LinkError> {
        let output = self.link()?;
        std::fs::write(path, &output).map_err(|e| LinkError::Io(e.to_string()))?;
        Ok(())
    }

    // ── Phase 1: Merge sections ──────────────────────────────────────────

    fn merge_sections(&mut self) -> Result<(), LinkError> {
        self.merged_sections.clear();
        self.section_base.clear();

        // Standard section order
        let section_order = [".text", ".rodata", ".data", ".bss"];

        // Current offset within each section
        let mut section_offsets: HashMap<String, usize> = HashMap::new();

        for obj in &self.objects {
            for section in &obj.sections {
                let entry = self.merged_sections.entry(section.name.clone()).or_default();
                let offset = section_offsets.entry(section.name.clone()).or_default();

                // Align
                let align = section.alignment as usize;
                if align > 1 {
                    let padding = (align - (entry.len() % align)) % align;
                    entry.extend(std::iter::repeat(0u8).take(padding));
                    *offset += padding;
                }

                entry.extend(&section.data);
                *offset += section.data.len();
            }
        }

        // Assign base addresses to sections
        let mut current_addr = self.config.text_base;
        for sec_name in &section_order {
            if let Some(data) = self.merged_sections.get(*sec_name) {
                if !data.is_empty() {
                    self.section_base.insert(sec_name.to_string(), current_addr);
                    current_addr += data.len() as u64;
                    // Page-align
                    current_addr = (current_addr + 0xFFF) & !0xFFF;
                }
            }
        }

        Ok(())
    }

    // ── Phase 2: Resolve symbols ─────────────────────────────────────────

    fn resolve_symbols(&mut self) -> Result<(), LinkError> {
        self.symbol_addresses.clear();

        for obj in &self.objects {
            for sym in &obj.symbols {
                // Skip undefined symbols
                if sym.section_index == 0 {
                    continue;
                }

                // Skip local symbols that are not global
                if sym.binding == 0 {
                    continue;
                }

                let section = obj.sections.get(sym.section_index as usize - 1)
                    .ok_or_else(|| LinkError::InvalidSectionIndex {
                        name: sym.name.clone(),
                        index: sym.section_index,
                    })?;

                let section_base = self.section_base.get(&section.name)
                    .copied()
                    .unwrap_or(0);

                let addr = section_base + sym.offset as u64;

                if sym.binding == 1 {
                    // Global: check for duplicates
                    if let Some(existing) = self.symbol_addresses.get(&sym.name) {
                        if *existing != addr {
                            return Err(LinkError::DuplicateSymbol {
                                name: sym.name.clone(),
                                addr1: *existing,
                                addr2: addr,
                            });
                        }
                    }
                    self.symbol_addresses.insert(sym.name.clone(), addr);
                }
            }
        }

        // Verify entry point exists
        if !self.symbol_addresses.contains_key(&self.config.entry) {
            return Err(LinkError::UndefinedSymbol {
                name: self.config.entry.clone(),
            });
        }

        Ok(())
    }

    // ── Phase 3: Apply relocations ───────────────────────────────────────

    fn apply_relocations(&mut self) -> Result<(), LinkError> {
        for obj in &self.objects {
            for reloc in &obj.relocations {
                let sym = obj.symbols.get(reloc.symbol_index as usize)
                    .ok_or_else(|| LinkError::InvalidSymbolIndex {
                        index: reloc.symbol_index,
                    })?;

                let target_addr = if sym.section_index == 0 {
                    // Undefined symbol: look up in resolved addresses
                    *self.symbol_addresses.get(&sym.name)
                        .ok_or_else(|| LinkError::UndefinedSymbol {
                            name: sym.name.clone(),
                        })?
                } else {
                    let section = obj.sections.get(sym.section_index as usize - 1)
                        .ok_or_else(|| LinkError::InvalidSectionIndex {
                            name: sym.name.clone(),
                            index: sym.section_index,
                        })?;
                    let section_base = self.section_base.get(&section.name).copied().unwrap_or(0);
                    section_base + sym.offset as u64
                };

                // Find the section containing this relocation
                // The relocation offset is relative to the section start
                let target_section = &obj.sections[0]; // Default to .text
                let section_data = self.merged_sections.get_mut(&target_section.name)
                    .ok_or_else(|| LinkError::MissingSection {
                        name: target_section.name.clone(),
                    })?;

                let patch_offset = reloc.offset as usize;
                if patch_offset + 8 > section_data.len() {
                    return Err(LinkError::RelocationOutOfBounds {
                        offset: patch_offset,
                        section_size: section_data.len(),
                    });
                }

                match reloc.reloc_type {
                    RelocType::Abs32 => {
                        let val = (target_addr as u32).wrapping_add(reloc.addend as u32);
                        section_data[patch_offset..patch_offset + 4]
                            .copy_from_slice(&val.to_le_bytes());
                    }
                    RelocType::Abs64 => {
                        let val = target_addr.wrapping_add(reloc.addend as u64);
                        section_data[patch_offset..patch_offset + 8]
                            .copy_from_slice(&val.to_le_bytes());
                    }
                    RelocType::PcRel12 => {
                        let pc = patch_offset as u64;
                        let offset = target_addr.wrapping_sub(pc) as i64;
                        let imm12 = offset >> 2;
                        let imm12 = ((imm12 as u16) & 0xFFF) as u16;
                        // Patch bytes 2-3 of the branch instruction
                        if patch_offset + 4 <= section_data.len() {
                            section_data[patch_offset + 2] = ((imm12 >> 8) & 0xFF) as u8;
                            section_data[patch_offset + 3] = (imm12 & 0xFF) as u8;
                        }
                    }
                    RelocType::PcRel20 => {
                        let pc = patch_offset as u64;
                        let offset = target_addr.wrapping_sub(pc) as i64;
                        let imm20 = offset >> 2;
                        let imm20 = ((imm20 as u32) & 0xFFFFF) as u32;
                        // Patch bytes 1-3 of the jump instruction
                        if patch_offset + 4 <= section_data.len() {
                            section_data[patch_offset + 1] = ((imm20 >> 12) & 0xFF) as u8;
                            section_data[patch_offset + 2] = (imm20 & 0xFF) as u8;
                            section_data[patch_offset + 3] = ((imm20 >> 8) & 0xF) as u8;
                        }
                    }
                    RelocType::Rel32 => {
                        let pc = patch_offset as u64;
                        let offset = target_addr.wrapping_sub(pc).wrapping_add(reloc.addend as u64);
                        section_data[patch_offset..patch_offset + 4]
                            .copy_from_slice(&(offset as u32).to_le_bytes());
                    }
                    RelocType::Rel64 => {
                        let pc = patch_offset as u64;
                        let offset = target_addr.wrapping_sub(pc).wrapping_add(reloc.addend as u64);
                        section_data[patch_offset..patch_offset + 8]
                            .copy_from_slice(&offset.to_le_bytes());
                    }
                }
            }
        }

        Ok(())
    }

    // ── Phase 4a: Generate flat binary ───────────────────────────────────

    fn generate_binary(&self) -> Result<Vec<u8>, LinkError> {
        let mut output = Vec::new();

        // Emit sections in order
        let section_order = [".text", ".rodata", ".data", ".bss"];
        for sec_name in &section_order {
            if let Some(data) = self.merged_sections.get(*sec_name) {
                output.extend(data);
            }
        }

        Ok(output)
    }

    // ── Phase 4b: Generate minimal ELF ───────────────────────────────────

    fn generate_elf(&self) -> Result<Vec<u8>, LinkError> {
        let mut output = Vec::new();

        let text_data = self.merged_sections.get(".text").cloned().unwrap_or_default();
        let rodata_data = self.merged_sections.get(".rodata").cloned().unwrap_or_default();
        let data_data = self.merged_sections.get(".data").cloned().unwrap_or_default();

        let text_base = self.section_base.get(".text").copied().unwrap_or(0x400000);
        let data_base = self.section_base.get(".data").copied().unwrap_or(0x600000);
        let entry_addr = self.symbol_addresses.get(&self.config.entry)
            .copied()
            .unwrap_or(text_base);

        let text_size = text_data.len() as u64;
        let rodata_size = rodata_data.len() as u64;
        let data_size = data_data.len() as u64;

        // ELF Header
        let mut elf_header = Vec::new();
        // e_ident
        elf_header.extend(b"\x7fELF");          // magic
        elf_header.push(2);                      // 64-bit
        elf_header.push(1);                      // little endian
        elf_header.push(1);                      // ELF version
        elf_header.push(0);                      // OS/ABI
        elf_header.push(0);                      // ABI version
        elf_header.extend(&[0u8; 7]);            // padding
        // e_type: ET_EXEC (2)
        elf_header.extend(&2u16.to_le_bytes());
        // e_machine: 0xF3 (custom MacroCore-X)
        elf_header.extend(&0xF3u16.to_le_bytes());
        // e_version
        elf_header.extend(&1u32.to_le_bytes());
        // e_entry
        elf_header.extend(&entry_addr.to_le_bytes());
        // e_phoff (program header offset)
        elf_header.extend(&64u64.to_le_bytes());
        // e_shoff (section header offset) - placed at end
        let shoff_offset = elf_header.len();
        elf_header.extend(&0u64.to_le_bytes());
        // e_flags
        elf_header.extend(&0u32.to_le_bytes());
        // e_ehsize
        elf_header.extend(&64u16.to_le_bytes());
        // e_phentsize
        elf_header.extend(&56u16.to_le_bytes());
        // e_phnum
        elf_header.extend(&2u16.to_le_bytes());
        // e_shentsize
        elf_header.extend(&64u16.to_le_bytes());
        // e_shnum
        elf_header.extend(&5u16.to_le_bytes());
        // e_shstrndx
        elf_header.extend(&4u16.to_le_bytes());

        output.extend(&elf_header);

        // Program Headers
        // PHDR 0: LOAD for .text + .rodata
        let phdr0_offset = text_base;
        let phdr0_filesz = text_size + rodata_size;
        let phdr0_memsz = phdr0_filesz;
        let phdr0_offset_in_file = 0x1000u64; // Page-aligned offset

        let mut phdr0 = Vec::new();
        phdr0.extend(&1u32.to_le_bytes());                   // p_type: PT_LOAD
        phdr0.extend(&7u32.to_le_bytes());                   // p_flags: PF_R | PF_W | PF_X
        phdr0.extend(&phdr0_offset_in_file.to_le_bytes());   // p_offset
        phdr0.extend(&phdr0_offset.to_le_bytes());           // p_vaddr
        phdr0.extend(&phdr0_offset.to_le_bytes());           // p_paddr
        phdr0.extend(&phdr0_filesz.to_le_bytes());           // p_filesz
        phdr0.extend(&phdr0_memsz.to_le_bytes());            // p_memsz
        phdr0.extend(&0x1000u64.to_le_bytes());              // p_align
        output.extend(&phdr0);

        // PHDR 1: LOAD for .data
        let phdr1_offset = data_base;
        let phdr1_filesz = data_size;
        let phdr1_memsz = data_size;
        let phdr1_offset_in_file = phdr0_offset_in_file + ((phdr0_filesz + 0xFFF) & !0xFFF);

        let mut phdr1 = Vec::new();
        phdr1.extend(&1u32.to_le_bytes());                   // p_type: PT_LOAD
        phdr1.extend(&6u32.to_le_bytes());                   // p_flags: PF_R | PF_W
        phdr1.extend(&phdr1_offset_in_file.to_le_bytes());   // p_offset
        phdr1.extend(&phdr1_offset.to_le_bytes());           // p_vaddr
        phdr1.extend(&phdr1_offset.to_le_bytes());           // p_paddr
        phdr1.extend(&phdr1_filesz.to_le_bytes());           // p_filesz
        phdr1.extend(&phdr1_memsz.to_le_bytes());            // p_memsz
        phdr1.extend(&0x1000u64.to_le_bytes());              // p_align
        output.extend(&phdr1);

        // Pad to page alignment
        while output.len() < 0x1000 {
            output.push(0);
        }

        // Section data
        output.extend(&text_data);
        output.extend(&rodata_data);

        // Pad .text+.rodata to page alignment
        while (output.len() as u64) < phdr1_offset_in_file {
            output.push(0);
        }

        let shoff = output.len() as u64;

        output.extend(&data_data);

        // Section Headers
        let section_names = [
            b".shstrtab\x00".as_slice(),
            b".text\x00".as_slice(),
            b".rodata\x00".as_slice(),
            b".data\x00".as_slice(),
            b".symtab\x00".as_slice(),
        ];

        // Build section name string table
        let mut shstrtab = Vec::new();
        shstrtab.push(0); // null entry
        let mut name_offsets: Vec<u32> = Vec::new();
        for name in &section_names {
            name_offsets.push(shstrtab.len() as u32);
            shstrtab.extend(*name);
        }

        // Section headers
        // SHDR 0: NULL
        output.extend(&[0u8; 64]);

        // SHDR 1: .text
        let mut shdr = Vec::new();
        shdr.extend(&name_offsets[1].to_le_bytes());  // sh_name
        shdr.extend(&1u32.to_le_bytes());              // sh_type: SHT_PROGBITS
        shdr.extend(&7u64.to_le_bytes());              // sh_flags: SHF_ALLOC | SHF_WRITE | SHF_EXECINSTR
        shdr.extend(&text_base.to_le_bytes());         // sh_addr
        shdr.extend(&0x1000u64.to_le_bytes());         // sh_offset
        shdr.extend(&text_size.to_le_bytes());          // sh_size
        shdr.extend(&0u32.to_le_bytes());              // sh_link
        shdr.extend(&0u32.to_le_bytes());              // sh_info
        shdr.extend(&16u64.to_le_bytes());             // sh_addralign
        shdr.extend(&0u64.to_le_bytes());              // sh_entsize
        output.extend(&shdr);

        // SHDR 2: .rodata
        let mut shdr = Vec::new();
        shdr.extend(&name_offsets[2].to_le_bytes());
        shdr.extend(&1u32.to_le_bytes());
        shdr.extend(&2u64.to_le_bytes());              // SHF_ALLOC
        shdr.extend(&(text_base + text_size).to_le_bytes());
        shdr.extend(&(0x1000u64 + text_size).to_le_bytes());
        shdr.extend(&rodata_size.to_le_bytes());
        shdr.extend(&0u32.to_le_bytes());
        shdr.extend(&0u32.to_le_bytes());
        shdr.extend(&16u64.to_le_bytes());
        shdr.extend(&0u64.to_le_bytes());
        output.extend(&shdr);

        // SHDR 3: .data
        let mut shdr = Vec::new();
        shdr.extend(&name_offsets[3].to_le_bytes());
        shdr.extend(&1u32.to_le_bytes());
        shdr.extend(&3u64.to_le_bytes());              // SHF_ALLOC | SHF_WRITE
        shdr.extend(&data_base.to_le_bytes());
        shdr.extend(&phdr1_offset_in_file.to_le_bytes());
        shdr.extend(&data_size.to_le_bytes());
        shdr.extend(&0u32.to_le_bytes());
        shdr.extend(&0u32.to_le_bytes());
        shdr.extend(&16u64.to_le_bytes());
        shdr.extend(&0u64.to_le_bytes());
        output.extend(&shdr);

        // SHDR 4: .shstrtab
        let mut shdr = Vec::new();
        shdr.extend(&name_offsets[0].to_le_bytes());
        shdr.extend(&3u32.to_le_bytes());              // SHT_STRTAB
        shdr.extend(&0u64.to_le_bytes());
        shdr.extend(&0u64.to_le_bytes());
        shdr.extend(&((output.len() as u64 + 64)).to_le_bytes());      // offset after section headers
        shdr.extend(&(shstrtab.len() as u64).to_le_bytes());
        shdr.extend(&0u32.to_le_bytes());
        shdr.extend(&0u32.to_le_bytes());
        shdr.extend(&1u64.to_le_bytes());
        shdr.extend(&0u64.to_le_bytes());
        output.extend(&shdr);

        // Append shstrtab
        output.extend(&shstrtab);

        // Patch e_shoff in header
        let shoff_bytes = shoff.to_le_bytes();
        output[shoff_offset..shoff_offset + 8].copy_from_slice(&shoff_bytes);

        Ok(output)
    }
}

// =============================================================================
//  Object file serialization
// =============================================================================

impl ObjectFile {
    /// Serialize an object file to bytes.
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Magic
        buf.extend(b"NOVA");

        // Header
        buf.extend(&(self.sections.len() as u32).to_le_bytes());
        buf.extend(&(self.symbols.len() as u32).to_le_bytes());
        buf.extend(&(self.relocations.len() as u32).to_le_bytes());
        buf.extend(&(self.target.len() as u32).to_le_bytes());
        buf.extend(&[0u32; 4].map(|x| x.to_le_bytes()).concat()); // reserved

        // Target string
        buf.extend(self.target.as_bytes());

        // Section table
        for section in &self.sections {
            let name_len = section.name.len() as u32;
            buf.extend(&name_len.to_le_bytes());
            buf.extend(section.name.as_bytes());
            buf.extend(&(section.data.len() as u32).to_le_bytes());
            buf.extend(&section.flags.to_le_bytes());
            buf.extend(&section.alignment.to_le_bytes());
            buf.extend(&section.data);
        }

        // Symbol table
        for sym in &self.symbols {
            let name_len = sym.name.len() as u32;
            buf.extend(&name_len.to_le_bytes());
            buf.extend(sym.name.as_bytes());
            buf.extend(&sym.section_index.to_le_bytes());
            buf.extend(&sym.offset.to_le_bytes());
            buf.extend(&sym.size.to_le_bytes());
            buf.push(sym.sym_type);
            buf.push(sym.binding);
            buf.extend(&[0u8; 2]); // padding
        }

        // Relocation table
        for reloc in &self.relocations {
            buf.extend(&reloc.offset.to_le_bytes());
            buf.extend(&reloc.symbol_index.to_le_bytes());
            let rt: u8 = match reloc.reloc_type {
                RelocType::Abs32 => 0,
                RelocType::Abs64 => 1,
                RelocType::PcRel12 => 2,
                RelocType::PcRel20 => 3,
                RelocType::Rel32 => 4,
                RelocType::Rel64 => 5,
            };
            buf.push(rt);
            buf.extend(&[0u8; 3]); // padding
            buf.extend(&reloc.addend.to_le_bytes());
        }

        buf
    }

    /// Deserialize an object file from bytes.
    pub fn deserialize(data: &[u8], name: &str) -> Result<Self, LinkError> {
        if data.len() < 32 {
            return Err(LinkError::InvalidObject("file too short".to_string()));
        }

        // Check magic
        if &data[0..4] != b"NOVA" {
            return Err(LinkError::InvalidObject("invalid magic".to_string()));
        }

        let mut offset = 4;

        // Read header
        let num_sections = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
        offset += 4;
        let num_symbols = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
        offset += 4;
        let num_relocations = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
        offset += 4;
        let target_len = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
        offset += 4;
        offset += 16; // skip reserved

        // Read target
        let target = String::from_utf8_lossy(&data[offset..offset + target_len as usize]).to_string();
        offset += target_len as usize;

        // Read sections
        let mut sections = Vec::new();
        for _ in 0..num_sections {
            let name_len = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap()) as usize;
            offset += 4;
            let name = String::from_utf8_lossy(&data[offset..offset+name_len]).to_string();
            offset += name_len;
            let data_len = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap()) as usize;
            offset += 4;
            let flags = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
            offset += 4;
            let alignment = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
            offset += 4;
            let section_data = data[offset..offset+data_len].to_vec();
            offset += data_len;
            sections.push(Section { name, data: section_data, flags, alignment });
        }

        // Read symbols
        let mut symbols = Vec::new();
        for _ in 0..num_symbols {
            let name_len = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap()) as usize;
            offset += 4;
            let name = String::from_utf8_lossy(&data[offset..offset+name_len]).to_string();
            offset += name_len;
            let section_index = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
            offset += 4;
            let sym_offset = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
            offset += 4;
            let size = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
            offset += 4;
            let sym_type = data[offset];
            offset += 1;
            let binding = data[offset];
            offset += 1;
            offset += 2; // padding
            symbols.push(Symbol {
                name, section_index, offset: sym_offset, size, sym_type, binding,
            });
        }

        // Read relocations
        let mut relocations = Vec::new();
        for _ in 0..num_relocations {
            let reloc_offset = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
            offset += 4;
            let symbol_index = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
            offset += 4;
            let reloc_type = match data[offset] {
                0 => RelocType::Abs32,
                1 => RelocType::Abs64,
                2 => RelocType::PcRel12,
                3 => RelocType::PcRel20,
                4 => RelocType::Rel32,
                5 => RelocType::Rel64,
                t => return Err(LinkError::InvalidRelocType(t)),
            };
            offset += 4; // skip RT byte + padding
            let addend = i64::from_le_bytes(data[offset..offset+8].try_into().unwrap());
            offset += 8;
            relocations.push(Relocation {
                offset: reloc_offset, symbol_index, reloc_type, addend,
            });
        }

        Ok(ObjectFile {
            name: name.to_string(),
            target,
            sections,
            symbols,
            relocations,
        })
    }
}

// =============================================================================
//  Convenience API
// =============================================================================

/// Create a simple object file from a single binary blob for a section.
pub fn create_object_from_binary(name: &str, target: &str, section_name: &str, data: Vec<u8>) -> ObjectFile {
    ObjectFile {
        name: name.to_string(),
        target: target.to_string(),
        sections: vec![Section {
            name: section_name.to_string(),
            data,
            flags: if section_name == ".text" { 7 } else { 3 },
            alignment: 4,
        }],
        symbols: vec![Symbol {
            name: "_start".to_string(),
            section_index: 1,
            offset: 0,
            size: 0,
            sym_type: 1, // func
            binding: 1,  // global
        }],
        relocations: Vec::new(),
    }
}

/// Link a single binary blob into an executable.
pub fn link_binary(data: &[u8], text_base: u64, format: OutputFormat) -> Result<Vec<u8>, LinkError> {
    let obj = create_object_from_binary("input.o", "macrocore-x", ".text", data.to_vec());
    let config = LinkConfig {
        format,
        text_base,
        data_base: text_base + 0x10000,
        entry: "_start".to_string(),
        output: "a.out".to_string(),
    };
    let mut linker = Linker::new(config);
    linker.add_object(obj);
    linker.link()
}

// =============================================================================
//  Error types
// =============================================================================

#[derive(Debug, Clone)]
pub enum LinkError {
    NoObjects,
    InvalidObject(String),
    InvalidSectionIndex { name: String, index: u32 },
    InvalidSymbolIndex { index: u32 },
    InvalidRelocType(u8),
    UndefinedSymbol { name: String },
    DuplicateSymbol { name: String, addr1: u64, addr2: u64 },
    MissingSection { name: String },
    RelocationOutOfBounds { offset: usize, section_size: usize },
    Io(String),
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkError::NoObjects => write!(f, "no object files to link"),
            LinkError::InvalidObject(msg) => write!(f, "invalid object file: {msg}"),
            LinkError::InvalidSectionIndex { name, index } => {
                write!(f, "symbol '{name}' references invalid section index {index}")
            }
            LinkError::InvalidSymbolIndex { index } => {
                write!(f, "relocation references invalid symbol index {index}")
            }
            LinkError::InvalidRelocType(t) => write!(f, "invalid relocation type: {t}"),
            LinkError::UndefinedSymbol { name } => write!(f, "undefined symbol: {name}"),
            LinkError::DuplicateSymbol { name, addr1, addr2 } => {
                write!(f, "duplicate symbol '{name}': 0x{addr1:x} and 0x{addr2:x}")
            }
            LinkError::MissingSection { name } => write!(f, "missing section: {name}"),
            LinkError::RelocationOutOfBounds { offset, section_size } => {
                write!(f, "relocation at offset {offset} out of bounds (section size {section_size})")
            }
            LinkError::Io(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

impl std::error::Error for LinkError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_link_binary() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        let result = link_binary(&data, 0x1000, OutputFormat::Binary);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output, data);
    }

    #[test]
    fn test_simple_link_elf() {
        let data = vec![0x00; 64]; // 64 bytes of code
        let result = link_binary(&data, 0x400000, OutputFormat::Elf);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.len() > 64);
        // Check ELF magic
        assert_eq!(&output[0..4], b"\x7fELF");
    }

    #[test]
    fn test_object_serialization_roundtrip() {
        let obj = ObjectFile {
            name: "test.o".to_string(),
            target: "macrocore-x".to_string(),
            sections: vec![
                Section {
                    name: ".text".to_string(),
                    data: vec![0x01, 0x02, 0x03, 0x04],
                    flags: 7,
                    alignment: 4,
                },
                Section {
                    name: ".data".to_string(),
                    data: vec![0xAA, 0xBB],
                    flags: 3,
                    alignment: 4,
                },
            ],
            symbols: vec![
                Symbol {
                    name: "_start".to_string(),
                    section_index: 1,
                    offset: 0,
                    size: 4,
                    sym_type: 1,
                    binding: 1,
                },
                Symbol {
                    name: "counter".to_string(),
                    section_index: 2,
                    offset: 0,
                    size: 2,
                    sym_type: 2,
                    binding: 1,
                },
            ],
            relocations: vec![
                Relocation {
                    offset: 0,
                    symbol_index: 1,
                    reloc_type: RelocType::Abs32,
                    addend: 0,
                },
            ],
        };

        let serialized = obj.serialize();
        let deserialized = ObjectFile::deserialize(&serialized, "test.o").unwrap();

        assert_eq!(deserialized.name, "test.o");
        assert_eq!(deserialized.target, "macrocore-x");
        assert_eq!(deserialized.sections.len(), 2);
        assert_eq!(deserialized.sections[0].name, ".text");
        assert_eq!(deserialized.sections[0].data, vec![0x01, 0x02, 0x03, 0x04]);
        assert_eq!(deserialized.sections[1].name, ".data");
        assert_eq!(deserialized.symbols.len(), 2);
        assert_eq!(deserialized.symbols[0].name, "_start");
        assert_eq!(deserialized.symbols[1].name, "counter");
        assert_eq!(deserialized.relocations.len(), 1);
        assert_eq!(deserialized.relocations[0].reloc_type, RelocType::Abs32);
    }

    #[test]
    fn test_linker_no_objects() {
        let config = LinkConfig::default();
        let mut linker = Linker::new(config);
        let result = linker.link();
        assert!(result.is_err());
    }

    #[test]
    fn test_linker_with_multiple_objects() {
        let obj1 = ObjectFile {
            name: "a.o".to_string(),
            target: "macrocore-x".to_string(),
            sections: vec![Section {
                name: ".text".to_string(),
                data: vec![0x00; 16],
                flags: 7,
                alignment: 4,
            }],
            symbols: vec![Symbol {
                name: "func_a".to_string(),
                section_index: 1,
                offset: 0,
                size: 16,
                sym_type: 1,
                binding: 1,
            }],
            relocations: vec![],
        };

        let obj2 = ObjectFile {
            name: "b.o".to_string(),
            target: "macrocore-x".to_string(),
            sections: vec![Section {
                name: ".text".to_string(),
                data: vec![0x01; 32],
                flags: 7,
                alignment: 4,
            }],
            symbols: vec![Symbol {
                name: "_start".to_string(),
                section_index: 1,
                offset: 0,
                size: 32,
                sym_type: 1,
                binding: 1,
            }],
            relocations: vec![],
        };

        let config = LinkConfig {
            text_base: 0x1000,
            entry: "_start".to_string(),
            ..LinkConfig::default()
        };
        let mut linker = Linker::new(config);
        linker.add_object(obj1);
        linker.add_object(obj2);
        let result = linker.link();
        assert!(result.is_ok());
        let output = result.unwrap();
        // .text from both objects should be merged
        assert_eq!(output.len(), 48); // 16 + 32
    }
}