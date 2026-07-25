use crate::cpu::Cpu;
use crate::decode::DecodedInst;
use crate::error::SimError;

/// Execute one decoded instruction. Returns `Ok(true)` to continue execution,
/// `Ok(false)` to stop, or `Err` on error.
pub fn execute_one(cpu: &mut Cpu, inst: &DecodedInst, length: usize, pc: u64) -> Result<bool, SimError> {
    match inst {
        DecodedInst::RType { opcode, rd, rs1, rs2 } => {
            exec_rtype(cpu, *opcode, *rd, *rs1, *rs2, pc)?;
        }
        DecodedInst::IType { opcode, rd, rs1, imm } => {
            exec_itype(cpu, *opcode, *rd, *rs1, *imm);
        }
        DecodedInst::Movi { rd, imm } => {
            exec_movi(cpu, *rd, *imm);
        }
        DecodedInst::LType4 { opcode, rd, rs1, off } => {
            exec_ltype4(cpu, *opcode, *rd, *rs1, *off, pc)?;
        }
        DecodedInst::LType6 { opcode, rd, rs1, off, rn, scale } => {
            exec_ltype6(cpu, *opcode, *rd, *rs1, *off, *rn, *scale, pc)?;
        }
        DecodedInst::BTypeJ { opcode: _, target } => {
            cpu.pc = *target;
            cpu.r[0] = 0;
            cpu.steps += 1;
            return Ok(true);
        }
        DecodedInst::BTypeCall { opcode: _, target } => {
            cpu.r[31] = pc + 4;
            cpu.pc = *target;
            cpu.r[0] = 0;
            cpu.steps += 1;
            return Ok(true);
        }
        DecodedInst::BTypeRet => {
            cpu.pc = cpu.r[31];
            cpu.r[0] = 0;
            cpu.steps += 1;
            return Ok(true);
        }
        DecodedInst::BTypeJreg { rs1 } => {
            cpu.pc = cpu.r[*rs1 as usize];
            cpu.r[0] = 0;
            cpu.steps += 1;
            return Ok(true);
        }
        DecodedInst::BTypeCallreg { rs1 } => {
            cpu.r[31] = pc + 4;
            cpu.pc = cpu.r[*rs1 as usize];
            cpu.r[0] = 0;
            cpu.steps += 1;
            return Ok(true);
        }
        DecodedInst::BTypeCond { opcode, rs1, rs2, target } => {
            exec_btype_cond(cpu, *opcode, *rs1, *rs2, *target, pc, length)?;
            return Ok(true);
        }
        DecodedInst::VType { vd, vs1, vs2, funct, aux, ext } => {
            exec_vtype(cpu, *vd, *vs1, *vs2, *funct, *aux, *ext, pc)?;
        }
        DecodedInst::FType { fd, fs1, fs2, funct, aux, off } => {
            exec_ftype(cpu, *fd, *fs1, *fs2, *funct, *aux, *off, pc)?;
        }
        DecodedInst::CType { opcode, rd: _, rs1, rs2, off, base } => {
            exec_ctype(cpu, *opcode, *rs1, *rs2, *off, *base, pc)?;
        }
        DecodedInst::Sys2 { opcode, imm8 } => {
            return exec_sys2(cpu, *opcode, *imm8, length);
        }
        DecodedInst::Sys4 { opcode, rs1, imm12 } => {
            exec_sys4(cpu, *opcode, *rs1, *imm12);
        }
    }

    cpu.pc += length as u64;
    cpu.r[0] = 0;
    cpu.steps += 1;
    Ok(true)
}

fn exec_rtype(cpu: &mut Cpu, opcode: u8, rd: u8, rs1: u8, rs2: u8, pc: u64) -> Result<(), SimError> {
    let val1 = cpu.r[rs1 as usize];
    let val2 = cpu.r[rs2 as usize];

    match opcode {
        0x00 => { // add
            let result = val1.wrapping_add(val2);
            set_flags_arith(cpu, result, val1, val2, false);
            cpu.r[rd as usize] = result;
        }
        0x01 => { // sub
            let result = val1.wrapping_sub(val2);
            set_flags_arith(cpu, result, val1, val2, true);
            cpu.r[rd as usize] = result;
        }
        0x02 => { // mul
            let result = val1.wrapping_mul(val2);
            set_flags_arith(cpu, result, val1, val2, false);
            cpu.r[rd as usize] = result;
        }
        0x03 => { // div
            if val2 == 0 {
                return Err(SimError::DivisionByZero { pc });
            }
            let s1 = val1 as i64;
            let s2 = val2 as i64;
            let q = s1.abs().wrapping_div(s2.abs());
            let q = if (s1 < 0) != (s2 < 0) { -q } else { q };
            let result = q as u64;
            cpu.r[rd as usize] = result;
            cpu.flags.zf = result == 0;
            cpu.flags.sf = (result as i64) < 0;
        }
        0x04 => { // divu
            if val2 == 0 {
                return Err(SimError::DivisionByZero { pc });
            }
            let result = val1.wrapping_div(val2);
            cpu.r[rd as usize] = result;
            cpu.flags.zf = result == 0;
            cpu.flags.sf = (result as i64) < 0;
        }
        0x05 => { // and
            let result = val1 & val2;
            set_flags_logical(cpu, result);
            cpu.r[rd as usize] = result;
        }
        0x06 => { // or
            let result = val1 | val2;
            set_flags_logical(cpu, result);
            cpu.r[rd as usize] = result;
        }
        0x07 => { // xor
            let result = val1 ^ val2;
            set_flags_logical(cpu, result);
            cpu.r[rd as usize] = result;
        }
        0x08 => { // shl
            let shift = val2 & 0x3F;
            let result = val1.wrapping_shl(shift as u32);
            set_flags_logical(cpu, result);
            cpu.r[rd as usize] = result;
        }
        0x09 => { // shr
            let shift = val2 & 0x3F;
            let result = val1 >> shift;
            set_flags_logical(cpu, result);
            cpu.r[rd as usize] = result;
        }
        0x0A => { // sar
            let shift = val2 & 0x3F;
            let result = (val1 as i64 >> shift) as u64;
            set_flags_logical(cpu, result);
            cpu.r[rd as usize] = result;
        }
        0x0B => { // eq
            let result = if val1 == val2 { 1u64 } else { 0u64 };
            cpu.r[rd as usize] = result;
            cpu.flags.zf = result == 0;
        }
        0x0C => { // lt
            let s1 = val1 as i64;
            let s2 = val2 as i64;
            let result = if s1 < s2 { 1u64 } else { 0u64 };
            cpu.r[rd as usize] = result;
            cpu.flags.zf = result == 0;
        }
        0x0D => { // ltu
            let result = if val1 < val2 { 1u64 } else { 0u64 };
            cpu.r[rd as usize] = result;
            cpu.flags.zf = result == 0;
        }
        0x0E => { // max
            let s1 = val1 as i64;
            let s2 = val2 as i64;
            cpu.r[rd as usize] = if s1 > s2 { val1 } else { val2 };
        }
        0x0F => { // min
            let s1 = val1 as i64;
            let s2 = val2 as i64;
            cpu.r[rd as usize] = if s1 < s2 { val1 } else { val2 };
        }
        0x10 => { // ror
            let shift = (val2 & 0x3F) as u32;
            let result = val1.rotate_right(shift);
            set_flags_logical(cpu, result);
            cpu.r[rd as usize] = result;
        }
        0x11 => { // rol
            let shift = (val2 & 0x3F) as u32;
            let result = val1.rotate_left(shift);
            set_flags_logical(cpu, result);
            cpu.r[rd as usize] = result;
        }
        0x12 => { // clz
            let result = val1.leading_zeros() as u64;
            cpu.r[rd as usize] = result;
        }
        _ => return Err(SimError::IllegalInstruction { opcode, pc }),
    }
    Ok(())
}

fn exec_itype(cpu: &mut Cpu, opcode: u8, rd: u8, rs1: u8, imm: i64) {
    let val1 = cpu.r[rs1 as usize];

    match opcode {
        0x20 => { // addi
            let result = val1.wrapping_add(imm as u64);
            set_flags_arith(cpu, result, val1, imm as u64, false);
            cpu.r[rd as usize] = result;
        }
        0x21 => { // subi
            let result = val1.wrapping_sub(imm as u64);
            set_flags_arith(cpu, result, val1, imm as u64, true);
            cpu.r[rd as usize] = result;
        }
        0x22 => { // muli
            let result = val1.wrapping_mul(imm as u64);
            set_flags_arith(cpu, result, val1, imm as u64, false);
            cpu.r[rd as usize] = result;
        }
        0x23 => { // andi
            let result = val1 & (imm as u64 & 0x3FFF);
            set_flags_logical(cpu, result);
            cpu.r[rd as usize] = result;
        }
        0x24 => { // ori
            let result = val1 | (imm as u64 & 0x3FFF);
            set_flags_logical(cpu, result);
            cpu.r[rd as usize] = result;
        }
        0x25 => { // xori
            let result = val1 ^ (imm as u64 & 0x3FFF);
            set_flags_logical(cpu, result);
            cpu.r[rd as usize] = result;
        }
        0x26 => { // shli
            let result = val1.wrapping_shl((imm as u64 & 0x3F) as u32);
            set_flags_logical(cpu, result);
            cpu.r[rd as usize] = result;
        }
        0x27 => { // shri
            let result = val1 >> (imm as u64 & 0x3F);
            set_flags_logical(cpu, result);
            cpu.r[rd as usize] = result;
        }
        0x28 => { // sari
            let shift = (imm as u64 & 0x3F) as u32;
            let result = (val1 as i64 >> shift) as u64;
            set_flags_logical(cpu, result);
            cpu.r[rd as usize] = result;
        }
        0x29 => { // mov
            let result = imm as u64;
            cpu.r[rd as usize] = result;
            set_flags_logical(cpu, result);
        }
        _ => {}
    }
}

fn exec_movi(cpu: &mut Cpu, rd: u8, imm: u32) {
    cpu.r[rd as usize] = imm as u64;
    set_flags_logical(cpu, imm as u64);
}

fn exec_ltype4(cpu: &mut Cpu, opcode: u8, rd: u8, rs1: u8, off: i16, _pc: u64) -> Result<(), SimError> {
    let addr = cpu.r[rs1 as usize].wrapping_add(off as u64);

    match opcode {
        0x40 => { // ld
            cpu.r[rd as usize] = read_mem_u64(cpu, addr)?;
        }
        0x41 => { // ldu
            cpu.r[rd as usize] = read_mem_u32(cpu, addr)?;
        }
        0x42 => { // lds
            let val = read_mem_u32(cpu, addr)?;
            cpu.r[rd as usize] = (val as i32) as u64;
        }
        0x43 => { // st
            write_mem_u64(cpu, addr, cpu.r[rd as usize])?;
        }
        0x44 => { // stw
            write_mem_u32(cpu, addr, cpu.r[rd as usize] as u32)?;
        }
        0x45 => { // stb
            write_mem_u8(cpu, addr, (cpu.r[rd as usize] & 0xFF) as u8)?;
        }
        0x46 => { // lda
            let extra = off as u16; // reuse off field as extra
            let rs2 = ((extra >> 2) & 0xF) as usize;
            let scale_bits = extra & 0x3;
            let scale = 1u64 << scale_bits;
            cpu.r[rd as usize] = cpu.r[rs1 as usize].wrapping_add(cpu.r[rs2].wrapping_mul(scale));
        }
        _ => {}
    }
    Ok(())
}

fn exec_ltype6(cpu: &mut Cpu, opcode: u8, rd: u8, rs1: u8, off: i16, rn: u8, scale: u32, _pc: u64) -> Result<(), SimError> {
    let addr = cpu.r[rs1 as usize]
        .wrapping_add(cpu.r[rn as usize].wrapping_mul(scale as u64))
        .wrapping_add(off as u64);

    match opcode {
        0x50 => { // ldr
            cpu.r[rd as usize] = read_mem_u64(cpu, addr)?;
        }
        0x51 => { // str
            write_mem_u64(cpu, addr, cpu.r[rd as usize])?;
        }
        _ => {}
    }
    Ok(())
}

fn exec_btype_cond(cpu: &mut Cpu, opcode: u8, rs1: u8, rs2: u8, target: u64, pc: u64, length: usize) -> Result<(), SimError> {
    let v1 = cpu.r[rs1 as usize];
    let v2 = cpu.r[rs2 as usize];

    let taken = match opcode {
        0x63 => v1 == v2, // beq
        0x64 => v1 != v2, // bne
        0x65 => (v1 as i64) < (v2 as i64), // blt
        0x66 => (v1 as i64) <= (v2 as i64), // ble
        0x67 => (v1 as i64) > (v2 as i64), // bgt
        0x68 => (v1 as i64) >= (v2 as i64), // bge
        0x69 => v1 < v2, // bltu
        0x6A => v1 >= v2, // bgeu
        _ => return Err(SimError::IllegalInstruction { opcode, pc }),
    };

    if taken {
        cpu.pc = target;
    } else {
        cpu.pc = pc + length as u64;
    }
    cpu.r[0] = 0;
    cpu.steps += 1;
    Ok(())
}

fn exec_vtype(cpu: &mut Cpu, vd: u8, vs1: u8, vs2: u8, funct: u8, _aux: u8, ext: u16, _pc: u64) -> Result<(), SimError> {
    match funct {
        0x00 => { // vadd
            cpu.v[vd as usize] = cpu.v[vs1 as usize].wrapping_add(cpu.v[vs2 as usize]);
        }
        0x01 => { // vsub
            cpu.v[vd as usize] = cpu.v[vs1 as usize].wrapping_sub(cpu.v[vs2 as usize]);
        }
        0x02 => { // vmul
            cpu.v[vd as usize] = cpu.v[vs1 as usize].wrapping_mul(cpu.v[vs2 as usize]);
        }
        0x03 => { // vand
            cpu.v[vd as usize] = cpu.v[vs1 as usize] & cpu.v[vs2 as usize];
        }
        0x04 => { // vor
            cpu.v[vd as usize] = cpu.v[vs1 as usize] | cpu.v[vs2 as usize];
        }
        0x05 => { // vxor
            cpu.v[vd as usize] = cpu.v[vs1 as usize] ^ cpu.v[vs2 as usize];
        }
        0x06 => { // vld
            let off = super::decode::sign_extend_64(ext as i64, 16) as u64;
            let addr = cpu.r[vs1 as usize].wrapping_add(off);
            cpu.v[vd as usize] = read_mem_u64(cpu, addr)?;
        }
        0x07 => { // vst
            let off = super::decode::sign_extend_64(ext as i64, 16) as u64;
            let addr = cpu.r[vs1 as usize].wrapping_add(off);
            write_mem_u64(cpu, addr, cpu.v[vd as usize])?;
        }
        0x08 => { // vshl
            cpu.v[vd as usize] = cpu.v[vs1 as usize].wrapping_shl(vs2 as u32 & 0x1F);
        }
        0x09 => { // vshr
            cpu.v[vd as usize] = cpu.v[vs1 as usize] >> (vs2 as u32 & 0x1F);
        }
        0x0A => { // vshuffle
            cpu.v[vd as usize] = cpu.v[vs1 as usize]; // simplified
        }
        0x0B => { // vfmadd
            let vs3 = (ext & 0xF) as usize;
            cpu.v[vd as usize] = cpu.v[vs1 as usize]
                .wrapping_mul(cpu.v[vs2 as usize])
                .wrapping_add(cpu.v[vs3]);
        }
        _ => {}
    }
    Ok(())
}

fn exec_ftype(cpu: &mut Cpu, fd: u8, fs1: u8, fs2: u8, funct: u8, aux: u8, off: Option<i16>, _pc: u64) -> Result<(), SimError> {
    let is_f64 = (aux >> 1) & 0x3 == 1;

    match funct {
        0x00 | 0x01 | 0x02 | 0x03 | 0x08 | 0x09 => {
            // fadd, fsub, fmul, fdiv, fmin, fmax — two-source
            fp_execute_f(cpu, fd, fs1, fs2, funct, is_f64);
        }
        0x04 | 0x06 | 0x07 => {
            // fsqrt, fcvt.w.s, fcvt.s.w — one-source
            fp_execute_f(cpu, fd, fs1, 0, funct, is_f64);
        }
        0x05 => { // fcmp
            fp_compare(cpu, fs1, fs2, is_f64);
        }
        0x0A => { // fneg
            fp_unary(cpu, fd, fs1, is_f64, true);
        }
        0x0B => { // fabs
            fp_unary(cpu, fd, fs1, is_f64, false);
        }
        0x0C => { // fld
            let o = off.unwrap_or(0) as u64;
            let addr = cpu.r[fs1 as usize].wrapping_add(o);
            cpu.f[fd as usize] = f64::from_bits(read_mem_u64(cpu, addr)?);
        }
        0x0D => { // fst
            let o = off.unwrap_or(0) as u64;
            let addr = cpu.r[fs1 as usize].wrapping_add(o);
            write_mem_u64(cpu, addr, cpu.f[fd as usize].to_bits())?;
        }
        _ => {}
    }
    Ok(())
}

fn fp_execute_f(cpu: &mut Cpu, fd: u8, fs1: u8, fs2: u8, funct: u8, is_f64: bool) {
    if is_f64 {
        let a = cpu.f[fs1 as usize];
        let b = cpu.f[fs2 as usize];

        let result = match funct {
            0x00 => a + b,                           // fadd
            0x01 => a - b,                           // fsub
            0x02 => a * b,                           // fmul
            0x03 => { if b != 0.0 { a / b } else { f64::INFINITY } }, // fdiv
            0x04 => { if a >= 0.0 { a.sqrt() } else { f64::NAN } },   // fsqrt
            0x06 => (a as i64) as f64,               // fcvt.w.s
            0x07 => a as f64,                        // fcvt.s.w (actually int→float)
            0x08 => if a < b { a } else { b },       // fmin
            0x09 => if a > b { a } else { b },       // fmax
            _ => 0.0f64,
        };
        cpu.f[fd as usize] = result;
    } else {
        let a = f32::from_bits(cpu.f[fs1 as usize].to_bits() as u32 as u32);
        let b = f32::from_bits(cpu.f[fs2 as usize].to_bits() as u32 as u32);

        let result = match funct {
            0x00 => a + b,
            0x01 => a - b,
            0x02 => a * b,
            0x03 => { if b != 0.0 { a / b } else { f32::INFINITY } },
            0x04 => { if a >= 0.0 { a.sqrt() } else { f32::NAN } },
            0x06 => (a as i64) as f32,
            0x07 => a as f32,
            0x08 => if a < b { a } else { b },
            0x09 => if a > b { a } else { b },
            _ => 0.0f32,
        };
        cpu.f[fd as usize] = f64::from_bits(result.to_bits() as u64);
    }
}

fn fp_compare(cpu: &mut Cpu, fs1: u8, fs2: u8, is_f64: bool) {
    if is_f64 {
        let a = cpu.f[fs1 as usize];
        let b = cpu.f[fs2 as usize];
        cpu.flags.zf = a == b;
        cpu.flags.cf = a < b;
    } else {
        let a = f32::from_bits(cpu.f[fs1 as usize].to_bits() as u32);
        let b = f32::from_bits(cpu.f[fs2 as usize].to_bits() as u32);
        cpu.flags.zf = a == b;
        cpu.flags.cf = a < b;
    }
    cpu.flags.sf = false;
    cpu.flags.of = false;
}

fn fp_unary(cpu: &mut Cpu, fd: u8, fs1: u8, is_f64: bool, negate: bool) {
    if is_f64 {
        let a = cpu.f[fs1 as usize];
        cpu.f[fd as usize] = if negate { -a } else { a.abs() };
    } else {
        let a = f32::from_bits(cpu.f[fs1 as usize].to_bits() as u32);
        let result = if negate { -a } else { a.abs() };
        cpu.f[fd as usize] = f64::from_bits(result.to_bits() as u64);
    }
}

fn exec_ctype(cpu: &mut Cpu, opcode: u8, rs1: u8, rs2: u8, off: Option<i16>, base: Option<u8>, _pc: u64) -> Result<(), SimError> {
    match opcode {
        0x90 => { // addm
            let o = off.unwrap_or(0) as u64;
            let addr = cpu.r[rs2 as usize].wrapping_add(o);
            let val = read_mem_u64(cpu, addr)?;
            write_mem_u64(cpu, addr, val.wrapping_add(cpu.r[rs1 as usize]))?;
        }
        0x91 => { // subm
            let o = off.unwrap_or(0) as u64;
            let addr = cpu.r[rs2 as usize].wrapping_add(o);
            let val = read_mem_u64(cpu, addr)?;
            write_mem_u64(cpu, addr, val.wrapping_sub(cpu.r[rs1 as usize]))?;
        }
        0x92 => { // xchg
            let o = off.unwrap_or(0) as u64;
            let addr = cpu.r[rs2 as usize].wrapping_add(o);
            let val = read_mem_u64(cpu, addr)?;
            write_mem_u64(cpu, addr, cpu.r[rs1 as usize])?;
            cpu.r[rs1 as usize] = val;
        }
        0x93 => { // cmpxchg
            let o = off.unwrap_or(0) as u64;
            let b = base.unwrap_or(0) as usize;
            let addr = cpu.r[b].wrapping_add(o);
            let val = read_mem_u64(cpu, addr)?;
            if val == cpu.r[rs1 as usize] {
                write_mem_u64(cpu, addr, cpu.r[rs2 as usize])?;
            }
        }
        0x94 => { // push
            cpu.r[2] = cpu.r[2].wrapping_sub(8);
            write_mem_u64(cpu, cpu.r[2], cpu.r[rs1 as usize])?;
        }
        0x95 => { // pop
            cpu.r[rs1 as usize] = read_mem_u64(cpu, cpu.r[2])?;
            cpu.r[2] = cpu.r[2].wrapping_add(8);
        }
        0x96 => { // enter
            let imm = off.unwrap_or(0) as u64;
            cpu.r[2] = cpu.r[2].wrapping_sub(imm);
            write_mem_u64(cpu, cpu.r[2], cpu.r[30])?;
        }
        0x97 => { // leave
            cpu.r[2] = cpu.r[30].wrapping_add(8);
            cpu.r[30] = read_mem_u64(cpu, cpu.r[2].wrapping_sub(8))?;
        }
        _ => {}
    }
    Ok(())
}

fn exec_sys2(cpu: &mut Cpu, opcode: u8, imm8: u8, length: usize) -> Result<bool, SimError> {
    match opcode {
        0xB0 => { // syscall
            // simplified: no syscall handler
            cpu.pc += length as u64;
            cpu.r[0] = 0;
            cpu.steps += 1;
            return Ok(true);
        }
        0xB1 => { // sysret
            cpu.pc = cpu.err;
        }
        0xB2 => { // int
            // simplified: halt
            cpu.running = false;
            cpu.pc += length as u64;
            cpu.r[0] = 0;
            cpu.steps += 1;
            return Ok(false);
        }
        0xB3 => { // iret
            cpu.pc = cpu.err;
        }
        0xB6 => { // cpuid
            cpu.r[0] = 0x4D43584D; // "MCXM"
            cpu.r[1] = 0x00020000; // version 2.0
            cpu.r[2] = 0x00000000;
            cpu.r[3] = 0x00000001;
        }
        0xB7 => { // hlt
            cpu.running = false;
            cpu.pc += length as u64;
            cpu.r[0] = 0;
            cpu.steps += 1;
            return Ok(false);
        }
        0xB8 => { // cli
            // no-op
        }
        0xB9 => { // sti
            // no-op
        }
        0xBA => { // nop
            cpu.pc += length as u64;
            cpu.steps += 1;
        }
        0xBB => { // ecall
            println!("\n[ecall] exit code: {}", imm8);
            cpu.pc += length as u64;
            cpu.r[0] = 0;
            cpu.steps += 1;
            return Ok(false);
        }
        0xBC => { // fence
            // no-op
        }
        0xBD => { // bkpt
            println!("\n[bkpt] breakpoint {} at PC=0x{:x}", imm8, cpu.pc);
            cpu.pc += length as u64;
            cpu.r[0] = 0;
            cpu.steps += 1;
            return Ok(false);
        }
        _ => {}
    }
    Ok(true)
}

fn exec_sys4(cpu: &mut Cpu, opcode: u8, rs1: u8, imm12: u16) {
    match opcode {
        0xB4 => { // rdmsr
            cpu.r[rs1 as usize] = csr_read(cpu, imm12);
        }
        0xB5 => { // wrmsr
            csr_write(cpu, imm12, cpu.r[rs1 as usize]);
        }
        _ => {}
    }
}

fn csr_read(cpu: &Cpu, csr_num: u16) -> u64 {
    match csr_num {
        0x000 => cpu.err,
        0x001 => cpu.ef,
        0x002 => cpu.csr_mode,
        0x003 => cpu.csr_cr3,
        0x004 => cpu.csr_ivec,
        0x00A => 0, // simplified
        _ => 0,
    }
}

fn csr_write(cpu: &mut Cpu, csr_num: u16, value: u64) {
    match csr_num {
        0x000 => cpu.err = value,
        0x001 => cpu.ef = value & 0xFF,
        0x002 => {
            cpu.csr_mode = value;
            cpu.priv_mode = value & 1;
        }
        0x003 => {
            cpu.csr_cr3 = value & 0xFFFFFFFFFFF000;
            cpu.mmu_enabled = (cpu.csr_cr3 != 0) && (cpu.priv_mode == 1);
        }
        0x004 => cpu.csr_ivec = value & 0xFFFFFFFFFFF000,
        0x00A => {} // simplified
        _ => {}
    }
}

// ---- Flag helpers ----

pub fn set_flags_arith(cpu: &mut Cpu, result: u64, op1: u64, op2: u64, is_sub: bool) {
    cpu.flags.zf = result == 0;
    cpu.flags.sf = (result as i64) < 0;
    if is_sub {
        cpu.flags.cf = op1 < op2;
        cpu.flags.of = ((op1 ^ op2) & (op1 ^ result) & 0x8000000000000000) != 0;
    } else {
        cpu.flags.cf = op1.checked_add(op2).is_none();
        cpu.flags.of = (!(op1 ^ op2) & (op1 ^ result) & 0x8000000000000000) != 0;
    }
}

pub fn set_flags_logical(cpu: &mut Cpu, result: u64) {
    cpu.flags.zf = result == 0;
    cpu.flags.sf = (result as i64) < 0;
    cpu.flags.cf = false;
    cpu.flags.of = false;
}

// ---- Memory helpers ----

fn read_mem_u64(cpu: &Cpu, addr: u64) -> Result<u64, SimError> {
    let p = addr as usize;
    if p + 8 > cpu.memory.len() {
        return Err(SimError::MemoryOutOfBounds { addr });
    }
    let bytes: [u8; 8] = cpu.memory[p..p + 8].try_into().unwrap();
    Ok(u64::from_le_bytes(bytes))
}

fn read_mem_u32(cpu: &Cpu, addr: u64) -> Result<u64, SimError> {
    let p = addr as usize;
    if p + 4 > cpu.memory.len() {
        return Err(SimError::MemoryOutOfBounds { addr });
    }
    let bytes: [u8; 4] = cpu.memory[p..p + 4].try_into().unwrap();
    Ok(u32::from_le_bytes(bytes) as u64)
}

fn write_mem_u64(cpu: &mut Cpu, addr: u64, value: u64) -> Result<(), SimError> {
    let p = addr as usize;
    if p + 8 > cpu.memory.len() {
        return Err(SimError::MemoryOutOfBounds { addr });
    }
    cpu.memory[p..p + 8].copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_mem_u32(cpu: &mut Cpu, addr: u64, value: u32) -> Result<(), SimError> {
    let p = addr as usize;
    if p + 4 > cpu.memory.len() {
        return Err(SimError::MemoryOutOfBounds { addr });
    }
    cpu.memory[p..p + 4].copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_mem_u8(cpu: &mut Cpu, addr: u64, value: u8) -> Result<(), SimError> {
    let p = addr as usize;
    if p >= cpu.memory.len() {
        return Err(SimError::MemoryOutOfBounds { addr });
    }
    cpu.memory[p] = value;
    Ok(())
}