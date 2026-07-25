//! Main assembler: two-pass assembly with label resolution.
//!
//! Port of the Python reference assembler at `/workspace/assembler.py`.

use crate::error::AsmError;
use crate::isa;
use crate::lexer::ParsedInstruction;

/// Default base address matching the simulator.
pub const BASE_ADDR: u32 = 0x1000;

/// Pending label reference to be resolved in pass 2.
#[derive(Debug, Clone)]
struct PendingRef {
    label: String,
    patch_offset: usize,
    ref_type: PendingRefType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingRefType {
    /// 12-bit branch offset (conditional branches)
    B12,
    /// 20-bit jump offset (j, call)
    B20,
    /// 32-bit absolute address (la → movi, movi with label)
    LaMovi,
}

/// The MacroCore-X assembler.
pub struct Assembler {
    base_addr: u32,
    labels: std::collections::HashMap<String, usize>,
    pending: Vec<PendingRef>,
    output: Vec<u8>,
    offset: usize,
}

impl Assembler {
    pub fn new(base_addr: u32) -> Self {
        Assembler {
            base_addr,
            labels: std::collections::HashMap::new(),
            pending: Vec::new(),
            output: Vec::new(),
            offset: 0,
        }
    }

    pub fn default() -> Self {
        Self::new(BASE_ADDR)
    }

    /// Assemble a list of parsed instructions into binary.
    pub fn assemble(
        &mut self,
        instructions: &[ParsedInstruction],
    ) -> Result<Vec<u8>, AsmError> {
        // Pass 1: collect labels and emit code
        for inst in instructions {
            if let Some(ref label) = inst.label {
                self.labels.insert(label.clone(), self.offset);
            }
            self.emit_instruction(inst)?;
        }

        // Pass 2: resolve pending label references
        let output = std::mem::take(&mut self.output);
        let mut output = output;

        for pending in &self.pending {
            let target = self
                .labels
                .get(&pending.label)
                .ok_or_else(|| AsmError::UndefinedLabel {
                    label: pending.label.clone(),
                })?;
            let target = *target;

            match pending.ref_type {
                PendingRefType::B12 => {
                    // 12-bit branch offset: patch at byte2-3
                    let pc = pending.patch_offset.wrapping_sub(2);
                    let imm12 = (target as i64 - pc as i64) >> 2;
                    let imm12 = sext(imm12, 12);
                    output[pending.patch_offset] = ((imm12 >> 8) & 0xFF) as u8;
                    output[pending.patch_offset + 1] = (imm12 & 0xFF) as u8;
                }
                PendingRefType::B20 => {
                    // 20-bit jump offset: patch at byte1-3
                    let pc = pending.patch_offset.wrapping_sub(1);
                    let imm20 = (target as i64 - pc as i64) >> 2;
                    let imm20 = sext(imm20, 20);
                    output[pending.patch_offset] = ((imm20 >> 12) & 0xFF) as u8;
                    output[pending.patch_offset + 1] = (imm20 & 0xFF) as u8;
                    output[pending.patch_offset + 2] = ((imm20 >> 8) & 0xF) as u8;
                }
                PendingRefType::LaMovi => {
                    // la → movi: patch 32-bit absolute address at byte2-5
                    let addr = target as u32 + self.base_addr;
                    output[pending.patch_offset] = (addr & 0xFF) as u8;
                    output[pending.patch_offset + 1] = ((addr >> 8) & 0xFF) as u8;
                    output[pending.patch_offset + 2] = ((addr >> 16) & 0xFF) as u8;
                    output[pending.patch_offset + 3] = ((addr >> 24) & 0xFF) as u8;
                }
            }
        }

        self.output = output.clone();
        self.offset = output.len();
        Ok(output)
    }

    fn emit_instruction(&mut self, inst: &ParsedInstruction) -> Result<(), AsmError> {
        let mnemonic = &inst.mnemonic;
        let ops = &inst.operands;

        if isa::is_r_type(mnemonic) {
            self.emit_r(mnemonic, ops, inst.line)?
        } else if isa::is_i_type(mnemonic) {
            self.emit_i(mnemonic, ops, inst.line)?
        } else if isa::is_l4_type(mnemonic) {
            self.emit_l4(mnemonic, ops, inst.line)?
        } else if isa::is_l6_type(mnemonic) {
            self.emit_l6(mnemonic, ops, inst.line)?
        } else if isa::is_b_type(mnemonic) {
            self.emit_b(mnemonic, ops, inst.line)?
        } else if isa::is_v_type(mnemonic) {
            self.emit_v(mnemonic, ops, inst.line)?
        } else if isa::is_f_type(mnemonic) {
            self.emit_f(mnemonic, ops, inst.line)?
        } else if isa::is_c_type(mnemonic) {
            self.emit_c(mnemonic, ops, inst.line)?
        } else if isa::is_sys2(mnemonic) {
            self.emit_sys2(mnemonic, ops, inst.line)?
        } else if isa::is_sys4(mnemonic) {
            self.emit_sys4(mnemonic, ops, inst.line)?
        } else if mnemonic == "li" {
            self.emit_li(ops, inst.line)?
        } else if mnemonic == "la" {
            self.emit_la(ops, inst.line)?
        } else if mnemonic == ".word" {
            self.emit_dot_word(ops, inst.line)?
        } else if mnemonic == ".byte" {
            self.emit_dot_byte(ops, inst.line)?
        } else if mnemonic == ".ascii" {
            self.emit_dot_ascii(ops, inst.line)?
        } else {
            return Err(AsmError::UnknownMnemonic {
                mnemonic: mnemonic.clone(),
                line: inst.line,
            });
        }
        Ok(())
    }

    // ── R-type ────────────────────────────────────────────────────────────

    fn emit_r(&mut self, mnemonic: &str, ops: &[String], line: usize) -> Result<(), AsmError> {
        let opcode = isa::r_type_opcode(mnemonic).unwrap();
        let rd = parse_reg(&ops[0], line)?;

        let (rs1, rs2) = if mnemonic == "clz" {
            (parse_reg(&ops[1], line)?, 0u8)
        } else {
            (parse_reg(&ops[1], line)?, parse_reg(&ops[2], line)?)
        };

        self.output.push(opcode);
        self.output.push(((rd & 0x1F) << 3) | ((rs1 >> 2) & 0x7));
        self.output.push(((rs1 & 0x3) << 6) | ((rs2 & 0x1F) << 1) | 0);
        self.output.push(0x00);
        self.offset += 4;
        Ok(())
    }

    // ── I-type ────────────────────────────────────────────────────────────

    fn emit_i(&mut self, mnemonic: &str, ops: &[String], line: usize) -> Result<(), AsmError> {
        if mnemonic == "movi" {
            return self.emit_movi(ops, line);
        }

        let opcode = isa::i_type_opcode(mnemonic).unwrap();
        let rd = parse_reg(&ops[0], line)?;

        if mnemonic == "mov" {
            // mov rd, rs → emit as R-type add rd, rs, r0
            let op = &ops[1];
            if op.starts_with('r') || op.starts_with('R') {
                let rs1 = parse_reg(op, line)?;
                let add_opcode = isa::r_type_opcode("add").unwrap();
                self.output.push(add_opcode);
                self.output.push(((rd & 0x1F) << 3) | ((rs1 >> 2) & 0x7));
                self.output.push(((rs1 & 0x3) << 6) | ((0 & 0x1F) << 1) | 0);
                self.output.push(0x00);
                self.offset += 4;
                return Ok(());
            } else {
                let imm = parse_imm(op, line)?;
                if !(-8192..=8191).contains(&imm) {
                    return Err(AsmError::ImmediateRange {
                        line,
                        msg: format!("mov immediate must be -8192..8191 (14-bit signed), got {imm}"),
                    });
                }
                self.output.push(opcode);
                self.output.push(((rd & 0x1F) << 3) | 0); // rs1=0
                self.output.push(((imm >> 8) & 0x3F) as u8);
                self.output.push((imm & 0xFF) as u8);
                self.offset += 4;
                return Ok(());
            }
        }

        // Other I-type: addi, subi, muli, andi, ori, xori, shli, shri, sari
        let rs1 = parse_reg(&ops[1], line)?;
        let imm = parse_imm(&ops[2], line)?;

        if matches!(mnemonic, "shli" | "shri" | "sari") {
            if !(0..=63).contains(&imm) {
                return Err(AsmError::ImmediateRange {
                    line,
                    msg: format!("shift amount must be 0-63, got {imm}"),
                });
            }
            let imm = imm & 0x3F;
            self.output.push(opcode);
            self.output.push(((rd & 0x1F) << 3) | ((rs1 >> 2) & 0x7));
            self.output.push(((rs1 & 0x3) << 6) | ((imm >> 8) & 0x3F) as u8);
            self.output.push((imm & 0xFF) as u8);
            self.offset += 4;
            return Ok(());
        }

        if !(-8192..=8191).contains(&imm) {
            return Err(AsmError::ImmediateRange {
                line,
                msg: format!("{mnemonic} immediate must be -8192..8191 (14-bit signed), got {imm}"),
            });
        }

        self.output.push(opcode);
        self.output.push(((rd & 0x1F) << 3) | ((rs1 >> 2) & 0x7));
        self.output.push(((rs1 & 0x3) << 6) | ((imm >> 8) & 0x3F) as u8);
        self.output.push((imm & 0xFF) as u8);
        self.offset += 4;
        Ok(())
    }

    fn emit_movi(&mut self, ops: &[String], line: usize) -> Result<(), AsmError> {
        let rd = parse_reg(&ops[0], line)?;
        let imm_str = &ops[1];

        if is_label_ref(imm_str) {
            // Label reference — emit placeholder and defer
            self.output.push(0x2A); // movi opcode
            self.output.push(((rd & 0x1F) << 3) | 0);
            self.output.push(0);
            self.output.push(0);
            self.output.push(0);
            self.output.push(0);
            self.pending.push(PendingRef {
                label: imm_str.clone(),
                patch_offset: self.offset + 2,
                ref_type: PendingRefType::LaMovi,
            });
            self.offset += 6;
            return Ok(());
        }

        let imm = parse_imm(imm_str, line)?;
        let opcode = isa::i_type_opcode("movi").unwrap();
        self.output.push(opcode);
        self.output.push(((rd & 0x1F) << 3) | 0);
        let imm32 = imm as u32;
        self.output.push((imm32 & 0xFF) as u8);
        self.output.push(((imm32 >> 8) & 0xFF) as u8);
        self.output.push(((imm32 >> 16) & 0xFF) as u8);
        self.output.push(((imm32 >> 24) & 0xFF) as u8);
        self.offset += 6;
        Ok(())
    }

    // ── L-type 4-byte ─────────────────────────────────────────────────────

    fn emit_l4(&mut self, mnemonic: &str, ops: &[String], line: usize) -> Result<(), AsmError> {
        let opcode = isa::l4_type_opcode(mnemonic).unwrap();

        if mnemonic == "lda" {
            let rd = parse_reg4(&ops[0], line, "lda")?;
            let rs1 = parse_reg4(&ops[1], line, "lda")?;
            let rs2 = parse_reg4(&ops[2], line, "lda")?;
            let scale = parse_imm(&ops[3], line)? as u32;
            let scale_bits = match scale {
                1 => 0, 2 => 1, 4 => 2, 8 => 3,
                _ => return Err(AsmError::InvalidOperand {
                    line,
                    msg: format!("lda scale must be 1, 2, 4, or 8, got {scale}"),
                }),
            };
            self.output.push(opcode);
            self.output.push(((rd & 0xF) << 4) | (rs1 & 0xF));
            self.output.push(((rs2 & 0xF) << 2) as u8 | scale_bits);
            self.output.push(0);
            self.offset += 4;
            return Ok(());
        }

        // Parse memory operand
        let (rd, rs1_field, off, _sz) = if mnemonic.starts_with("st") {
            let rs1 = parse_reg4(&ops[0], line, mnemonic)?;
            let (base, off, _, _) = parse_mem_operand(&ops[1], line)?;
            (rs1, base, off, 0)
        } else {
            let rd = parse_reg4(&ops[0], line, mnemonic)?;
            let (base, off, _, _) = parse_mem_operand(&ops[1], line)?;
            (rd, base, off, 0)
        };

        let _sz = match mnemonic {
            "stb" => 0u8,
            "stw" => 2u8,
            "ldu" | "lds" => 2u8,
            _ => 3u8, // default 64-bit
        };

        self.output.push(opcode);
        self.output.push(((rd & 0xF) << 4) | (rs1_field & 0xF));
        let off16 = off as i16;
        self.output.push((off16 & 0xFF) as u8);
        self.output.push(((off16 >> 8) & 0xFF) as u8);
        self.offset += 4;
        Ok(())
    }

    // ── L-type 6-byte ─────────────────────────────────────────────────────

    fn emit_l6(&mut self, mnemonic: &str, ops: &[String], line: usize) -> Result<(), AsmError> {
        let opcode = isa::l6_type_opcode(mnemonic).unwrap();

        let (rd, rs1_field, off, idx_reg, scale) = if mnemonic == "ldr" {
            let rd = parse_reg4(&ops[0], line, "ldr")?;
            let (base, off, idx, sc) = parse_mem_operand(&ops[1], line)?;
            (rd, base, off, idx, sc)
        } else {
            let rs1 = parse_reg4(&ops[0], line, "str")?;
            let (base, off, idx, sc) = parse_mem_operand(&ops[1], line)?;
            (rs1, base, off, idx, sc)
        };

        let scale_bits = match scale {
            1 => 0u8, 2 => 1, 4 => 2, 8 => 3,
            _ => 0u8,
        };

        self.output.push(opcode);
        self.output.push(((rd & 0xF) << 4) | (rs1_field & 0xF));
        let off16 = off as i16;
        self.output.push((off16 & 0xFF) as u8);
        self.output.push(((off16 >> 8) & 0xFF) as u8);
        self.output.push(((idx_reg & 0xF) << 2) as u8 | scale_bits);
        self.output.push(0);
        self.offset += 6;
        Ok(())
    }

    // ── B-type ────────────────────────────────────────────────────────────

    fn emit_b(&mut self, mnemonic: &str, ops: &[String], line: usize) -> Result<(), AsmError> {
        let opcode = isa::b_type_opcode(mnemonic).unwrap();

        if mnemonic == "ret" {
            self.output.push(opcode);
            self.output.extend(&[0, 0, 0]);
            self.offset += 4;
            return Ok(());
        }

        if mnemonic == "j" || mnemonic == "call" {
            let target_str = &ops[0];
            if is_numeric(target_str) {
                let target = parse_imm(target_str, line)? as usize;
                let pc = self.offset;
                let imm20 = (target as i64 - pc as i64) >> 2;
                let imm20 = sext(imm20, 20);
                self.output.push(opcode);
                self.output.push(((imm20 >> 12) & 0xFF) as u8);
                self.output.push((imm20 & 0xFF) as u8);
                self.output.push(((imm20 >> 8) & 0xF) as u8);
            } else {
                self.output.push(opcode);
                self.output.push(0);
                self.output.push(0);
                self.output.push(0);
                self.pending.push(PendingRef {
                    label: target_str.clone(),
                    patch_offset: self.offset + 1,
                    ref_type: PendingRefType::B20,
                });
            }
            self.offset += 4;
            return Ok(());
        }

        if mnemonic == "jreg" || mnemonic == "callreg" {
            let rs1 = parse_reg4(&ops[0], line, mnemonic)?;
            self.output.push(opcode);
            self.output.push((rs1 & 0xF) << 4);
            self.output.extend(&[0, 0]);
            self.offset += 4;
            return Ok(());
        }

        // Conditional branches
        let rs1 = parse_reg4(&ops[0], line, mnemonic)?;
        let rs2 = parse_reg4(&ops[1], line, mnemonic)?;
        let target_str = &ops[2];

        self.output.push(opcode);
        self.output.push(((rs1 & 0xF) << 4) | (rs2 & 0xF));

        if is_numeric(target_str) {
            let target = parse_imm(target_str, line)? as usize;
            let pc = self.offset;
            let imm12 = (target as i64 - pc as i64) >> 2;
            let imm12 = sext(imm12, 12);
            self.output.push(((imm12 >> 8) & 0xFF) as u8);
            self.output.push((imm12 & 0xFF) as u8);
        } else {
            self.output.push(0);
            self.output.push(0);
            self.pending.push(PendingRef {
                label: target_str.clone(),
                patch_offset: self.offset + 2,
                ref_type: PendingRefType::B12,
            });
        }
        self.offset += 4;
        Ok(())
    }

    // ── V-type ────────────────────────────────────────────────────────────

    fn emit_v(&mut self, mnemonic: &str, ops: &[String], line: usize) -> Result<(), AsmError> {
        let funct = isa::v_type_funct(mnemonic).unwrap();

        if mnemonic == "vld" || mnemonic == "vst" {
            let vd = parse_vreg(&ops[0], line)?;
            let (base, off, _, _) = parse_mem_operand(&ops[1], line)?;
            self.output.push(0x80 | (vd & 0xF));
            self.output.push(((base & 0xF) << 4) | 0);
            self.output.push(funct);
            self.output.push(0);
            let off16 = off as i16;
            self.output.push((off16 & 0xFF) as u8);
            self.output.push(((off16 >> 8) & 0xFF) as u8);
            self.offset += 6;
            return Ok(());
        }

        if mnemonic == "vshl" || mnemonic == "vshr" {
            let vd = parse_vreg(&ops[0], line)?;
            let vs1 = parse_vreg(&ops[1], line)?;
            let imm = parse_imm(&ops[2], line)? & 0x1F;
            self.output.push(0x80 | (vd & 0xF));
            self.output.push(((vs1 & 0xF) << 4) | (imm as u8 & 0xF));
            self.output.push(funct);
            self.output.push(0);
            self.output.extend(&[0, 0]);
            self.offset += 6;
            return Ok(());
        }

        if mnemonic == "vshuffle" {
            let vd = parse_vreg(&ops[0], line)?;
            let vs1 = parse_vreg(&ops[1], line)?;
            let imm = parse_imm(&ops[2], line)? & 0xFF;
            self.output.push(0x80 | (vd & 0xF));
            self.output.push(((vs1 & 0xF) << 4) | 0);
            self.output.push(funct);
            self.output.push(imm as u8);
            self.output.extend(&[0, 0]);
            self.offset += 6;
            return Ok(());
        }

        if mnemonic == "vfmadd" {
            let vd = parse_vreg(&ops[0], line)?;
            let vs1 = parse_vreg(&ops[1], line)?;
            let vs2 = parse_vreg(&ops[2], line)?;
            let vs3 = parse_vreg(&ops[3], line)?;
            self.output.push(0x80 | (vd & 0xF));
            self.output.push(((vs1 & 0xF) << 4) | (vs2 & 0xF));
            self.output.push(funct);
            self.output.push(0);
            self.output.push((vs3 & 0xF) as u8);
            self.output.push(0);
            self.output.extend(&[0, 0]);
            self.offset += 8;
            return Ok(());
        }

        // vadd, vsub, vmul, vand, vor, vxor
        let vd = parse_vreg(&ops[0], line)?;
        let vs1 = parse_vreg(&ops[1], line)?;
        let vs2 = parse_vreg(&ops[2], line)?;
        self.output.push(0x80 | (vd & 0xF));
        self.output.push(((vs1 & 0xF) << 4) | (vs2 & 0xF));
        self.output.push(funct);
        self.output.push(0);
        self.output.extend(&[0, 0]);
        self.offset += 6;
        Ok(())
    }

    // ── F-type ────────────────────────────────────────────────────────────

    fn emit_f(&mut self, mnemonic: &str, ops: &[String], line: usize) -> Result<(), AsmError> {
        let funct = isa::f_type_funct(mnemonic).unwrap();

        let (fd, fs1, fs2, off) = if mnemonic == "fcmp" {
            let fs1 = parse_freg(&ops[0], line)?;
            let fs2 = parse_freg(&ops[1], line)?;
            (0u8, fs1, fs2, 0i64)
        } else if matches!(mnemonic, "fsqrt" | "fneg" | "fabs" | "fcvt.w.s" | "fcvt.s.w") {
            let fd = parse_freg(&ops[0], line)?;
            let fs1 = parse_freg(&ops[1], line)?;
            (fd, fs1, 0u8, 0i64)
        } else if mnemonic == "fld" || mnemonic == "fst" {
            let fd = parse_freg(&ops[0], line)?;
            let (base, off, _, _) = parse_mem_operand(&ops[1], line)?;
            (fd, base as u8, 0u8, off)
        } else {
            let fd = parse_freg(&ops[0], line)?;
            let fs1 = parse_freg(&ops[1], line)?;
            let fs2 = parse_freg(&ops[2], line)?;
            (fd, fs1, fs2, 0i64)
        };

        let aux: u8 = 0; // default: f32, RNE

        self.output.push(0xA0 | (fd & 0xF));
        self.output.push(((fs1 & 0xF) << 4) | (fs2 & 0xF));
        self.output.push(funct);
        self.output.push(aux);

        if mnemonic == "fld" || mnemonic == "fst" {
            let off16 = off as i16;
            self.output.push((off16 & 0xFF) as u8);
            self.output.push(((off16 >> 8) & 0xFF) as u8);
        } else {
            self.output.extend(&[0, 0]);
        }
        self.offset += 6;
        Ok(())
    }

    // ── C-type ────────────────────────────────────────────────────────────

    fn emit_c(&mut self, mnemonic: &str, ops: &[String], line: usize) -> Result<(), AsmError> {
        let opcode = isa::c_type_opcode(mnemonic).unwrap();

        if mnemonic == "addm" || mnemonic == "subm" {
            let rs1 = parse_reg4(&ops[0], line, mnemonic)?;
            let (base, off, _, _) = parse_mem_operand(&ops[1], line)?;
            self.output.push(opcode);
            self.output.push(((rs1 & 0xF) << 4) | (base & 0xF));
            let off16 = off as i16;
            self.output.push((off16 & 0xFF) as u8);
            self.output.push(((off16 >> 8) & 0xFF) as u8);
            self.output.extend(&[0, 0]);
            self.offset += 6;
        } else if mnemonic == "xchg" {
            let rs1 = parse_reg4(&ops[0], line, "xchg")?;
            let (base, off, _, _) = parse_mem_operand(&ops[1], line)?;
            self.output.push(opcode);
            self.output.push(((rs1 & 0xF) << 4) | (base & 0xF));
            let off16 = off as i16;
            self.output.push((off16 & 0xFF) as u8);
            self.output.push(((off16 >> 8) & 0xFF) as u8);
            self.output.extend(&[0, 0]);
            self.offset += 6;
        } else if mnemonic == "cmpxchg" {
            let rs1 = parse_reg4(&ops[0], line, "cmpxchg")?;
            let rs2 = parse_reg4(&ops[1], line, "cmpxchg")?;
            let (base, off, _, _) = parse_mem_operand(&ops[2], line)?;
            self.output.push(opcode);
            self.output.push(((rs1 & 0xF) << 4) | (rs2 & 0xF));
            let off16 = off as i16;
            self.output.push((off16 & 0xFF) as u8);
            self.output.push(((off16 >> 8) & 0xFF) as u8);
            self.output.push((base & 0xF) as u8);
            self.output.push(0);
            self.offset += 6;
        } else if mnemonic == "push" || mnemonic == "pop" {
            let rs1 = parse_reg4(&ops[0], line, mnemonic)?;
            self.output.push(opcode);
            self.output.push(((rs1 & 0xF) << 4) | 0);
            self.output.extend(&[0, 0, 0, 0]);
            self.offset += 6;
        } else if mnemonic == "enter" {
            let imm = parse_imm(&ops[0], line)?;
            self.output.push(opcode);
            self.output.push(0);
            let imm16 = imm as i16;
            self.output.push((imm16 & 0xFF) as u8);
            self.output.push(((imm16 >> 8) & 0xFF) as u8);
            self.output.extend(&[0, 0]);
            self.offset += 6;
        } else if mnemonic == "leave" {
            self.output.push(opcode);
            self.output.extend(&[0, 0, 0, 0, 0]);
            self.offset += 6;
        }

        Ok(())
    }

    // ── System 2-byte ─────────────────────────────────────────────────────

    fn emit_sys2(&mut self, mnemonic: &str, ops: &[String], line: usize) -> Result<(), AsmError> {
        let opcode = isa::sys2_opcode(mnemonic).unwrap();

        if !ops.is_empty() && matches!(mnemonic, "syscall" | "int" | "ecall" | "bkpt") {
            let imm8 = parse_imm(&ops[0], line)? & 0xFF;
            self.output.push(opcode);
            self.output.push(imm8 as u8);
        } else if mnemonic == "fence" {
            let (pi, po) = if !ops.is_empty() {
                let p = parse_imm(&ops[0], line)? & 0xF;
                let q = if ops.len() > 1 {
                    parse_imm(&ops[1], line)? & 0xF
                } else {
                    p
                };
                (p, q)
            } else {
                (0xF, 0xF)
            };
            self.output.push(opcode);
            self.output.push(((pi & 0xF) << 4) as u8 | (po & 0xF) as u8);
        } else {
            self.output.push(opcode);
            self.output.push(0);
        }
        self.offset += 2;
        Ok(())
    }

    // ── System 4-byte ─────────────────────────────────────────────────────

    fn emit_sys4(&mut self, mnemonic: &str, ops: &[String], line: usize) -> Result<(), AsmError> {
        let opcode = isa::sys4_opcode(mnemonic).unwrap();
        let rs1 = parse_reg(&ops[0], line)?;
        let imm12 = parse_imm(&ops[1], line)? & 0xFFF;
        self.output.push(opcode);
        self.output.push(((rs1 & 0xF) << 4) | ((imm12 >> 8) & 0xF) as u8);
        self.output.push((imm12 & 0xFF) as u8);
        self.output.push(0);
        self.offset += 4;
        Ok(())
    }

    // ── Pseudo-instructions ───────────────────────────────────────────────

    fn emit_li(&mut self, ops: &[String], line: usize) -> Result<(), AsmError> {
        let imm = parse_imm(&ops[1], line)?;
        if (-8192..=8191).contains(&imm) {
            // Use mov (4-byte)
            let rd = ops[0].clone();
            let imm_str = ops[1].clone();
            self.emit_i("mov", &[rd, imm_str], line)
        } else {
            // Use movi (6-byte)
            self.emit_movi(ops, line)
        }
    }

    fn emit_la(&mut self, ops: &[String], line: usize) -> Result<(), AsmError> {
        // la Rd, label → movi Rd, label_addr
        let rd = parse_reg(&ops[0], line)?;
        let target_str = &ops[1];

        if is_numeric(target_str) {
            let target = parse_imm(target_str, line)?;
            let opcode = isa::i_type_opcode("movi").unwrap();
            let imm32 = target as u32;
            self.output.push(opcode);
            self.output.push(((rd & 0x1F) << 3) | 0);
            self.output.push((imm32 & 0xFF) as u8);
            self.output.push(((imm32 >> 8) & 0xFF) as u8);
            self.output.push(((imm32 >> 16) & 0xFF) as u8);
            self.output.push(((imm32 >> 24) & 0xFF) as u8);
            self.offset += 6;
        } else {
            // Label reference — emit placeholder and defer
            self.output.push(0x2A);
            self.output.push(((rd & 0x1F) << 3) | 0);
            self.output.push(0);
            self.output.push(0);
            self.output.push(0);
            self.output.push(0);
            self.pending.push(PendingRef {
                label: target_str.clone(),
                patch_offset: self.offset + 2,
                ref_type: PendingRefType::LaMovi,
            });
            self.offset += 6;
        }
        Ok(())
    }

    // ── Data directives ───────────────────────────────────────────────────

    fn emit_dot_word(&mut self, ops: &[String], line: usize) -> Result<(), AsmError> {
        let val = parse_imm(&ops[0], line)? as u64;
        self.output.push((val & 0xFF) as u8);
        self.output.push(((val >> 8) & 0xFF) as u8);
        self.output.push(((val >> 16) & 0xFF) as u8);
        self.output.push(((val >> 24) & 0xFF) as u8);
        self.output.push(((val >> 32) & 0xFF) as u8);
        self.output.push(((val >> 40) & 0xFF) as u8);
        self.output.push(((val >> 48) & 0xFF) as u8);
        self.output.push(((val >> 56) & 0xFF) as u8);
        self.offset += 8;
        Ok(())
    }

    fn emit_dot_byte(&mut self, ops: &[String], line: usize) -> Result<(), AsmError> {
        for op in ops {
            let val = parse_imm(op, line)?;
            self.output.push((val & 0xFF) as u8);
            self.offset += 1;
        }
        Ok(())
    }

    fn emit_dot_ascii(&mut self, ops: &[String], _line: usize) -> Result<(), AsmError> {
        for op in ops {
            self.output.extend(op.as_bytes());
            self.offset += op.len();
        }
        Ok(())
    }
}

// ── Helper functions ────────────────────────────────────────────────────────

fn parse_reg(s: &str, line: usize) -> Result<u8, AsmError> {
    let s = s.trim().to_lowercase();
    if s.starts_with('r') {
        let num: u8 = s[1..]
            .parse()
            .map_err(|_| AsmError::InvalidOperand {
                line,
                msg: format!("invalid register: {s}"),
            })?;
        if num > 31 {
            return Err(AsmError::InvalidOperand {
                line,
                msg: format!("register out of range: {s}"),
            });
        }
        Ok(num)
    } else {
        Err(AsmError::InvalidOperand {
            line,
            msg: format!("expected register, got {s}"),
        })
    }
}

fn parse_reg4(s: &str, line: usize, _ctx: &str) -> Result<u8, AsmError> {
    let r = parse_reg(s, line)?;
    if r > 15 {
        return Err(AsmError::RegisterOutOfRange {
            reg: s.to_string(),
            line,
        });
    }
    Ok(r)
}

fn parse_vreg(s: &str, line: usize) -> Result<u8, AsmError> {
    let s = s.trim().to_lowercase();
    if s.starts_with('v') {
        let num: u8 = s[1..]
            .parse()
            .map_err(|_| AsmError::InvalidOperand {
                line,
                msg: format!("invalid vector register: {s}"),
            })?;
        if num > 31 {
            return Err(AsmError::InvalidOperand {
                line,
                msg: format!("vector register out of range: {s}"),
            });
        }
        Ok(num)
    } else {
        Err(AsmError::InvalidOperand {
            line,
            msg: format!("expected vector register, got {s}"),
        })
    }
}

fn parse_freg(s: &str, line: usize) -> Result<u8, AsmError> {
    let s = s.trim().to_lowercase();
    if s.starts_with('f') {
        let num: u8 = s[1..]
            .parse()
            .map_err(|_| AsmError::InvalidOperand {
                line,
                msg: format!("invalid float register: {s}"),
            })?;
        if num > 31 {
            return Err(AsmError::InvalidOperand {
                line,
                msg: format!("float register out of range: {s}"),
            });
        }
        Ok(num)
    } else {
        Err(AsmError::InvalidOperand {
            line,
            msg: format!("expected float register, got {s}"),
        })
    }
}

fn parse_imm(s: &str, line: usize) -> Result<i64, AsmError> {
    let s = s.trim();
    if s.starts_with("0x") || s.starts_with("-0x") {
        // Handle negative hex
        let (neg, num_str) = if s.starts_with('-') {
            (true, &s[3..])
        } else {
            (false, &s[2..])
        };
        let val = i64::from_str_radix(num_str, 16).map_err(|_| AsmError::InvalidOperand {
            line,
            msg: format!("invalid hex number: {s}"),
        })?;
        Ok(if neg { -val } else { val })
    } else {
        s.parse::<i64>().map_err(|_| AsmError::InvalidOperand {
            line,
            msg: format!("invalid number: {s}"),
        })
    }
}

fn sext(val: i64, bits: u32) -> i64 {
    let sign_bit = 1i64 << (bits - 1);
    let mask = (1i64 << bits) - 1;
    let val = val & mask;
    if val & sign_bit != 0 {
        val | ((-1i64 << bits) & 0xFFFF_FFFF_FFFF_FFFFu64 as i64)
    } else {
        val
    }
}

fn is_label_ref(s: &str) -> bool {
    s.starts_with('.') || s.starts_with('@')
}

fn is_numeric(s: &str) -> bool {
    let s = s.trim();
    if s.starts_with("0x") || s.starts_with("-0x") {
        return true;
    }
    if let Some(stripped) = s.strip_prefix('-') {
        return stripped.chars().all(|c| c.is_ascii_digit());
    }
    s.chars().all(|c| c.is_ascii_digit())
}

fn parse_mem_operand(
    op: &str,
    line: usize,
) -> Result<(u8, i64, u8, i64), AsmError> {
    // Strip brackets
    let inner = op.trim().trim_start_matches('[').trim_end_matches(']').trim();
    let mut base_reg: u8 = 0;
    let mut offset: i64 = 0;
    let mut index_reg: u8 = 0;
    let mut scale: i64 = 0;

    // Split by + and - while preserving signs
    let parts: Vec<&str> = inner.split(&['+', '-']).collect();
    let signs: Vec<char> = inner
        .chars()
        .filter(|c| *c == '+' || *c == '-')
        .collect();

    if parts.is_empty() || parts[0].is_empty() {
        return Err(AsmError::InvalidOperand {
            line,
            msg: format!("invalid memory operand: {op}"),
        });
    }

    // First part: sign is implicit positive
    let mut sign: i64 = 1;

    for (i, part) in parts.iter().enumerate() {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if part.contains('*') {
            let mut sp = part.split('*');
            let reg_part = sp.next().unwrap().trim();
            let scale_part = sp.next().unwrap().trim();
            index_reg = parse_reg(reg_part, line)?;
            scale = scale_part.parse::<i64>().map_err(|_| AsmError::InvalidOperand {
                line,
                msg: format!("invalid scale: {scale_part}"),
            })?;
        } else if part.starts_with('r') || part.starts_with('R') {
            if base_reg == 0 {
                base_reg = parse_reg(part, line)?;
            } else {
                index_reg = parse_reg(part, line)?;
            }
        } else {
            offset = sign * parse_imm(part, line)?;
        }

        if i < signs.len() {
            sign = if signs[i] == '-' { -1 } else { 1 };
        }
    }

    Ok((base_reg, offset, index_reg, scale))
}