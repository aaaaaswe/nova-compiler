//! NIR (Nova Intermediate Representation) core data structures.
//!
//! Defines the complete IR for the MacroCore-X compiler:
//! - Values (virtual registers, constants, globals, parameters)
//! - Instructions (126 opcodes across 13 categories)
//! - Basic blocks, functions, and modules.

use crate::types::IrType;
use crate::types::Value;
use std::fmt;

// =============================================================================
//  AddrExpr
// =============================================================================

/// Memory address expression: `[base + index*scale + offset]`.
#[derive(Clone, Debug)]
pub struct AddrExpr {
    /// Base register or global variable.
    pub base: Value,
    /// Optional index register (scaled).
    pub index: Option<Value>,
    /// Scale factor (must be 1, 2, 4, or 8).
    pub scale: i32,
    /// Constant byte offset.
    pub offset: i64,
}

impl AddrExpr {
    /// Create a new AddrExpr, validating the scale factor.
    pub fn new(base: Value, index: Option<Value>, scale: i32, offset: i64) -> Result<Self, crate::types::NirError> {
        if ![1, 2, 4, 8].contains(&scale) {
            return Err(crate::types::NirError::InvalidScale(scale));
        }
        Ok(AddrExpr {
            base,
            index,
            scale,
            offset,
        })
    }
}

impl fmt::Display for AddrExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("{}", self.base);
        if let Some(ref index) = self.index {
            s = format!("{s} + {index}*{}", self.scale);
        }
        if self.offset != 0 {
            if self.offset < 0 {
                s = format!("{s} - {}", -self.offset);
            } else {
                s = format!("{s} + {}", self.offset);
            }
        }
        write!(f, "[{s}]")
    }
}

// =============================================================================
//  Instruction
// =============================================================================

/// All NIR instructions (126 opcodes across 13 categories).
#[derive(Clone, Debug)]
pub enum Instruction {
    // -------------------------------------------------------------------------
    //  Category 1: Arithmetic (flag-producing) – 18 opcodes
    // -------------------------------------------------------------------------

    /// Integer addition.
    Add {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Integer subtraction.
    Sub {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Integer multiplication.
    Mul {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Integer multiplication (high half).
    Mulh {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Signed integer division.
    Div {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Unsigned integer division.
    Divu {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Signed integer remainder.
    Rem {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Unsigned integer remainder.
    Remu {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Bitwise AND.
    And {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Bitwise OR.
    Or {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Bitwise XOR.
    Xor {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Logical shift left.
    Shl {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Logical shift right.
    Shr {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Arithmetic shift right.
    Sar {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Rotate left.
    Rotl {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Rotate right.
    Rotr {
        result: Value,
        lhs: Value,
        rhs: Value,
        flags_result: Value,
    },
    /// Arithmetic negation.
    Neg {
        result: Value,
        operand: Value,
        flags_result: Value,
    },
    /// Bitwise NOT.
    Not {
        result: Value,
        operand: Value,
        flags_result: Value,
    },

    // -------------------------------------------------------------------------
    //  Category 2: Immediate – 13 opcodes
    // -------------------------------------------------------------------------

    /// Add immediate.
    Addi {
        result: Value,
        lhs: Value,
        imm: i64,
        flags_result: Option<Value>,
    },
    /// Subtract immediate.
    Subi {
        result: Value,
        lhs: Value,
        imm: i64,
        flags_result: Option<Value>,
    },
    /// Multiply immediate.
    Muli {
        result: Value,
        lhs: Value,
        imm: i64,
        flags_result: Option<Value>,
    },
    /// Bitwise AND immediate.
    Andi {
        result: Value,
        lhs: Value,
        imm: i64,
        flags_result: Option<Value>,
    },
    /// Bitwise OR immediate.
    Ori {
        result: Value,
        lhs: Value,
        imm: i64,
        flags_result: Option<Value>,
    },
    /// Bitwise XOR immediate.
    Xori {
        result: Value,
        lhs: Value,
        imm: i64,
        flags_result: Option<Value>,
    },
    /// Logical shift left immediate.
    Shli {
        result: Value,
        lhs: Value,
        imm: i64,
        flags_result: Option<Value>,
    },
    /// Logical shift right immediate.
    Shri {
        result: Value,
        lhs: Value,
        imm: i64,
        flags_result: Option<Value>,
    },
    /// Arithmetic shift right immediate.
    Sari {
        result: Value,
        lhs: Value,
        imm: i64,
        flags_result: Option<Value>,
    },
    /// Rotate left immediate.
    Rotli {
        result: Value,
        lhs: Value,
        imm: i64,
        flags_result: Option<Value>,
    },
    /// Rotate right immediate.
    Rotri {
        result: Value,
        lhs: Value,
        imm: i64,
        flags_result: Option<Value>,
    },
    /// Move immediate into a register.
    Movi {
        result: Value,
        imm: i64,
    },
    /// Move one register into another.
    Mov {
        result: Value,
        src: Value,
    },

    // -------------------------------------------------------------------------
    //  Category 3: Flag consumers – 13 opcodes
    // -------------------------------------------------------------------------

    /// Test equal (ZF=1).
    TestEq { result: Value, flags: Value },
    /// Test not equal (ZF=0).
    TestNe { result: Value, flags: Value },
    /// Test less than (signed, SF≠OF).
    TestLt { result: Value, flags: Value },
    /// Test less than or equal (signed, ZF=1 or SF≠OF).
    TestLe { result: Value, flags: Value },
    /// Test less than unsigned (CF=1).
    TestLtu { result: Value, flags: Value },
    /// Test less than or equal unsigned (CF=1 or ZF=1).
    TestLeu { result: Value, flags: Value },
    /// Test overflow flag.
    TestOf { result: Value, flags: Value },
    /// Test carry flag.
    TestCf { result: Value, flags: Value },
    /// Test sign flag.
    TestSf { result: Value, flags: Value },
    /// Test greater than or equal (signed, SF=OF).
    TestGe { result: Value, flags: Value },
    /// Test greater than (signed, ZF=0 and SF=OF).
    TestGt { result: Value, flags: Value },
    /// Test greater than or equal unsigned (CF=0).
    TestGeu { result: Value, flags: Value },
    /// Test greater than unsigned (CF=0 and ZF=0).
    TestGtu { result: Value, flags: Value },

    // -------------------------------------------------------------------------
    //  Category 4: Memory – 7 opcodes
    // -------------------------------------------------------------------------

    /// Load from memory at the given address expression.
    Load {
        result: Value,
        addr: AddrExpr,
    },
    /// Load from memory at base + offset.
    Loadi {
        result: Value,
        base: Value,
        offset: i64,
    },
    /// Load from memory and sign-extend.
    LoadSext {
        result: Value,
        addr: AddrExpr,
        from_type: IrType,
    },
    /// Load from memory and zero-extend.
    LoadZext {
        result: Value,
        addr: AddrExpr,
        from_type: IrType,
    },
    /// Store value to memory at the given address expression.
    Store {
        value: Value,
        addr: AddrExpr,
    },
    /// Store value to memory at base + offset.
    Storei {
        value: Value,
        base: Value,
        offset: i64,
    },
    /// Load effective address.
    Lea {
        result: Value,
        addr: AddrExpr,
    },

    // -------------------------------------------------------------------------
    //  Category 5: Composite memory – 6 opcodes
    // -------------------------------------------------------------------------

    /// Atomic-like memory add.
    MemAdd {
        addr: AddrExpr,
        value: Value,
    },
    /// Atomic-like memory subtract.
    MemSub {
        addr: AddrExpr,
        value: Value,
    },
    /// Atomic-like memory AND.
    MemAnd {
        addr: AddrExpr,
        value: Value,
    },
    /// Atomic-like memory OR.
    MemOr {
        addr: AddrExpr,
        value: Value,
    },
    /// Atomic-like memory XOR.
    MemXor {
        addr: AddrExpr,
        value: Value,
    },
    /// Exchange value in memory, returning the old value.
    MemXchg {
        result: Value,
        addr: AddrExpr,
        value: Value,
    },

    // -------------------------------------------------------------------------
    //  Category 6: Atomic – 3 opcodes
    // -------------------------------------------------------------------------

    /// Atomic memory add.
    AtomicMemAdd {
        addr: AddrExpr,
        value: Value,
    },
    /// Atomic exchange, returning the old value.
    AtomicMemXchg {
        result: Value,
        addr: AddrExpr,
        value: Value,
    },
    /// Atomic compare-and-swap.
    AtomicCas {
        result: Value,
        addr: AddrExpr,
        expected: Value,
        desired: Value,
    },

    // -------------------------------------------------------------------------
    //  Category 7: Stack – 4 opcodes
    // -------------------------------------------------------------------------

    /// Push a value onto the stack.
    Push { value: Value },
    /// Pop a value from the stack.
    Pop { result: Value },
    /// Set up a stack frame of the given size.
    Enter { frame_size: i64 },
    /// Tear down the current stack frame.
    Leave,

    // -------------------------------------------------------------------------
    //  Category 8: Control flow – 7 opcodes
    // -------------------------------------------------------------------------

    /// Unconditional branch to a basic block.
    Br { target_bb: String },
    /// Conditional branch.
    BrCond {
        cond: Value,
        true_bb: String,
        false_bb: String,
    },
    /// Multi-way branch via a jump table.
    Switch {
        value: Value,
        default_bb: String,
        /// (case_value, bb_name) pairs.
        cases: Vec<(Value, String)>,
    },
    /// Call a function by name.
    Call {
        result: Option<Value>,
        callee_name: String,
        args: Vec<Value>,
    },
    /// Call a function through a pointer.
    CallIndirect {
        result: Option<Value>,
        fnptr: Value,
        args: Vec<Value>,
    },
    /// Return from a function.
    Ret { value: Option<Value> },
    /// Tail-call a function.
    TailCall {
        callee_name: String,
        args: Vec<Value>,
    },

    // -------------------------------------------------------------------------
    //  Category 9: Float – 17 opcodes
    // -------------------------------------------------------------------------

    /// Floating-point addition.
    Fadd {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Floating-point subtraction.
    Fsub {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Floating-point multiplication.
    Fmul {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Floating-point division.
    Fdiv {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Floating-point negation.
    Fneg {
        result: Value,
        operand: Value,
    },
    /// Floating-point absolute value.
    Fabs {
        result: Value,
        operand: Value,
    },
    /// Floating-point square root.
    Fsqrt {
        result: Value,
        operand: Value,
    },
    /// Floating-point minimum.
    Fmin {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Floating-point maximum.
    Fmax {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Fused multiply-add: result = a * b + c.
    Ffma {
        result: Value,
        a: Value,
        b: Value,
        c: Value,
    },
    /// Floating-point comparison: equal.
    FcmpEq {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Floating-point comparison: not equal.
    FcmpNe {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Floating-point comparison: less than.
    FcmpLt {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Floating-point comparison: less than or equal.
    FcmpLe {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Floating-point comparison: greater than.
    FcmpGt {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Floating-point comparison: greater than or equal.
    FcmpGe {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Floating-point comparison: ordered (neither operand is NaN).
    FcmpOrd {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Floating-point comparison: unordered (either operand is NaN).
    FcmpUno {
        result: Value,
        lhs: Value,
        rhs: Value,
    },

    // -------------------------------------------------------------------------
    //  Category 10: Vector – 15 opcodes
    // -------------------------------------------------------------------------

    /// Vector addition.
    Vadd {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Vector subtraction.
    Vsub {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Vector multiplication.
    Vmul {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Vector division.
    Vdiv {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    /// Vector fused multiply-add.
    Vfma {
        result: Value,
        a: Value,
        b: Value,
        c: Value,
    },
    /// Vector shuffle/permute.
    Vshuffle {
        result: Value,
        lhs: Value,
        rhs: Value,
        mask: Value,
    },
    /// Broadcast a scalar to all lanes of a vector.
    Vbroadcast {
        result: Value,
        value: Value,
    },
    /// Extract a single element from a vector.
    Vextract {
        result: Value,
        vector: Value,
        index: usize,
    },
    /// Insert a scalar into a vector at a given lane.
    Vinsert {
        result: Value,
        vector: Value,
        value: Value,
        index: usize,
    },
    /// Vector reduction: sum.
    VreduceAdd {
        result: Value,
        vector: Value,
    },
    /// Vector reduction: minimum.
    VreduceMin {
        result: Value,
        vector: Value,
    },
    /// Vector reduction: maximum.
    VreduceMax {
        result: Value,
        vector: Value,
    },
    /// Vector load from memory.
    Vload {
        result: Value,
        addr: AddrExpr,
    },
    /// Vector store to memory.
    Vstore {
        value: Value,
        addr: AddrExpr,
    },
    /// Vector gather from memory.
    Vgather {
        result: Value,
        addr: AddrExpr,
        mask: Value,
    },
    /// Vector scatter to memory.
    Vscatter {
        value: Value,
        addr: AddrExpr,
        mask: Value,
    },

    // -------------------------------------------------------------------------
    //  Category 11: Conversion – 10 opcodes
    // -------------------------------------------------------------------------

    /// Sign-extend integer.
    Sext {
        result: Value,
        value: Value,
        from_type: IrType,
    },
    /// Zero-extend integer.
    Zext {
        result: Value,
        value: Value,
        from_type: IrType,
    },
    /// Truncate integer.
    Trunc {
        result: Value,
        value: Value,
        from_type: IrType,
    },
    /// Convert signed integer to float.
    Sitofp {
        result: Value,
        value: Value,
    },
    /// Convert unsigned integer to float.
    Uitofp {
        result: Value,
        value: Value,
    },
    /// Convert float to signed integer.
    Fptosi {
        result: Value,
        value: Value,
    },
    /// Convert float to unsigned integer.
    Fptoui {
        result: Value,
        value: Value,
    },
    /// Extend float to larger width.
    Fpext {
        result: Value,
        value: Value,
    },
    /// Truncate float to smaller width.
    Fptrunc {
        result: Value,
        value: Value,
    },
    /// Bitcast between types of the same width.
    Bitcast {
        result: Value,
        value: Value,
        to_type: IrType,
    },

    // -------------------------------------------------------------------------
    //  Category 12: System – 8 opcodes
    // -------------------------------------------------------------------------

    /// System call.
    Syscall,
    /// Software interrupt / trap.
    Int { vector: i64 },
    /// Memory fence / barrier.
    Fence,
    /// Breakpoint.
    Bkpt,
    /// Halt the processor.
    Hlt,
    /// Clear interrupt flag.
    Cli,
    /// Set interrupt flag.
    Sti,
    /// Read CPU identification.
    Cpuid { result: Value },

    // -------------------------------------------------------------------------
    //  Category 13: Auxiliary – 3 opcodes
    // -------------------------------------------------------------------------

    /// Ternary select: result = cond ? true_val : false_val.
    Select {
        result: Value,
        cond: Value,
        true_val: Value,
        false_val: Value,
    },
    /// Phi node: merge values from predecessor blocks.
    Phi {
        result: Value,
        /// (value, bb_name) pairs.
        incoming: Vec<(Value, String)>,
    },
    /// No-operation.
    Nop,
}

impl Instruction {
    /// Return the opcode mnemonic string.
    pub fn opcode(&self) -> &str {
        match self {
            // Category 1: Arithmetic
            Instruction::Add { .. } => "add",
            Instruction::Sub { .. } => "sub",
            Instruction::Mul { .. } => "mul",
            Instruction::Mulh { .. } => "mulh",
            Instruction::Div { .. } => "div",
            Instruction::Divu { .. } => "divu",
            Instruction::Rem { .. } => "rem",
            Instruction::Remu { .. } => "remu",
            Instruction::And { .. } => "and",
            Instruction::Or { .. } => "or",
            Instruction::Xor { .. } => "xor",
            Instruction::Shl { .. } => "shl",
            Instruction::Shr { .. } => "shr",
            Instruction::Sar { .. } => "sar",
            Instruction::Rotl { .. } => "rotl",
            Instruction::Rotr { .. } => "rotr",
            Instruction::Neg { .. } => "neg",
            Instruction::Not { .. } => "not",

            // Category 2: Immediate
            Instruction::Addi { .. } => "addi",
            Instruction::Subi { .. } => "subi",
            Instruction::Muli { .. } => "muli",
            Instruction::Andi { .. } => "andi",
            Instruction::Ori { .. } => "ori",
            Instruction::Xori { .. } => "xori",
            Instruction::Shli { .. } => "shli",
            Instruction::Shri { .. } => "shri",
            Instruction::Sari { .. } => "sari",
            Instruction::Rotli { .. } => "rotli",
            Instruction::Rotri { .. } => "rotri",
            Instruction::Movi { .. } => "movi",
            Instruction::Mov { .. } => "mov",

            // Category 3: Flag consumers
            Instruction::TestEq { .. } => "test_eq",
            Instruction::TestNe { .. } => "test_ne",
            Instruction::TestLt { .. } => "test_lt",
            Instruction::TestLe { .. } => "test_le",
            Instruction::TestLtu { .. } => "test_ltu",
            Instruction::TestLeu { .. } => "test_leu",
            Instruction::TestOf { .. } => "test_of",
            Instruction::TestCf { .. } => "test_cf",
            Instruction::TestSf { .. } => "test_sf",
            Instruction::TestGe { .. } => "test_ge",
            Instruction::TestGt { .. } => "test_gt",
            Instruction::TestGeu { .. } => "test_geu",
            Instruction::TestGtu { .. } => "test_gtu",

            // Category 4: Memory
            Instruction::Load { .. } => "load",
            Instruction::Loadi { .. } => "loadi",
            Instruction::LoadSext { .. } => "load_sext",
            Instruction::LoadZext { .. } => "load_zext",
            Instruction::Store { .. } => "store",
            Instruction::Storei { .. } => "storei",
            Instruction::Lea { .. } => "lea",

            // Category 5: Composite memory
            Instruction::MemAdd { .. } => "mem_add",
            Instruction::MemSub { .. } => "mem_sub",
            Instruction::MemAnd { .. } => "mem_and",
            Instruction::MemOr { .. } => "mem_or",
            Instruction::MemXor { .. } => "mem_xor",
            Instruction::MemXchg { .. } => "mem_xchg",

            // Category 6: Atomic
            Instruction::AtomicMemAdd { .. } => "atomic_add",
            Instruction::AtomicMemXchg { .. } => "atomic_xchg",
            Instruction::AtomicCas { .. } => "atomic_cas",

            // Category 7: Stack
            Instruction::Push { .. } => "push",
            Instruction::Pop { .. } => "pop",
            Instruction::Enter { .. } => "enter",
            Instruction::Leave => "leave",

            // Category 8: Control flow
            Instruction::Br { .. } => "br",
            Instruction::BrCond { .. } => "br_cond",
            Instruction::Switch { .. } => "switch",
            Instruction::Call { .. } => "call",
            Instruction::CallIndirect { .. } => "call_indirect",
            Instruction::Ret { .. } => "ret",
            Instruction::TailCall { .. } => "tail_call",

            // Category 9: Float
            Instruction::Fadd { .. } => "fadd",
            Instruction::Fsub { .. } => "fsub",
            Instruction::Fmul { .. } => "fmul",
            Instruction::Fdiv { .. } => "fdiv",
            Instruction::Fneg { .. } => "fneg",
            Instruction::Fabs { .. } => "fabs",
            Instruction::Fsqrt { .. } => "fsqrt",
            Instruction::Fmin { .. } => "fmin",
            Instruction::Fmax { .. } => "fmax",
            Instruction::Ffma { .. } => "ffma",
            Instruction::FcmpEq { .. } => "fcmp_eq",
            Instruction::FcmpNe { .. } => "fcmp_ne",
            Instruction::FcmpLt { .. } => "fcmp_lt",
            Instruction::FcmpLe { .. } => "fcmp_le",
            Instruction::FcmpGt { .. } => "fcmp_gt",
            Instruction::FcmpGe { .. } => "fcmp_ge",
            Instruction::FcmpOrd { .. } => "fcmp_ord",
            Instruction::FcmpUno { .. } => "fcmp_uno",

            // Category 10: Vector
            Instruction::Vadd { .. } => "vadd",
            Instruction::Vsub { .. } => "vsub",
            Instruction::Vmul { .. } => "vmul",
            Instruction::Vdiv { .. } => "vdiv",
            Instruction::Vfma { .. } => "vfma",
            Instruction::Vshuffle { .. } => "vshuffle",
            Instruction::Vbroadcast { .. } => "vbroadcast",
            Instruction::Vextract { .. } => "vextract",
            Instruction::Vinsert { .. } => "vinsert",
            Instruction::VreduceAdd { .. } => "vreduce_add",
            Instruction::VreduceMin { .. } => "vreduce_min",
            Instruction::VreduceMax { .. } => "vreduce_max",
            Instruction::Vload { .. } => "vload",
            Instruction::Vstore { .. } => "vstore",
            Instruction::Vgather { .. } => "vgather",
            Instruction::Vscatter { .. } => "vscatter",

            // Category 11: Conversion
            Instruction::Sext { .. } => "sext",
            Instruction::Zext { .. } => "zext",
            Instruction::Trunc { .. } => "trunc",
            Instruction::Sitofp { .. } => "sitofp",
            Instruction::Uitofp { .. } => "uitofp",
            Instruction::Fptosi { .. } => "fptosi",
            Instruction::Fptoui { .. } => "fptoui",
            Instruction::Fpext { .. } => "fpext",
            Instruction::Fptrunc { .. } => "fptrunc",
            Instruction::Bitcast { .. } => "bitcast",

            // Category 12: System
            Instruction::Syscall => "syscall",
            Instruction::Int { .. } => "int",
            Instruction::Fence => "fence",
            Instruction::Bkpt => "bkpt",
            Instruction::Hlt => "hlt",
            Instruction::Cli => "cli",
            Instruction::Sti => "sti",
            Instruction::Cpuid { .. } => "cpuid",

            // Category 13: Auxiliary
            Instruction::Select { .. } => "select",
            Instruction::Phi { .. } => "phi",
            Instruction::Nop => "nop",
        }
    }

    /// Return true if this instruction has observable side effects.
    pub fn has_side_effects(&self) -> bool {
        match self {
            // Category 4: Memory – stores have side effects
            Instruction::Store { .. } | Instruction::Storei { .. } => true,

            // Category 5: Composite memory
            Instruction::MemAdd { .. }
            | Instruction::MemSub { .. }
            | Instruction::MemAnd { .. }
            | Instruction::MemOr { .. }
            | Instruction::MemXor { .. }
            | Instruction::MemXchg { .. } => true,

            // Category 6: Atomic
            Instruction::AtomicMemAdd { .. }
            | Instruction::AtomicMemXchg { .. }
            | Instruction::AtomicCas { .. } => true,

            // Category 7: Stack
            Instruction::Push { .. }
            | Instruction::Pop { .. }
            | Instruction::Enter { .. }
            | Instruction::Leave => true,

            // Category 8: Control flow
            Instruction::Br { .. }
            | Instruction::BrCond { .. }
            | Instruction::Switch { .. }
            | Instruction::Call { .. }
            | Instruction::CallIndirect { .. }
            | Instruction::Ret { .. }
            | Instruction::TailCall { .. } => true,

            // Category 10: Vector – vector stores have side effects
            Instruction::Vstore { .. } | Instruction::Vscatter { .. } => true,

            // Category 12: System
            Instruction::Syscall
            | Instruction::Int { .. }
            | Instruction::Fence
            | Instruction::Bkpt
            | Instruction::Hlt
            | Instruction::Cli
            | Instruction::Sti
            | Instruction::Cpuid { .. } => true,

            // Everything else has no side effects
            _ => false,
        }
    }

    /// Return the type of the result value, if this instruction produces one.
    pub fn result_type(&self) -> Option<&IrType> {
        match self {
            // Category 1: Arithmetic
            | Instruction::Add { ref result, .. }
            | Instruction::Sub { ref result, .. }
            | Instruction::Mul { ref result, .. }
            | Instruction::Mulh { ref result, .. }
            | Instruction::Div { ref result, .. }
            | Instruction::Divu { ref result, .. }
            | Instruction::Rem { ref result, .. }
            | Instruction::Remu { ref result, .. }
            | Instruction::And { ref result, .. }
            | Instruction::Or { ref result, .. }
            | Instruction::Xor { ref result, .. }
            | Instruction::Shl { ref result, .. }
            | Instruction::Shr { ref result, .. }
            | Instruction::Sar { ref result, .. }
            | Instruction::Rotl { ref result, .. }
            | Instruction::Rotr { ref result, .. }
            | Instruction::Neg { ref result, .. }
            | Instruction::Not { ref result, .. }

            // Category 2: Immediate
            | Instruction::Addi { ref result, .. }
            | Instruction::Subi { ref result, .. }
            | Instruction::Muli { ref result, .. }
            | Instruction::Andi { ref result, .. }
            | Instruction::Ori { ref result, .. }
            | Instruction::Xori { ref result, .. }
            | Instruction::Shli { ref result, .. }
            | Instruction::Shri { ref result, .. }
            | Instruction::Sari { ref result, .. }
            | Instruction::Rotli { ref result, .. }
            | Instruction::Rotri { ref result, .. }
            | Instruction::Movi { ref result, .. }
            | Instruction::Mov { ref result, .. }

            // Category 3: Flag consumers
            | Instruction::TestEq { ref result, .. }
            | Instruction::TestNe { ref result, .. }
            | Instruction::TestLt { ref result, .. }
            | Instruction::TestLe { ref result, .. }
            | Instruction::TestLtu { ref result, .. }
            | Instruction::TestLeu { ref result, .. }
            | Instruction::TestOf { ref result, .. }
            | Instruction::TestCf { ref result, .. }
            | Instruction::TestSf { ref result, .. }
            | Instruction::TestGe { ref result, .. }
            | Instruction::TestGt { ref result, .. }
            | Instruction::TestGeu { ref result, .. }
            | Instruction::TestGtu { ref result, .. }

            // Category 4: Memory
            | Instruction::Load { ref result, .. }
            | Instruction::Loadi { ref result, .. }
            | Instruction::LoadSext { ref result, .. }
            | Instruction::LoadZext { ref result, .. }
            | Instruction::Lea { ref result, .. }

            // Category 5: Composite memory
            | Instruction::MemXchg { ref result, .. }

            // Category 6: Atomic
            | Instruction::AtomicMemXchg { ref result, .. }
            | Instruction::AtomicCas { ref result, .. }

            // Category 7: Stack
            | Instruction::Pop { ref result, .. }

            // Category 8: Control flow
            | Instruction::Call { result: Some(ref result), .. }
            | Instruction::CallIndirect { result: Some(ref result), .. }

            // Category 9: Float
            | Instruction::Fadd { ref result, .. }
            | Instruction::Fsub { ref result, .. }
            | Instruction::Fmul { ref result, .. }
            | Instruction::Fdiv { ref result, .. }
            | Instruction::Fneg { ref result, .. }
            | Instruction::Fabs { ref result, .. }
            | Instruction::Fsqrt { ref result, .. }
            | Instruction::Fmin { ref result, .. }
            | Instruction::Fmax { ref result, .. }
            | Instruction::Ffma { ref result, .. }
            | Instruction::FcmpEq { ref result, .. }
            | Instruction::FcmpNe { ref result, .. }
            | Instruction::FcmpLt { ref result, .. }
            | Instruction::FcmpLe { ref result, .. }
            | Instruction::FcmpGt { ref result, .. }
            | Instruction::FcmpGe { ref result, .. }
            | Instruction::FcmpOrd { ref result, .. }
            | Instruction::FcmpUno { ref result, .. }

            // Category 10: Vector
            | Instruction::Vadd { ref result, .. }
            | Instruction::Vsub { ref result, .. }
            | Instruction::Vmul { ref result, .. }
            | Instruction::Vdiv { ref result, .. }
            | Instruction::Vfma { ref result, .. }
            | Instruction::Vshuffle { ref result, .. }
            | Instruction::Vbroadcast { ref result, .. }
            | Instruction::Vextract { ref result, .. }
            | Instruction::Vinsert { ref result, .. }
            | Instruction::VreduceAdd { ref result, .. }
            | Instruction::VreduceMin { ref result, .. }
            | Instruction::VreduceMax { ref result, .. }
            | Instruction::Vload { ref result, .. }
            | Instruction::Vgather { ref result, .. }

            // Category 11: Conversion
            | Instruction::Sext { ref result, .. }
            | Instruction::Zext { ref result, .. }
            | Instruction::Trunc { ref result, .. }
            | Instruction::Sitofp { ref result, .. }
            | Instruction::Uitofp { ref result, .. }
            | Instruction::Fptosi { ref result, .. }
            | Instruction::Fptoui { ref result, .. }
            | Instruction::Fpext { ref result, .. }
            | Instruction::Fptrunc { ref result, .. }
            | Instruction::Bitcast { ref result, .. }

            // Category 12: System
            | Instruction::Cpuid { ref result, .. }

            // Category 13: Auxiliary
            | Instruction::Select { ref result, .. }
            | Instruction::Phi { ref result, .. } => Some(result.ty()),
            _ => None,
        }
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // =================================================================
            //  Category 1: Arithmetic (flag-producing)
            // =================================================================
            Instruction::Add {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = add {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Sub {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = sub {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Mul {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = mul {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Mulh {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = mulh {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Div {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = div {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Divu {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = divu {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Rem {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = rem {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Remu {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = remu {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::And {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = and {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Or {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = or {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Xor {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = xor {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Shl {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = shl {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Shr {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = shr {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Sar {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = sar {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Rotl {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = rotl {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Rotr {
                result,
                lhs,
                rhs,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = rotr {} {lhs}, {rhs}",
                result.ty()
            ),
            Instruction::Neg {
                result,
                operand,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = neg {} {operand}",
                result.ty()
            ),
            Instruction::Not {
                result,
                operand,
                flags_result,
            } => write!(
                f,
                "{result}, {flags_result} = not {} {operand}",
                result.ty()
            ),

            // =================================================================
            //  Category 2: Immediate
            // =================================================================
            Instruction::Addi {
                result,
                lhs,
                imm,
                flags_result: _,
            } => write!(f, "{result} = addi {} {lhs}, {imm}", result.ty()),
            Instruction::Subi {
                result,
                lhs,
                imm,
                flags_result: _,
            } => write!(f, "{result} = subi {} {lhs}, {imm}", result.ty()),
            Instruction::Muli {
                result,
                lhs,
                imm,
                flags_result: _,
            } => write!(f, "{result} = muli {} {lhs}, {imm}", result.ty()),
            Instruction::Andi {
                result,
                lhs,
                imm,
                flags_result: _,
            } => write!(f, "{result} = andi {} {lhs}, {imm}", result.ty()),
            Instruction::Ori {
                result,
                lhs,
                imm,
                flags_result: _,
            } => write!(f, "{result} = ori {} {lhs}, {imm}", result.ty()),
            Instruction::Xori {
                result,
                lhs,
                imm,
                flags_result: _,
            } => write!(f, "{result} = xori {} {lhs}, {imm}", result.ty()),
            Instruction::Shli {
                result,
                lhs,
                imm,
                flags_result: _,
            } => write!(f, "{result} = shli {} {lhs}, {imm}", result.ty()),
            Instruction::Shri {
                result,
                lhs,
                imm,
                flags_result: _,
            } => write!(f, "{result} = shri {} {lhs}, {imm}", result.ty()),
            Instruction::Sari {
                result,
                lhs,
                imm,
                flags_result: _,
            } => write!(f, "{result} = sari {} {lhs}, {imm}", result.ty()),
            Instruction::Rotli {
                result,
                lhs,
                imm,
                flags_result: _,
            } => write!(f, "{result} = rotli {} {lhs}, {imm}", result.ty()),
            Instruction::Rotri {
                result,
                lhs,
                imm,
                flags_result: _,
            } => write!(f, "{result} = rotri {} {lhs}, {imm}", result.ty()),
            Instruction::Movi { result, imm } => {
                write!(f, "{result} = movi {} {imm}", result.ty())
            }
            Instruction::Mov { result, src } => {
                write!(f, "{result} = mov {} {src}", result.ty())
            }

            // =================================================================
            //  Category 3: Flag consumers
            // =================================================================
            Instruction::TestEq { result, flags } => write!(f, "{result} = test_eq {flags}"),
            Instruction::TestNe { result, flags } => write!(f, "{result} = test_ne {flags}"),
            Instruction::TestLt { result, flags } => write!(f, "{result} = test_lt {flags}"),
            Instruction::TestLe { result, flags } => write!(f, "{result} = test_le {flags}"),
            Instruction::TestLtu { result, flags } => write!(f, "{result} = test_ltu {flags}"),
            Instruction::TestLeu { result, flags } => write!(f, "{result} = test_leu {flags}"),
            Instruction::TestOf { result, flags } => write!(f, "{result} = test_of {flags}"),
            Instruction::TestCf { result, flags } => write!(f, "{result} = test_cf {flags}"),
            Instruction::TestSf { result, flags } => write!(f, "{result} = test_sf {flags}"),
            Instruction::TestGe { result, flags } => write!(f, "{result} = test_ge {flags}"),
            Instruction::TestGt { result, flags } => write!(f, "{result} = test_gt {flags}"),
            Instruction::TestGeu { result, flags } => write!(f, "{result} = test_geu {flags}"),
            Instruction::TestGtu { result, flags } => write!(f, "{result} = test_gtu {flags}"),

            // =================================================================
            //  Category 4: Memory
            // =================================================================
            Instruction::Load { result, addr } => {
                write!(f, "{result} = load {}, {addr}", result.ty())
            }
            Instruction::Loadi {
                result,
                base,
                offset,
            } => write!(f, "{result} = loadi {}, {base}, {offset}", result.ty()),
            Instruction::LoadSext {
                result,
                addr,
                from_type,
            } => write!(
                f,
                "{result} = load_sext {from_type} -> {}, {addr}",
                result.ty()
            ),
            Instruction::LoadZext {
                result,
                addr,
                from_type,
            } => write!(
                f,
                "{result} = load_zext {from_type} -> {}, {addr}",
                result.ty()
            ),
            Instruction::Store { value, addr } => {
                write!(f, "store {} {value}, {addr}", value.ty())
            }
            Instruction::Storei {
                value,
                base,
                offset,
            } => write!(
                f,
                "storei {} {value}, {base}, {offset}",
                value.ty()
            ),
            Instruction::Lea { result, addr } => write!(f, "{result} = lea {addr}"),

            // =================================================================
            //  Category 5: Composite memory
            // =================================================================
            Instruction::MemAdd { addr, value } => write!(f, "mem_add {addr}, {value}"),
            Instruction::MemSub { addr, value } => write!(f, "mem_sub {addr}, {value}"),
            Instruction::MemAnd { addr, value } => write!(f, "mem_and {addr}, {value}"),
            Instruction::MemOr { addr, value } => write!(f, "mem_or {addr}, {value}"),
            Instruction::MemXor { addr, value } => write!(f, "mem_xor {addr}, {value}"),
            Instruction::MemXchg {
                result,
                addr,
                value,
            } => write!(f, "{result} = mem_xchg {addr}, {value}"),

            // =================================================================
            //  Category 6: Atomic
            // =================================================================
            Instruction::AtomicMemAdd { addr, value } => {
                write!(f, "atomic_add {addr}, {value}")
            }
            Instruction::AtomicMemXchg {
                result,
                addr,
                value,
            } => write!(f, "{result} = atomic_xchg {addr}, {value}"),
            Instruction::AtomicCas {
                result,
                addr,
                expected,
                desired,
            } => write!(f, "{result} = atomic_cas {addr}, {expected}, {desired}"),

            // =================================================================
            //  Category 7: Stack
            // =================================================================
            Instruction::Push { value } => write!(f, "push {} {value}", value.ty()),
            Instruction::Pop { result } => write!(f, "{result} = pop {}", result.ty()),
            Instruction::Enter { frame_size } => write!(f, "enter {frame_size}"),
            Instruction::Leave => write!(f, "leave"),

            // =================================================================
            //  Category 8: Control flow
            // =================================================================
            Instruction::Br { target_bb } => write!(f, "br %{target_bb}"),
            Instruction::BrCond {
                cond,
                true_bb,
                false_bb,
            } => write!(f, "br_cond {cond}, %{true_bb}, %{false_bb}"),
            Instruction::Switch {
                value,
                default_bb,
                cases,
            } => {
                let case_strs: Vec<String> = cases
                    .iter()
                    .map(|(cv, bb)| format!("{cv}: %{bb}"))
                    .collect();
                write!(f, "switch {value}, %{default_bb} [{}]", case_strs.join(", "))
            }
            Instruction::Call {
                result,
                callee_name,
                args,
            } => {
                let arg_strs: Vec<String> =
                    args.iter().map(|a| format!("{} {a}", a.ty())).collect();
                let arg_str = arg_strs.join(", ");
                if let Some(ref r) = result {
                    write!(f, "{r} = call @{callee_name}({arg_str})")
                } else {
                    write!(f, "call @{callee_name}({arg_str})")
                }
            }
            Instruction::CallIndirect {
                result,
                fnptr,
                args,
            } => {
                let arg_strs: Vec<String> =
                    args.iter().map(|a| format!("{} {a}", a.ty())).collect();
                let arg_str = arg_strs.join(", ");
                if let Some(ref r) = result {
                    write!(f, "{r} = call_indirect {fnptr}({arg_str})")
                } else {
                    write!(f, "call_indirect {fnptr}({arg_str})")
                }
            }
            Instruction::Ret { value } => {
                if let Some(ref v) = value {
                    write!(f, "ret {} {v}", v.ty())
                } else {
                    write!(f, "ret void")
                }
            }
            Instruction::TailCall { callee_name, args } => {
                let arg_strs: Vec<String> =
                    args.iter().map(|a| format!("{} {a}", a.ty())).collect();
                let arg_str = arg_strs.join(", ");
                write!(f, "tail_call @{callee_name}({arg_str})")
            }

            // =================================================================
            //  Category 9: Float
            // =================================================================
            Instruction::Fadd { result, lhs, rhs } => {
                write!(f, "{result} = fadd {} {lhs}, {rhs}", result.ty())
            }
            Instruction::Fsub { result, lhs, rhs } => {
                write!(f, "{result} = fsub {} {lhs}, {rhs}", result.ty())
            }
            Instruction::Fmul { result, lhs, rhs } => {
                write!(f, "{result} = fmul {} {lhs}, {rhs}", result.ty())
            }
            Instruction::Fdiv { result, lhs, rhs } => {
                write!(f, "{result} = fdiv {} {lhs}, {rhs}", result.ty())
            }
            Instruction::Fneg { result, operand } => {
                write!(f, "{result} = fneg {} {operand}", result.ty())
            }
            Instruction::Fabs { result, operand } => {
                write!(f, "{result} = fabs {} {operand}", result.ty())
            }
            Instruction::Fsqrt { result, operand } => {
                write!(f, "{result} = fsqrt {} {operand}", result.ty())
            }
            Instruction::Fmin { result, lhs, rhs } => {
                write!(f, "{result} = fmin {} {lhs}, {rhs}", result.ty())
            }
            Instruction::Fmax { result, lhs, rhs } => {
                write!(f, "{result} = fmax {} {lhs}, {rhs}", result.ty())
            }
            Instruction::Ffma { result, a, b, c } => {
                write!(f, "{result} = ffma {} {a}, {b}, {c}", result.ty())
            }
            Instruction::FcmpEq { result, lhs, rhs } => {
                write!(f, "{result} = fcmp_eq {} {lhs}, {rhs}", lhs.ty())
            }
            Instruction::FcmpNe { result, lhs, rhs } => {
                write!(f, "{result} = fcmp_ne {} {lhs}, {rhs}", lhs.ty())
            }
            Instruction::FcmpLt { result, lhs, rhs } => {
                write!(f, "{result} = fcmp_lt {} {lhs}, {rhs}", lhs.ty())
            }
            Instruction::FcmpLe { result, lhs, rhs } => {
                write!(f, "{result} = fcmp_le {} {lhs}, {rhs}", lhs.ty())
            }
            Instruction::FcmpGt { result, lhs, rhs } => {
                write!(f, "{result} = fcmp_gt {} {lhs}, {rhs}", lhs.ty())
            }
            Instruction::FcmpGe { result, lhs, rhs } => {
                write!(f, "{result} = fcmp_ge {} {lhs}, {rhs}", lhs.ty())
            }
            Instruction::FcmpOrd { result, lhs, rhs } => {
                write!(f, "{result} = fcmp_ord {} {lhs}, {rhs}", lhs.ty())
            }
            Instruction::FcmpUno { result, lhs, rhs } => {
                write!(f, "{result} = fcmp_uno {} {lhs}, {rhs}", lhs.ty())
            }

            // =================================================================
            //  Category 10: Vector
            // =================================================================
            Instruction::Vadd { result, lhs, rhs } => {
                write!(f, "{result} = vadd {} {lhs}, {rhs}", result.ty())
            }
            Instruction::Vsub { result, lhs, rhs } => {
                write!(f, "{result} = vsub {} {lhs}, {rhs}", result.ty())
            }
            Instruction::Vmul { result, lhs, rhs } => {
                write!(f, "{result} = vmul {} {lhs}, {rhs}", result.ty())
            }
            Instruction::Vdiv { result, lhs, rhs } => {
                write!(f, "{result} = vdiv {} {lhs}, {rhs}", result.ty())
            }
            Instruction::Vfma { result, a, b, c } => {
                write!(f, "{result} = vfma {} {a}, {b}, {c}", result.ty())
            }
            Instruction::Vshuffle {
                result,
                lhs,
                rhs,
                mask,
            } => write!(
                f,
                "{result} = vshuffle {} {lhs}, {rhs}, {mask}",
                result.ty()
            ),
            Instruction::Vbroadcast { result, value } => {
                write!(
                    f,
                    "{result} = vbroadcast {} {value}",
                    result.ty()
                )
            }
            Instruction::Vextract {
                result,
                vector,
                index,
            } => write!(
                f,
                "{result} = vextract {} {vector}, {index}",
                result.ty()
            ),
            Instruction::Vinsert {
                result,
                vector,
                value,
                index,
            } => write!(
                f,
                "{result} = vinsert {} {vector}, {value}, {index}",
                result.ty()
            ),
            Instruction::VreduceAdd { result, vector } => {
                write!(f, "{result} = vreduce_add {} {vector}", vector.ty())
            }
            Instruction::VreduceMin { result, vector } => {
                write!(f, "{result} = vreduce_min {} {vector}", vector.ty())
            }
            Instruction::VreduceMax { result, vector } => {
                write!(f, "{result} = vreduce_max {} {vector}", vector.ty())
            }
            Instruction::Vload { result, addr } => {
                write!(f, "{result} = vload {}, {addr}", result.ty())
            }
            Instruction::Vstore { value, addr } => {
                write!(f, "vstore {} {value}, {addr}", value.ty())
            }
            Instruction::Vgather {
                result,
                addr,
                mask,
            } => write!(
                f,
                "{result} = vgather {}, {addr}, {mask}",
                result.ty()
            ),
            Instruction::Vscatter {
                value,
                addr,
                mask,
            } => write!(
                f,
                "vscatter {} {value}, {addr}, {mask}",
                value.ty()
            ),

            // =================================================================
            //  Category 11: Conversion
            // =================================================================
            Instruction::Sext {
                result,
                value,
                from_type,
            } => write!(
                f,
                "{result} = sext {from_type} {value} to {}",
                result.ty()
            ),
            Instruction::Zext {
                result,
                value,
                from_type,
            } => write!(
                f,
                "{result} = zext {from_type} {value} to {}",
                result.ty()
            ),
            Instruction::Trunc {
                result,
                value,
                from_type,
            } => write!(
                f,
                "{result} = trunc {from_type} {value} to {}",
                result.ty()
            ),
            Instruction::Sitofp { result, value } => write!(
                f,
                "{result} = sitofp {} {value} to {}",
                value.ty(),
                result.ty()
            ),
            Instruction::Uitofp { result, value } => write!(
                f,
                "{result} = uitofp {} {value} to {}",
                value.ty(),
                result.ty()
            ),
            Instruction::Fptosi { result, value } => write!(
                f,
                "{result} = fptosi {} {value} to {}",
                value.ty(),
                result.ty()
            ),
            Instruction::Fptoui { result, value } => write!(
                f,
                "{result} = fptoui {} {value} to {}",
                value.ty(),
                result.ty()
            ),
            Instruction::Fpext { result, value } => write!(
                f,
                "{result} = fpext {} {value} to {}",
                value.ty(),
                result.ty()
            ),
            Instruction::Fptrunc { result, value } => write!(
                f,
                "{result} = fptrunc {} {value} to {}",
                value.ty(),
                result.ty()
            ),
            Instruction::Bitcast {
                result,
                value,
                to_type,
            } => write!(
                f,
                "{result} = bitcast {} {value} to {to_type}",
                value.ty()
            ),

            // =================================================================
            //  Category 12: System
            // =================================================================
            Instruction::Syscall => write!(f, "syscall"),
            Instruction::Int { vector } => write!(f, "int {vector}"),
            Instruction::Fence => write!(f, "fence"),
            Instruction::Bkpt => write!(f, "bkpt"),
            Instruction::Hlt => write!(f, "hlt"),
            Instruction::Cli => write!(f, "cli"),
            Instruction::Sti => write!(f, "sti"),
            Instruction::Cpuid { result } => write!(f, "{result} = cpuid"),

            // =================================================================
            //  Category 13: Auxiliary
            // =================================================================
            Instruction::Select {
                result,
                cond,
                true_val,
                false_val,
            } => write!(
                f,
                "{result} = select {} {cond}, {true_val}, {false_val}",
                cond.ty()
            ),
            Instruction::Phi { result, incoming } => {
                let parts: Vec<String> = incoming
                    .iter()
                    .map(|(v, bb)| format!("[{v}, %{bb}]"))
                    .collect();
                write!(f, "{result} = phi {} {}", result.ty(), parts.join(", "))
            }
            Instruction::Nop => write!(f, "nop"),
        }
    }
}

// =============================================================================
//  BasicBlock
// =============================================================================

/// A single-entry, single-exit sequence of instructions.
#[derive(Clone, Debug)]
pub struct BasicBlock {
    /// Block name (without % prefix).
    pub name: String,
    /// Instructions in this block.
    pub instructions: Vec<Instruction>,
    /// Predecessor block names.
    pub predecessors: Vec<String>,
    /// Successor block names.
    pub successors: Vec<String>,
    /// Whether this is the entry block of the function.
    pub is_entry: bool,
}

impl BasicBlock {
    /// Create a new basic block with the given name.
    pub fn new(name: String) -> Self {
        BasicBlock {
            name,
            instructions: Vec::new(),
            predecessors: Vec::new(),
            successors: Vec::new(),
            is_entry: false,
        }
    }

    /// Append an instruction to this block, updating successors for terminators.
    pub fn add_instruction(&mut self, inst: Instruction) {
        // Update successors based on terminator instructions
        match &inst {
            Instruction::Br { target_bb } => {
                if !self.successors.contains(target_bb) {
                    self.successors.push(target_bb.clone());
                }
            }
            Instruction::BrCond {
                true_bb, false_bb, ..
            } => {
                if !self.successors.contains(true_bb) {
                    self.successors.push(true_bb.clone());
                }
                if !self.successors.contains(false_bb) {
                    self.successors.push(false_bb.clone());
                }
            }
            Instruction::Switch {
                default_bb, cases, ..
            } => {
                if !self.successors.contains(default_bb) {
                    self.successors.push(default_bb.clone());
                }
                for (_, case_bb) in cases {
                    if !self.successors.contains(case_bb) {
                        self.successors.push(case_bb.clone());
                    }
                }
            }
            _ => {}
        }
        self.instructions.push(inst);
    }

    /// Return the terminator instruction (last in the block), or None.
    pub fn terminator(&self) -> Option<&Instruction> {
        self.instructions.last()
    }
}

impl fmt::Display for BasicBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "%{}:", self.name)?;
        if !self.predecessors.is_empty() {
            let preds: Vec<String> = self
                .predecessors
                .iter()
                .map(|p| format!("%{p}"))
                .collect();
            writeln!(f, "  ; preds: {}", preds.join(", "))?;
        }
        for inst in &self.instructions {
            writeln!(f, "  {inst}")?;
        }
        Ok(())
    }
}

// =============================================================================
//  Function
// =============================================================================

/// A function in the IR.
#[derive(Clone, Debug)]
pub struct Function {
    /// Function name (without @ prefix).
    pub name: String,
    /// Function parameters.
    pub parameters: Vec<Value>,
    /// Return type of the function.
    pub return_type: IrType,
    /// Basic blocks in this function.
    pub basic_blocks: Vec<BasicBlock>,
    /// Calling convention name.
    pub call_conv: String,
    /// Per-function virtual register counter.
    pub vreg_counter: usize,
}

impl Function {
    /// Create a new function with the given name and calling convention.
    pub fn new(name: String, call_conv: String) -> Self {
        Function {
            name,
            parameters: Vec::new(),
            return_type: IrType::Void,
            basic_blocks: Vec::new(),
            call_conv,
            vreg_counter: 0,
        }
    }

    /// Generate a unique virtual register name scoped to this function.
    pub fn new_vreg(&mut self, ty: IrType) -> Value {
        let name = format!("%{}", self.vreg_counter);
        self.vreg_counter += 1;
        Value::VReg { name, ty }
    }

    /// Return the entry basic block, or None.
    pub fn entry_block(&self) -> Option<&BasicBlock> {
        for bb in &self.basic_blocks {
            if bb.is_entry {
                return Some(bb);
            }
        }
        self.basic_blocks.first()
    }

    /// Mutable reference to the entry basic block, or None.
    pub fn entry_block_mut(&mut self) -> Option<&mut BasicBlock> {
        if self.basic_blocks.iter().any(|bb| bb.is_entry) {
            self.basic_blocks.iter_mut().find(|bb| bb.is_entry)
        } else {
            self.basic_blocks.first_mut()
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "function {} ({}) -> {}:",
            self.name, self.call_conv, self.return_type
        )?;
        for p in &self.parameters {
            writeln!(f, "  param {} {p}", p.ty())?;
        }
        for (i, bb) in self.basic_blocks.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{bb}")?;
        }
        Ok(())
    }
}

// =============================================================================
//  Module
// =============================================================================

/// Top-level compilation unit.
#[derive(Clone, Debug)]
pub struct Module {
    /// Module name.
    pub name: String,
    /// Functions in this module.
    pub functions: Vec<Function>,
    /// Global variables in this module.
    pub globals: Vec<Value>,
    /// Target triple string.
    pub target_triple: String,
}

impl Module {
    /// Create a new module with the given name.
    pub fn new(name: String) -> Self {
        Module {
            name,
            functions: Vec::new(),
            globals: Vec::new(),
            target_triple: "macrocore-x-unknown-elf".to_string(),
        }
    }
}

impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "module {} {{", self.name)?;
        writeln!(f, "  target_triple = \"{}\"", self.target_triple)?;
        for gv in &self.globals {
            writeln!(f, "  global {} {gv}", gv.ty())?;
        }
        for func in &self.functions {
            writeln!(f)?;
            write!(f, "{func}")?;
        }
        writeln!(f)?;
        writeln!(f, "}}")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::IrType;

    #[test]
    fn test_basic_block_display() {
        let mut bb = BasicBlock::new("entry".to_string());
        bb.is_entry = true;
        let vreg = Value::VReg {
            name: "%0".to_string(),
            ty: IrType::I64,
        };
        let inst = Instruction::Movi {
            result: vreg,
            imm: 42,
        };
        bb.add_instruction(inst);
        let output = bb.to_string();
        assert!(output.contains("%entry:"));
        assert!(output.contains("movi i64 42"));
    }

    #[test]
    fn test_function_display() {
        let mut func = Function::new("main".to_string(), "nova".to_string());
        let param = Value::FuncParam {
            name: "%argc".to_string(),
            ty: IrType::I64,
            index: 0,
        };
        func.parameters.push(param);
        func.return_type = IrType::I64;
        let mut bb = BasicBlock::new("entry".to_string());
        bb.is_entry = true;
        let vreg = Value::VReg {
            name: "%0".to_string(),
            ty: IrType::I64,
        };
        bb.add_instruction(Instruction::Movi {
            result: vreg,
            imm: 0,
        });
        func.basic_blocks.push(bb);
        let output = func.to_string();
        assert!(output.contains("function main (nova) -> i64:"));
        assert!(output.contains("param i64 %argc"));
    }

    #[test]
    fn test_module_display() {
        let module = Module::new("test".to_string());
        let output = module.to_string();
        assert!(output.contains("module test {"));
        assert!(output.contains("target_triple"));
        assert!(output.contains("}"));
    }

    #[test]
    fn test_instruction_opcode() {
        let inst = Instruction::Nop;
        assert_eq!(inst.opcode(), "nop");
        let inst = Instruction::Add {
            result: Value::VReg {
                name: "%0".to_string(),
                ty: IrType::I64,
            },
            lhs: Value::ConstInt {
                value: 1,
                ty: IrType::I64,
            },
            rhs: Value::ConstInt {
                value: 2,
                ty: IrType::I64,
            },
            flags_result: Value::VReg {
                name: "%1".to_string(),
                ty: IrType::Flags,
            },
        };
        assert_eq!(inst.opcode(), "add");
    }

    #[test]
    fn test_has_side_effects() {
        assert!(!Instruction::Nop.has_side_effects());
        assert!(Instruction::Syscall.has_side_effects());
        let store = Instruction::Store {
            value: Value::ConstInt {
                value: 42,
                ty: IrType::I64,
            },
            addr: AddrExpr {
                base: Value::VReg {
                    name: "%0".to_string(),
                    ty: IrType::Ptr,
                },
                index: None,
                scale: 1,
                offset: 0,
            },
        };
        assert!(store.has_side_effects());
    }

    #[test]
    fn test_result_type() {
        let inst = Instruction::Nop;
        assert_eq!(inst.result_type(), None);
        let inst = Instruction::Movi {
            result: Value::VReg {
                name: "%0".to_string(),
                ty: IrType::I64,
            },
            imm: 42,
        };
        assert_eq!(inst.result_type(), Some(&IrType::I64));
    }
}