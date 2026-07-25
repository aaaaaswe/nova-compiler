//! MacroCore-X ISA instruction encoding definitions.
//!
//! Opcode values and instruction type categories matching the Python reference assembler.

// ── R-type (4 bytes) ────────────────────────────────────────────────────────
// byte0 = opcode, byte1 = (Rd<<3)|(Rs1>>2), byte2 = ((Rs1&3)<<6)|(Rs2<<1)|X,
// byte3 = 0x00

/// Get the R-type opcode for a mnemonic, or None.
pub fn r_type_opcode(m: &str) -> Option<u8> {
    match m {
        "add" => Some(0x00), "sub" => Some(0x01), "mul" => Some(0x02),
        "div" => Some(0x03), "divu" => Some(0x04), "and" => Some(0x05),
        "or" => Some(0x06), "xor" => Some(0x07), "shl" => Some(0x08),
        "shr" => Some(0x09), "sar" => Some(0x0A), "eq" => Some(0x0B),
        "lt" => Some(0x0C), "ltu" => Some(0x0D), "max" => Some(0x0E),
        "min" => Some(0x0F), "ror" => Some(0x10), "rol" => Some(0x11),
        "clz" => Some(0x12),
        _ => None,
    }
}

/// Check if a mnemonic is an R-type instruction.
pub fn is_r_type(m: &str) -> bool {
    r_type_opcode(m).is_some()
}

// ── I-type (4/6 bytes) ──────────────────────────────────────────────────────
// 4-byte: byte0 = opcode, byte1 = (Rd<<3)|(Rs1>>2), byte2 = ((Rs1&3)<<6)|(imm>>8),
//         byte3 = imm&0xFF
// 6-byte movi: byte0 = 0x2A, byte1 = (Rd<<3)|0, byte2-5 = imm32 LE

pub fn i_type_opcode(m: &str) -> Option<u8> {
    match m {
        "addi" => Some(0x20), "subi" => Some(0x21), "muli" => Some(0x22),
        "andi" => Some(0x23), "ori" => Some(0x24), "xori" => Some(0x25),
        "shli" => Some(0x26), "shri" => Some(0x27), "sari" => Some(0x28),
        "mov" => Some(0x29), "movi" => Some(0x2A),
        _ => None,
    }
}

pub fn is_i_type(m: &str) -> bool {
    i_type_opcode(m).is_some()
}

// ── L-type 4-byte ───────────────────────────────────────────────────────────
// byte0 = opcode, byte1 = (Rd<<4)|(Rs1&0xF), byte2-3 = off16 LE

pub fn l4_type_opcode(m: &str) -> Option<u8> {
    match m {
        "ld" => Some(0x40), "ldu" => Some(0x41), "lds" => Some(0x42),
        "st" => Some(0x43), "stw" => Some(0x44), "stb" => Some(0x45),
        "lda" => Some(0x46),
        _ => None,
    }
}

pub fn is_l4_type(m: &str) -> bool {
    l4_type_opcode(m).is_some()
}

// ── L-type 6-byte (indexed) ─────────────────────────────────────────────────

pub fn l6_type_opcode(m: &str) -> Option<u8> {
    match m {
        "ldr" => Some(0x50), "str" => Some(0x51),
        _ => None,
    }
}

pub fn is_l6_type(m: &str) -> bool {
    l6_type_opcode(m).is_some()
}

// ── B-type (4 bytes) ────────────────────────────────────────────────────────

pub fn b_type_opcode(m: &str) -> Option<u8> {
    match m {
        "j" => Some(0x60), "call" => Some(0x61), "ret" => Some(0x62),
        "beq" => Some(0x63), "bne" => Some(0x64), "blt" => Some(0x65),
        "ble" => Some(0x66), "bgt" => Some(0x67), "bge" => Some(0x68),
        "bltu" => Some(0x69), "bgeu" => Some(0x6A), "jreg" => Some(0x6B),
        "callreg" => Some(0x6C),
        _ => None,
    }
}

pub fn is_b_type(m: &str) -> bool {
    b_type_opcode(m).is_some()
}

// ── V-type (6/8 bytes) ──────────────────────────────────────────────────────

pub fn v_type_funct(m: &str) -> Option<u8> {
    match m {
        "vadd" => Some(0x00), "vsub" => Some(0x01), "vmul" => Some(0x02),
        "vand" => Some(0x03), "vor" => Some(0x04), "vxor" => Some(0x05),
        "vld" => Some(0x06), "vst" => Some(0x07), "vshl" => Some(0x08),
        "vshr" => Some(0x09), "vshuffle" => Some(0x0A), "vfmadd" => Some(0x0B),
        _ => None,
    }
}

pub fn is_v_type(m: &str) -> bool {
    v_type_funct(m).is_some()
}

// ── F-type (6 bytes) ────────────────────────────────────────────────────────

pub fn f_type_funct(m: &str) -> Option<u8> {
    match m {
        "fadd" => Some(0x00), "fsub" => Some(0x01), "fmul" => Some(0x02),
        "fdiv" => Some(0x03), "fsqrt" => Some(0x04), "fcmp" => Some(0x05),
        "fcvt.w.s" => Some(0x06), "fcvt.s.w" => Some(0x07),
        "fmin" => Some(0x08), "fmax" => Some(0x09), "fneg" => Some(0x0A),
        "fabs" => Some(0x0B), "fld" => Some(0x0C), "fst" => Some(0x0D),
        _ => None,
    }
}

pub fn is_f_type(m: &str) -> bool {
    f_type_funct(m).is_some()
}

// ── C-type (6 bytes) ────────────────────────────────────────────────────────

pub fn c_type_opcode(m: &str) -> Option<u8> {
    match m {
        "addm" => Some(0x90), "subm" => Some(0x91), "xchg" => Some(0x92),
        "cmpxchg" => Some(0x93), "push" => Some(0x94), "pop" => Some(0x95),
        "enter" => Some(0x96), "leave" => Some(0x97),
        _ => None,
    }
}

pub fn is_c_type(m: &str) -> bool {
    c_type_opcode(m).is_some()
}

// ── System 2-byte ───────────────────────────────────────────────────────────

pub fn sys2_opcode(m: &str) -> Option<u8> {
    match m {
        "syscall" => Some(0xB0), "sysret" => Some(0xB1), "int" => Some(0xB2),
        "iret" => Some(0xB3), "cpuid" => Some(0xB6), "hlt" => Some(0xB7),
        "cli" => Some(0xB8), "sti" => Some(0xB9), "nop" => Some(0xBA),
        "ecall" => Some(0xBB), "fence" => Some(0xBC), "bkpt" => Some(0xBD),
        _ => None,
    }
}

pub fn is_sys2(m: &str) -> bool {
    sys2_opcode(m).is_some()
}

// ── System 4-byte ───────────────────────────────────────────────────────────

pub fn sys4_opcode(m: &str) -> Option<u8> {
    match m {
        "rdmsr" => Some(0xB4), "wrmsr" => Some(0xB5),
        _ => None,
    }
}

pub fn is_sys4(m: &str) -> bool {
    sys4_opcode(m).is_some()
}

// ── Pseudo-instructions ─────────────────────────────────────────────────────

pub fn is_pseudo(m: &str) -> bool {
    matches!(m, "li" | "la")
}

// ── Directives ──────────────────────────────────────────────────────────────

pub fn is_directive(m: &str) -> bool {
    matches!(m, ".word" | ".byte" | ".half" | ".quad" | ".space" | ".align"
        | ".data" | ".text" | ".section" | ".ascii" | ".asciiz")
}

// ── Helper functions ────────────────────────────────────────────────────────

/// Check if a mnemonic is a known instruction type or directive.
pub fn is_known_mnemonic(m: &str) -> bool {
    is_r_type(m) || is_i_type(m) || is_l4_type(m) || is_l6_type(m)
        || is_b_type(m) || is_v_type(m) || is_f_type(m) || is_c_type(m)
        || is_sys2(m) || is_sys4(m) || is_pseudo(m) || is_directive(m)
}

/// Get the primary opcode byte for any instruction type that has one.
/// Returns None for V-type and F-type (which use funct fields).
pub fn get_opcode(m: &str) -> Option<u8> {
    r_type_opcode(m)
        .or_else(|| i_type_opcode(m))
        .or_else(|| l4_type_opcode(m))
        .or_else(|| l6_type_opcode(m))
        .or_else(|| b_type_opcode(m))
        .or_else(|| c_type_opcode(m))
        .or_else(|| sys2_opcode(m))
        .or_else(|| sys4_opcode(m))
}

/// Instruction category enum used for dispatch in the assembler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstCategory {
    RType,
    IType,
    L4Type,
    L6Type,
    BType,
    VType,
    FType,
    CType,
    Sys2,
    Sys4,
    Pseudo,
    Directive,
}

/// Classify a mnemonic into its instruction category.
pub fn classify(mnemonic: &str) -> InstCategory {
    if is_r_type(mnemonic) { return InstCategory::RType; }
    if is_i_type(mnemonic) { return InstCategory::IType; }
    if is_l4_type(mnemonic) { return InstCategory::L4Type; }
    if is_l6_type(mnemonic) { return InstCategory::L6Type; }
    if is_b_type(mnemonic) { return InstCategory::BType; }
    if is_v_type(mnemonic) { return InstCategory::VType; }
    if is_f_type(mnemonic) { return InstCategory::FType; }
    if is_c_type(mnemonic) { return InstCategory::CType; }
    if is_sys2(mnemonic) { return InstCategory::Sys2; }
    if is_sys4(mnemonic) { return InstCategory::Sys4; }
    if is_pseudo(mnemonic) { return InstCategory::Pseudo; }
    if is_directive(mnemonic) { return InstCategory::Directive; }
    InstCategory::Directive
}