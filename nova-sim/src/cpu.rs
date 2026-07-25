use crate::decode::decode_one;
use crate::error::SimError;
use crate::execute::execute_one;

/// Step result: indicates whether execution should continue.
pub enum StepResult {
    Continue,
    Halted,
    ECall { exit_code: u64 },
}

/// Condition flags.
#[derive(Debug, Clone, Default)]
pub struct Flags {
    pub cf: bool,
    pub zf: bool,
    pub sf: bool,
    pub of: bool,
}

/// MacroCore-X CPU state.
pub struct Cpu {
    pub regs: [u64; 32],     // R0-R31 general purpose registers
    pub fregs: [f64; 32],    // F0-F31 floating point registers
    pub pc: u64,             // Program counter
    pub flags: Flags,        // Condition flags
    pub memory: Vec<u8>,     // Flat memory
    pub running: bool,

    // Convenience aliases (same memory as regs)
    // r[0] = regs[0], r[1] = regs[1], etc.
    pub r: [u64; 32],
    pub f: [f64; 32],
    pub v: [u64; 32],        // Vector registers (simplified, scalar)

    // CPU state
    pub steps: u64,
    pub halted: bool,

    // CSR registers
    pub csr_cr3: u64,
    pub csr_mode: u64,
    pub csr_ivec: u64,
    pub err: u64,
    pub ef: u64,
    pub priv_mode: u64,
    pub mmu_enabled: bool,
}

impl Cpu {
    pub fn new() -> Self {
        let memory_size = 0x100000; // 1MB
        let mut cpu = Cpu {
            regs: [0; 32],
            fregs: [0.0; 32],
            pc: 0x1000,
            flags: Flags::default(),
            memory: vec![0; memory_size],
            running: true,
            r: [0; 32],
            f: [0.0; 32],
            v: [0; 32],
            steps: 0,
            halted: false,
            csr_cr3: 0,
            csr_mode: 0,
            csr_ivec: 0,
            err: 0,
            ef: 0,
            priv_mode: 1,
            mmu_enabled: false,
        };
        cpu.r[2] = 0xFF000;  // SP
        cpu.r[30] = 0xFF000; // FP
        cpu.regs[2] = 0xFF000;
        cpu.regs[30] = 0xFF000;
        cpu
    }

    /// Load a binary blob at address 0x1000.
    pub fn load_binary(&mut self, data: &[u8]) {
        let base = 0x1000;
        let end = base + data.len();
        if end > self.memory.len() {
            self.memory.resize(end, 0);
        }
        self.memory[base..end].copy_from_slice(data);
        self.pc = base as u64;
        self.r[2] = (self.memory.len() - 0x1000) as u64;
        self.r[30] = self.r[2];
        self.regs[2] = self.r[2];
        self.regs[30] = self.r[30];
    }

    /// Execute one instruction. Returns step result.
    pub fn step(&mut self) -> Result<StepResult, SimError> {
        let pc = self.pc;

        if pc as usize >= self.memory.len() {
            return Err(SimError::PcOutOfBounds { pc });
        }

        let opcode = self.memory[pc as usize];
        let length = crate::decode::get_inst_length(&self.memory, pc, opcode);

        // Ensure we have enough bytes
        if pc as usize + length > self.memory.len() {
            return Err(SimError::PcOutOfBounds { pc: pc + length as u64 });
        }

        let (inst, _) = decode_one(&self.memory, pc);

        // Sync aliases from regs
        self.r.copy_from_slice(&self.regs);
        self.f.copy_from_slice(&self.fregs);

        let cont = execute_one(self, &inst, length, pc)?;

        // Sync aliases back to regs
        self.regs.copy_from_slice(&self.r);
        self.fregs.copy_from_slice(&self.f);

        if !cont {
            if self.halted {
                return Ok(StepResult::Halted);
            }
            return Ok(StepResult::ECall { exit_code: self.r[1] });
        }

        Ok(StepResult::Continue)
    }

    /// Run until ecall/hlt/bkpt, with an optional step limit.
    /// Returns the exit code (R1 value).
    pub fn run(&mut self, debug: bool) -> Result<u64, Box<dyn std::error::Error>> {
        self.run_with_limit(debug, 0)
    }

    /// Run until ecall/hlt/bkpt, with a maximum step limit (0 = no limit).
    pub fn run_with_limit(&mut self, debug: bool, max_steps: u64) -> Result<u64, Box<dyn std::error::Error>> {
        if debug {
            println!("\n{:=>60}", "");
            println!("  MacroCore-X Simulator Trace");
            println!("{:=>60}", "");
            println!("  PC    : 0x{:04x}", self.pc);
            println!("  SP (R2): 0x{:016x}", self.r[2]);
            println!("  FP (R30): 0x{:016x}", self.r[30]);
            println!("{:=>60}\n", "");
        }

        while self.running {
            if max_steps > 0 && self.steps >= max_steps {
                if debug {
                    println!("\n[sim] Step limit reached: {}", max_steps);
                }
                break;
            }

            let pc = self.pc;

            if pc as usize >= self.memory.len() {
                println!("\n[sim] PC out of bounds: 0x{:x}", pc);
                break;
            }

            let (inst, _) = decode_one(&self.memory, pc);

            if debug {
                println!("  [{:6}] 0x{:04x}: {}", self.steps, pc, inst);
            }

            match self.step()? {
                StepResult::Continue => {}
                StepResult::Halted => {
                    break;
                }
                StepResult::ECall { exit_code } => {
                    self.print_state();
                    return Ok(exit_code);
                }
            }
        }

        self.print_state();
        Ok(self.r[1])
    }

    fn print_state(&self) {
        println!("\n{:=>60}", "");
        println!("  Simulation Complete — {} steps", self.steps);
        println!("{:=>60}", "");
        println!("  Registers:");
        for i in (0..32).step_by(4) {
            println!(
                "  {}  {}  {}  {}",
                format!("R{:2}=0x{:016x}", i, self.r[i]),
                format!("R{:2}=0x{:016x}", i + 1, self.r[i + 1]),
                format!("R{:2}=0x{:016x}", i + 2, self.r[i + 2]),
                format!("R{:2}=0x{:016x}", i + 3, self.r[i + 3]),
            );
        }
        println!(
            "  Flags: CF={} ZF={} SF={} OF={}",
            self.flags.cf as u8, self.flags.zf as u8, self.flags.sf as u8, self.flags.of as u8,
        );
        println!("  F-Registers:");
        for i in (0..32).step_by(4) {
            println!(
                "  {}  {}  {}  {}",
                format!("F{:2}=0x{:016x}", i, self.f[i].to_bits()),
                format!("F{:2}=0x{:016x}", i + 1, self.f[i + 1].to_bits()),
                format!("F{:2}=0x{:016x}", i + 2, self.f[i + 2].to_bits()),
                format!("F{:2}=0x{:016x}", i + 3, self.f[i + 3].to_bits()),
            );
        }
        if self.csr_cr3 != 0 {
            println!(
                "  MMU: CR3=0x{:016x} PRIV={} enabled={}",
                self.csr_cr3, self.priv_mode, self.mmu_enabled,
            );
        }
        println!("{:=>60}", "");
    }
}