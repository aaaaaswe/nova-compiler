use std::fmt;

/// R-type mnemonic reverse map
pub const R_MNEMONICS: [(u8, &str); 19] = [
    (0x00, "add"), (0x01, "sub"), (0x02, "mul"), (0x03, "div"), (0x04, "divu"),
    (0x05, "and"), (0x06, "or"), (0x07, "xor"), (0x08, "shl"), (0x09, "shr"),
    (0x0A, "sar"), (0x0B, "eq"), (0x0C, "lt"), (0x0D, "ltu"), (0x0E, "max"),
    (0x0F, "min"), (0x10, "ror"), (0x11, "rol"), (0x12, "clz"),
];

/// I-type mnemonic reverse map
pub const I_MNEMONICS: [(u8, &str); 11] = [
    (0x20, "addi"), (0x21, "subi"), (0x22, "muli"), (0x23, "andi"), (0x24, "ori"),
    (0x25, "xori"), (0x26, "shli"), (0x27, "shri"), (0x28, "sari"), (0x29, "mov"),
    (0x2A, "movi"),
];

/// L-type 4-byte mnemonic reverse map
pub const L4_MNEMONICS: [(u8, &str); 7] = [
    (0x40, "ld"), (0x41, "ldu"), (0x42, "lds"), (0x43, "st"), (0x44, "stw"),
    (0x45, "stb"), (0x46, "lda"),
];

/// L-type 6-byte mnemonic reverse map
pub const L6_MNEMONICS: [(u8, &str); 2] = [
    (0x50, "ldr"), (0x51, "str"),
];

/// B-type mnemonic reverse map
pub const B_MNEMONICS: [(u8, &str); 13] = [
    (0x60, "j"), (0x61, "call"), (0x62, "ret"), (0x63, "beq"), (0x64, "bne"),
    (0x65, "blt"), (0x66, "ble"), (0x67, "bgt"), (0x68, "bge"), (0x69, "bltu"),
    (0x6A, "bgeu"), (0x6B, "jreg"), (0x6C, "callreg"),
];

/// V-type function mnemonic reverse map
pub const V_MNEMONICS: [(u8, &str); 12] = [
    (0x00, "vadd"), (0x01, "vsub"), (0x02, "vmul"), (0x03, "vand"), (0x04, "vor"),
    (0x05, "vxor"), (0x06, "vld"), (0x07, "vst"), (0x08, "vshl"), (0x09, "vshr"),
    (0x0A, "vshuffle"), (0x0B, "vfmadd"),
];

/// F-type function mnemonic reverse map
pub const F_MNEMONICS: [(u8, &str); 14] = [
    (0x00, "fadd"), (0x01, "fsub"), (0x02, "fmul"), (0x03, "fdiv"),
    (0x04, "fsqrt"), (0x05, "fcmp"), (0x06, "fcvt.w.s"), (0x07, "fcvt.s.w"),
    (0x08, "fmin"), (0x09, "fmax"), (0x0A, "fneg"), (0x0B, "fabs"),
    (0x0C, "fld"), (0x0D, "fst"),
];

/// C-type mnemonic reverse map
pub const C_MNEMONICS: [(u8, &str); 8] = [
    (0x90, "addm"), (0x91, "subm"), (0x92, "xchg"), (0x93, "cmpxchg"),
    (0x94, "push"), (0x95, "pop"), (0x96, "enter"), (0x97, "leave"),
];

/// System 2-byte mnemonic reverse map
pub const SYS2_MNEMONICS: [(u8, &str); 12] = [
    (0xB0, "syscall"), (0xB1, "sysret"), (0xB2, "int"), (0xB3, "iret"),
    (0xB6, "cpuid"), (0xB7, "hlt"), (0xB8, "cli"), (0xB9, "sti"),
    (0xBA, "nop"), (0xBB, "ecall"), (0xBC, "fence"), (0xBD, "bkpt"),
];

/// System 4-byte mnemonic reverse map
pub const SYS4_MNEMONICS: [(u8, &str); 2] = [
    (0xB4, "rdmsr"), (0xB5, "wrmsr"),
];

/// Decoded instruction representation
#[derive(Debug, Clone)]
pub enum DecodedInst {
    RType { opcode: u8, rd: u8, rs1: u8, rs2: u8 },
    IType { opcode: u8, rd: u8, rs1: u8, imm: i64 },
    Movi { rd: u8, imm: u32 },
    LType4 { opcode: u8, rd: u8, rs1: u8, off: i16 },
    LType6 { opcode: u8, rd: u8, rs1: u8, off: i16, rn: u8, scale: u32 },
    BTypeJ { opcode: u8, target: u64 },
    BTypeCall { opcode: u8, target: u64 },
    BTypeRet,
    BTypeJreg { rs1: u8 },
    BTypeCallreg { rs1: u8 },
    BTypeCond { opcode: u8, rs1: u8, rs2: u8, target: u64 },
    VType { vd: u8, vs1: u8, vs2: u8, funct: u8, aux: u8, ext: u16 },
    FType { fd: u8, fs1: u8, fs2: u8, funct: u8, aux: u8, off: Option<i16> },
    CType { opcode: u8, rd: u8, rs1: u8, rs2: u8, off: Option<i16>, base: Option<u8> },
    Sys2 { opcode: u8, imm8: u8 },
    Sys4 { opcode: u8, rs1: u8, imm12: u16 },
}

impl fmt::Display for DecodedInst {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodedInst::RType { opcode, rd, rs1, rs2 } => {
                let mnem = lookup_r(*opcode);
                if *opcode == 0x12 {
                    write!(f, "{} r{}, r{}", mnem, rd, rs1)
                } else {
                    write!(f, "{} r{}, r{}, r{}", mnem, rd, rs1, rs2)
                }
            }
            DecodedInst::IType { opcode, rd, rs1, imm } => {
                let mnem = lookup_i(*opcode);
                if mnem == "shli" || mnem == "shri" || mnem == "sari" {
                    write!(f, "{} r{}, r{}, {}", mnem, rd, rs1, *imm & 0x3F)
                } else if mnem == "mov" {
                    write!(f, "{} r{}, {}", mnem, rd, imm)
                } else {
                    write!(f, "{} r{}, r{}, {}", mnem, rd, rs1, imm)
                }
            }
            DecodedInst::Movi { rd, imm } => {
                write!(f, "movi r{}, 0x{:x}", rd, imm)
            }
            DecodedInst::LType4 { opcode, rd, rs1, off } => {
                let mnem = lookup_l4(*opcode);
                if *opcode == 0x46 {
                    write!(f, "{} r{}, r{}, ...", mnem, rd, rs1)
                } else if mnem.starts_with("st") {
                    write!(f, "{} r{}, [r{} + {}]", mnem, rd, rs1, off)
                } else {
                    write!(f, "{} r{}, [r{} + {}]", mnem, rd, rs1, off)
                }
            }
            DecodedInst::LType6 { opcode, rd, rs1, off, rn, scale } => {
                let mnem = lookup_l6(*opcode);
                if mnem == "str" {
                    write!(f, "{} r{}, [r{} + r{}*{} + {}]", mnem, rd, rs1, rn, scale, off)
                } else {
                    write!(f, "{} r{}, [r{} + r{}*{} + {}]", mnem, rd, rs1, rn, scale, off)
                }
            }
            DecodedInst::BTypeJ { opcode: _, target } => {
                write!(f, "j 0x{:x}", target)
            }
            DecodedInst::BTypeCall { opcode: _, target } => {
                write!(f, "call 0x{:x}", target)
            }
            DecodedInst::BTypeRet => write!(f, "ret"),
            DecodedInst::BTypeJreg { rs1 } => write!(f, "jreg r{}", rs1),
            DecodedInst::BTypeCallreg { rs1 } => write!(f, "callreg r{}", rs1),
            DecodedInst::BTypeCond { opcode, rs1, rs2, target } => {
                let mnem = lookup_b(*opcode);
                write!(f, "{} r{}, r{}, 0x{:x}", mnem, rs1, rs2, target)
            }
            DecodedInst::VType { vd, vs1, vs2, funct, aux, ext } => {
                let mnem = lookup_v(*funct);
                match mnem {
                    "vld" | "vst" => {
                        let off = sign_extend_64(*ext as i64, 16);
                        write!(f, "{} v{}, [r{} + {}]", mnem, vd, vs1, off)
                    }
                    "vshl" | "vshr" => {
                        write!(f, "{} v{}, v{}, {}", mnem, vd, vs1, vs2)
                    }
                    "vshuffle" => {
                        write!(f, "{} v{}, v{}, {}", mnem, vd, vs1, aux)
                    }
                    "vfmadd" => {
                        let vs3 = ext & 0xF;
                        write!(f, "{} v{}, v{}, v{}, v{}", mnem, vd, vs1, vs2, vs3)
                    }
                    _ => write!(f, "{} v{}, v{}, v{}", mnem, vd, vs1, vs2),
                }
            }
            DecodedInst::FType { fd, fs1, fs2, funct, aux, off } => {
                let mnem = lookup_f(*funct);
                let _prec = if (aux & 0x06) != 0 { "f64" } else { "f32" };
                match mnem {
                    "fsqrt" | "fneg" | "fabs" | "fcvt.w.s" | "fcvt.s.w" => {
                        write!(f, "{} f{}, f{}", mnem, fd, fs1)
                    }
                    "fcmp" => write!(f, "{} f{}, f{}", mnem, fs1, fs2),
                    "fld" => {
                        let o = off.unwrap_or(0);
                        write!(f, "{} f{}, [r{} + {}]", mnem, fd, fs1, o)
                    }
                    "fst" => {
                        let o = off.unwrap_or(0);
                        write!(f, "{} f{}, [r{} + {}]", mnem, fd, fs1, o)
                    }
                    _ => write!(f, "{} f{}, f{}, f{}", mnem, fd, fs1, fs2),
                }
            }
            DecodedInst::CType { opcode, rd: _, rs1, rs2, off, base } => {
                let mnem = lookup_c(*opcode);
                match mnem {
                    "addm" | "subm" | "xchg" => {
                        let o = off.unwrap_or(0);
                        write!(f, "{} r{}, [r{} + {}]", mnem, rs1, rs2, o)
                    }
                    "cmpxchg" => {
                        let o = off.unwrap_or(0);
                        let b = base.unwrap_or(0);
                        write!(f, "{} r{}, r{}, [r{} + {}]", mnem, rs1, rs2, b, o)
                    }
                    "push" | "pop" => write!(f, "{} r{}", mnem, rs1),
                    "enter" => {
                        let o = off.unwrap_or(0);
                        write!(f, "{} {}", mnem, o)
                    }
                    "leave" => write!(f, "leave"),
                    _ => write!(f, "{}", mnem),
                }
            }
            DecodedInst::Sys2 { opcode, imm8 } => {
                let mnem = lookup_sys2(*opcode);
                match mnem {
                    "syscall" | "int" | "ecall" | "bkpt" => {
                        write!(f, "{} {}", mnem, imm8)
                    }
                    "fence" => {
                        let pi = (imm8 >> 4) & 0xF;
                        let po = imm8 & 0xF;
                        write!(f, "{} 0x{:x}, 0x{:x}", mnem, pi, po)
                    }
                    _ => write!(f, "{}", mnem),
                }
            }
            DecodedInst::Sys4 { opcode, rs1, imm12 } => {
                let mnem = lookup_sys4(*opcode);
                write!(f, "{} r{}, {}", mnem, rs1, imm12)
            }
        }
    }
}

fn lookup_r(opcode: u8) -> &'static str {
    R_MNEMONICS.iter().find(|(o, _)| *o == opcode).map(|(_, n)| *n).unwrap_or("???")
}

fn lookup_i(opcode: u8) -> &'static str {
    I_MNEMONICS.iter().find(|(o, _)| *o == opcode).map(|(_, n)| *n).unwrap_or("???")
}

fn lookup_l4(opcode: u8) -> &'static str {
    L4_MNEMONICS.iter().find(|(o, _)| *o == opcode).map(|(_, n)| *n).unwrap_or("???")
}

fn lookup_l6(opcode: u8) -> &'static str {
    L6_MNEMONICS.iter().find(|(o, _)| *o == opcode).map(|(_, n)| *n).unwrap_or("???")
}

fn lookup_b(opcode: u8) -> &'static str {
    B_MNEMONICS.iter().find(|(o, _)| *o == opcode).map(|(_, n)| *n).unwrap_or("???")
}

fn lookup_v(funct: u8) -> &'static str {
    V_MNEMONICS.iter().find(|(o, _)| *o == funct).map(|(_, n)| *n).unwrap_or("???")
}

fn lookup_f(funct: u8) -> &'static str {
    F_MNEMONICS.iter().find(|(o, _)| *o == funct).map(|(_, n)| *n).unwrap_or("???")
}

fn lookup_c(opcode: u8) -> &'static str {
    C_MNEMONICS.iter().find(|(o, _)| *o == opcode).map(|(_, n)| *n).unwrap_or("???")
}

fn lookup_sys2(opcode: u8) -> &'static str {
    SYS2_MNEMONICS.iter().find(|(o, _)| *o == opcode).map(|(_, n)| *n).unwrap_or("???")
}

fn lookup_sys4(opcode: u8) -> &'static str {
    SYS4_MNEMONICS.iter().find(|(o, _)| *o == opcode).map(|(_, n)| *n).unwrap_or("???")
}

/// Sign-extend a value to 64 bits.
pub fn sign_extend_64(val: i64, bits: u32) -> i64 {
    if bits >= 64 {
        return val;
    }
    let mask = (1u64 << bits) - 1;
    let val = (val as u64) & mask;
    if val & (1u64 << (bits - 1)) != 0 {
        (val as i64).wrapping_sub(1i64 << bits)
    } else {
        val as i64
    }
}

/// Determine instruction length from opcode byte.
pub fn get_inst_length(mem: &[u8], pc: u64, opcode: u8) -> usize {
    if opcode <= 0x1F {
        return 4; // R-type (4 bytes)
    }
    if opcode == 0x2A {
        return 6; // movi (6-byte I-type)
    }
    if (0x20..=0x29).contains(&opcode) {
        return 4; // I-type
    }
    if (0x40..=0x46).contains(&opcode) {
        return 4; // L-type 4-byte
    }
    if (0x50..=0x51).contains(&opcode) {
        return 6; // L-type 6-byte
    }
    if (0x60..=0x6C).contains(&opcode) {
        return 4; // B-type
    }
    if (opcode & 0xF0) == 0x80 {
        // V-type: check funct for vfmadd (8 bytes)
        let idx = (pc + 2) as usize;
        if idx + 1 < mem.len() {
            let funct = mem[idx];
            if funct == 0x0B {
                return 8;
            }
        }
        return 6;
    }
    if (opcode & 0xF0) == 0xA0 {
        return 6; // F-type scalar FP
    }
    if (0x90..=0x97).contains(&opcode) {
        return 6; // C-type
    }
    if matches!(opcode, 0xB0 | 0xB1 | 0xB2 | 0xB3 | 0xB6 | 0xB7 | 0xB8 | 0xB9 | 0xBA | 0xBB | 0xBC | 0xBD) {
        return 2; // System 2-byte
    }
    if matches!(opcode, 0xB4 | 0xB5) {
        return 4; // System 4-byte
    }
    2 // default
}

/// Decode one instruction at the given PC. Returns (instruction, length).
pub fn decode_one(mem: &[u8], pc: u64) -> (DecodedInst, usize) {
    let p = pc as usize;
    let opcode = mem[p];
    let length = get_inst_length(mem, pc, opcode);

    let inst = if opcode <= 0x1F {
        // R-type: byte0=opcode, byte1=(Rd<<3)|(Rs1>>2), byte2=((Rs1&3)<<6)|(Rs2<<1)|X, byte3=0
        let byte1 = mem[p + 1];
        let byte2 = mem[p + 2];
        let rd = (byte1 >> 3) & 0x1F;
        let rs1 = ((byte1 & 0x7) << 2) | ((byte2 >> 6) & 0x3);
        let rs2 = (byte2 >> 1) & 0x1F;
        DecodedInst::RType { opcode, rd, rs1, rs2 }
    } else if (0x20..=0x29).contains(&opcode) {
        // I-type 4-byte
        let byte1 = mem[p + 1];
        let byte2 = mem[p + 2];
        let byte3 = mem[p + 3];
        let rd = (byte1 >> 3) & 0x1F;
        let rs1 = ((byte1 & 0x7) << 2) | ((byte2 >> 6) & 0x3);
        let imm = ((byte2 as u64 & 0x3F) << 8) | byte3 as u64;
        let imm = sign_extend_64(imm as i64, 14);
        DecodedInst::IType { opcode, rd, rs1, imm }
    } else if opcode == 0x2A {
        // movi (6-byte)
        let byte1 = mem[p + 1];
        let rd = (byte1 >> 3) & 0x1F;
        let imm = u32::from_le_bytes([mem[p + 2], mem[p + 3], mem[p + 4], mem[p + 5]]);
        DecodedInst::Movi { rd, imm }
    } else if (0x40..=0x46).contains(&opcode) {
        // L-type 4-byte: byte1=(rd<<4)|rs1, byte2-3=off16
        let byte1 = mem[p + 1];
        let rd = (byte1 >> 4) & 0xF;
        let rs1 = byte1 & 0xF;
        let off = i16::from_le_bytes([mem[p + 2], mem[p + 3]]);
        DecodedInst::LType4 { opcode, rd, rs1, off }
    } else if (0x50..=0x51).contains(&opcode) {
        // L-type 6-byte indexed
        let byte1 = mem[p + 1];
        let rd = (byte1 >> 4) & 0xF;
        let rs1 = byte1 & 0xF;
        let off = i16::from_le_bytes([mem[p + 2], mem[p + 3]]);
        let extra = u16::from_le_bytes([mem[p + 4], mem[p + 5]]);
        let rn = ((extra >> 2) & 0xF) as u8;
        let scale = 1u32 << (extra & 0x3);
        DecodedInst::LType6 { opcode, rd, rs1, off, rn, scale }
    } else if (0x60..=0x6C).contains(&opcode) {
        // B-type
        let byte1 = mem[p + 1];
        if opcode == 0x62 {
            DecodedInst::BTypeRet
        } else if opcode == 0x60 {
            let imm_hi = byte1 as u64;
            let imm_mid = mem[p + 2] as u64;
            let imm_lo = mem[p + 3] as u64;
            let imm20 = (imm_hi << 12) | ((imm_lo & 0xF) << 8) | imm_mid;
            let imm20 = sign_extend_64(imm20 as i64, 20);
            let target = (pc as i64).wrapping_add(imm20 << 2) as u64;
            DecodedInst::BTypeJ { opcode, target }
        } else if opcode == 0x61 {
            let imm_hi = byte1 as u64;
            let imm_mid = mem[p + 2] as u64;
            let imm_lo = mem[p + 3] as u64;
            let imm20 = (imm_hi << 12) | ((imm_lo & 0xF) << 8) | imm_mid;
            let imm20 = sign_extend_64(imm20 as i64, 20);
            let target = (pc as i64).wrapping_add(imm20 << 2) as u64;
            DecodedInst::BTypeCall { opcode, target }
        } else if opcode == 0x6B {
            let rs1 = (byte1 >> 4) & 0xF;
            DecodedInst::BTypeJreg { rs1 }
        } else if opcode == 0x6C {
            let rs1 = (byte1 >> 4) & 0xF;
            DecodedInst::BTypeCallreg { rs1 }
        } else {
            let rs1 = (byte1 >> 4) & 0xF;
            let rs2 = byte1 & 0xF;
            let imm_hi = mem[p + 2] as u64;
            let imm_lo = mem[p + 3] as u64;
            let imm12 = (imm_hi << 8) | imm_lo;
            let imm12 = sign_extend_64(imm12 as i64, 12);
            let target = (pc as i64).wrapping_add(imm12 << 2) as u64;
            DecodedInst::BTypeCond { opcode, rs1, rs2, target }
        }
    } else if (opcode & 0xF0) == 0x80 {
        // V-type
        let byte1 = mem[p + 1];
        let vd = opcode & 0xF;
        let vs1 = (byte1 >> 4) & 0xF;
        let vs2 = byte1 & 0xF;
        let funct = mem[p + 2];
        let aux = mem[p + 3];
        let ext = u16::from_le_bytes([mem[p + 4], mem[p + 5]]);
        DecodedInst::VType { vd, vs1, vs2, funct, aux, ext }
    } else if (opcode & 0xF0) == 0xA0 {
        // F-type scalar FP
        let byte1 = mem[p + 1];
        let fd = opcode & 0xF;
        let fs1 = (byte1 >> 4) & 0xF;
        let fs2 = byte1 & 0xF;
        let funct = mem[p + 2];
        let aux = mem[p + 3];
        let off = if funct == 0x0C || funct == 0x0D {
            Some(i16::from_le_bytes([mem[p + 4], mem[p + 5]]))
        } else {
            None
        };
        DecodedInst::FType { fd, fs1, fs2, funct, aux, off }
    } else if (0x90..=0x97).contains(&opcode) {
        // C-type
        let byte1 = mem[p + 1];
        let rd = opcode & 0xF;
        let rs1 = (byte1 >> 4) & 0xF;
        let rs2 = byte1 & 0xF;
        let off = if matches!(opcode, 0x90 | 0x91 | 0x92 | 0x93 | 0x96) {
            Some(i16::from_le_bytes([mem[p + 2], mem[p + 3]]))
        } else {
            None
        };
        let base = if opcode == 0x93 {
            let extra = u16::from_le_bytes([mem[p + 4], mem[p + 5]]);
            Some((extra & 0xF) as u8)
        } else {
            None
        };
        DecodedInst::CType { opcode, rd, rs1, rs2, off, base }
    } else if matches!(opcode, 0xB0 | 0xB1 | 0xB2 | 0xB3 | 0xB6 | 0xB7 | 0xB8 | 0xB9 | 0xBA | 0xBB | 0xBC | 0xBD) {
        let imm8 = mem[p + 1];
        DecodedInst::Sys2 { opcode, imm8 }
    } else if matches!(opcode, 0xB4 | 0xB5) {
        let byte1 = mem[p + 1];
        let rs1 = (byte1 >> 4) & 0xF;
        let imm_hi = (byte1 & 0xF) as u16;
        let imm_lo = mem[p + 2] as u16;
        let imm12 = (imm_hi << 8) | imm_lo;
        DecodedInst::Sys4 { opcode, rs1, imm12 }
    } else {
        // Unknown - default to 2 bytes, treat as unknown sys2
        DecodedInst::Sys2 { opcode: 0xBA, imm8: 0 }
    };

    (inst, length)
}