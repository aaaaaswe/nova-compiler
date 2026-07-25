use thiserror::Error;

#[derive(Error, Debug)]
pub enum SimError {
    #[error("PC out of bounds: 0x{pc:x}")]
    PcOutOfBounds { pc: u64 },

    #[error("Memory access out of bounds at address 0x{addr:x}")]
    MemoryOutOfBounds { addr: u64 },

    #[error("Division by zero at PC=0x{pc:x}")]
    DivisionByZero { pc: u64 },

    #[error("Illegal instruction 0x{opcode:02x} at PC=0x{pc:x}")]
    IllegalInstruction { opcode: u8, pc: u64 },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}