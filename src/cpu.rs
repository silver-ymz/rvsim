use super::{
    assembler::Program,
    instruction::{AluType, Instruction, MemType, WBType},
};
use nu_ansi_term::Color::Blue;
use std::{
    collections::HashMap,
    fmt::{self, Display},
    ops::Index,
};

#[derive(Default)]
pub struct CpuState {
    if_id: TempState,
    id_ex: TempState,
    ex_mem: TempState,
    mem_wb: TempState,
    regs: Register,
    mem: Memory,
    pc: u32,
    npc: u32,
    inst_name: HashMap<u32, String>,
    stall: bool,
    cycle: u32,
    data_hazard: u32,
    control_hazard: u32,
    exit: bool,
}

#[derive(Default)]
struct TempState {
    pc: u32,
    npc: u32,
    ir: Instruction,
    imm_a: u32,
    imm_b: u32,
    imm_src: u32,
    cond: bool,
    alu_out: u32,
    mem_out: u32,
    write_out: u32,
}

struct Memory {
    data: [u32; 1024 * 8], // 32KB
}

struct Register {
    regs: [u32; 32],
}

pub enum RunState {
    Running,
    Exit(u32),
    Break,
}

impl CpuState {
    fn if_cycle(&mut self) -> Result<(), String> {
        if self.ex_mem.cond {
            self.npc = self.ex_mem.alu_out;
        }

        if self.id_ex.ir.is_jump() || self.exit {
            self.if_id.ir = Instruction::nop();
            return Ok(());
        } else if !self.stall {
            self.if_id.ir = Instruction::from_binary(self.mem.load(self.npc)).unwrap();
        }

        if self.if_id.ir.is_ecall() {
            self.exit = true;
        }

        if !self.stall {
            self.if_id.npc = self.npc + 4;
            self.if_id.pc = self.npc;
            self.npc = self.if_id.npc;
        }

        Ok(())
    }

    fn id_cycle(&mut self) {
        self.stall = false;

        // data hazard
        if self.id_ex.ir.is_load()
            && (self.id_ex.ir.rd() == self.if_id.ir.rs1()
                || self.id_ex.ir.rd() == self.if_id.ir.rs2())
        {
            self.stall = true;
            self.data_hazard += 1;
        }

        if self.stall {
            self.id_ex.ir = Instruction::nop();
            self.id_ex.pc = self.if_id.pc;
            self.id_ex.npc = self.if_id.npc;
            self.id_ex.imm_a = 0;
            self.id_ex.imm_b = 0;
            self.id_ex.imm_src = 0;
            return;
        }

        self.id_ex.pc = self.if_id.pc;
        self.id_ex.npc = self.if_id.npc;
        self.id_ex.ir = self.if_id.ir.clone();
        self.id_ex.imm_a = self.regs[self.if_id.ir.rs1()];
        self.id_ex.imm_b = self.regs[self.if_id.ir.rs2()];
        self.id_ex.imm_src = self.if_id.ir.imm();

        // control hazard
        if self.id_ex.ir.is_jump() {
            self.stall = true;
            self.control_hazard += 1;
        }
    }

    fn ex_cycle(&mut self) {
        self.ex_mem.pc = self.id_ex.pc;
        self.ex_mem.npc = self.id_ex.npc;
        self.ex_mem.ir = self.id_ex.ir.clone();
        self.ex_mem.imm_a = self.id_ex.imm_a;
        self.ex_mem.imm_b = self.id_ex.imm_b;
        self.ex_mem.imm_src = self.id_ex.imm_src;

        let alu_in_a = if self.id_ex.ir.alu_use_reg1() {
            self.id_ex.imm_a
        } else {
            self.id_ex.pc
        };

        let alu_in_b = if self.id_ex.ir.alu_use_reg2() {
            self.id_ex.imm_b
        } else {
            self.id_ex.imm_src
        };

        self.ex_mem.alu_out = alu(alu_in_a, alu_in_b, self.id_ex.ir.alu_op());
        self.ex_mem.cond = self.id_ex.ir.branch(self.id_ex.imm_a, self.id_ex.imm_b);
    }

    fn mem_cycle(&mut self) {
        self.mem_wb.pc = self.ex_mem.pc;
        self.mem_wb.npc = self.ex_mem.npc;
        self.mem_wb.ir = self.ex_mem.ir.clone();
        self.mem_wb.imm_a = self.ex_mem.imm_a;
        self.mem_wb.imm_b = self.ex_mem.imm_b;
        self.mem_wb.imm_src = self.ex_mem.imm_src;
        self.mem_wb.alu_out = self.ex_mem.alu_out;
        self.mem_wb.cond = self.ex_mem.cond;

        if self.ex_mem.cond && self.ex_mem.ir.is_jump() {
            self.mem_wb.npc = self.ex_mem.alu_out;
        }

        match self.ex_mem.ir.mem_op() {
            MemType::Load => {
                self.mem_wb.mem_out = self.mem.load(self.ex_mem.alu_out);
            }
            MemType::Store => {
                self.mem.store(self.ex_mem.alu_out, self.ex_mem.imm_b);
                self.mem_wb.mem_out = 0;
            }
            MemType::None => {
                self.mem_wb.mem_out = 0;
            }
        }

        self.mem_wb.write_out = match self.mem_wb.ir.write_back() {
            WBType::Mem => self.mem_wb.mem_out,
            WBType::Alu => self.ex_mem.alu_out,
            WBType::Pc => self.ex_mem.pc + 4,
            WBType::None => 0,
        };

        // data forwarding.
        if self.ex_mem.ir.rd() == self.id_ex.ir.rs1() && self.ex_mem.ir.reg_write() {
            self.id_ex.imm_a = self.mem_wb.write_out;
        }
        if self.ex_mem.ir.rd() == self.id_ex.ir.rs2() && self.ex_mem.ir.reg_write() {
            self.id_ex.imm_b = self.mem_wb.write_out;
        }
    }

    fn wb_cycle(&mut self) -> Result<RunState, String> {
        if self.mem_wb.ir.reg_write() {
            self.regs.set(self.mem_wb.ir.rd(), self.mem_wb.write_out);
        }

        if !self.mem_wb.ir.is_nop() {
            self.pc = self.mem_wb.npc;
        }

        // data forwarding.
        if self.mem_wb.ir.rd() == self.id_ex.ir.rs1() && self.mem_wb.ir.reg_write() {
            self.id_ex.imm_a = self.mem_wb.write_out;
        }
        if self.mem_wb.ir.rd() == self.id_ex.ir.rs2() && self.mem_wb.ir.reg_write() {
            self.id_ex.imm_b = self.mem_wb.write_out;
        }

        if self.mem_wb.ir.is_ebreak() {
            Ok(RunState::Break)
        } else if self.mem_wb.ir.is_ecall() {
            if self.regs[10] == 17 {
                Ok(RunState::Exit(self.regs[11]))
            } else {
                return Err("unknown ecall".to_string());
            }
        } else {
            Ok(RunState::Running)
        }
    }

    pub fn step(&mut self) -> Result<RunState, String> {
        let mut state = RunState::Running;

        if self.cycle > 3 {
            state = self.wb_cycle()?;
        }
        if self.cycle > 2 {
            self.mem_cycle();
        }
        if self.cycle > 1 {
            self.ex_cycle();
        }
        if self.cycle > 0 {
            self.id_cycle();
        }
        self.if_cycle()?;

        self.cycle += 1;

        if self.cycle > 10000 {
            return Err("too many cycles".to_string());
        }

        Ok(state)
    }

    pub fn load(&mut self, program: &Program) {
        self.mem.load_mem(program.mem());
        self.inst_name = program.inst_name().clone();
        self.npc = program.entry();
        self.pc = program.entry();
    }

    pub fn cycle(&self) -> u32 {
        self.cycle
    }

    pub fn data_hazard(&self) -> u32 {
        self.data_hazard
    }

    pub fn control_hazard(&self) -> u32 {
        self.control_hazard
    }
}

impl Display for CpuState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "========== {} ==========",
            Blue.paint(format!("Cycle {}", self.cycle))
        )?;
        writeln!(f, "-- cpu state")?;
        writeln!(
            f,
            "pc: {:08x}, npc: {:08x}, stall: {}, inst: {}, next_pipein_inst: {}",
            self.pc,
            self.npc,
            self.stall,
            Blue.paint(self.inst_name.get(&self.pc).unwrap_or(&"???".to_owned())),
            Blue.paint(self.inst_name.get(&self.npc).unwrap_or(&"???".to_owned()))
        )?;
        write!(f, "{}", self.regs)?;
        writeln!(f, "-- IF/ID")?;
        write!(f, "{}", self.if_id)?;
        writeln!(f, "-- ID/EX")?;
        write!(f, "{}", self.id_ex)?;
        writeln!(f, "-- EX/MEM")?;
        write!(f, "{}", self.ex_mem)?;
        writeln!(f, "-- MEM/WB")?;
        write!(f, "{}", self.mem_wb)?;

        Ok(())
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            data: [0; 1024 * 8],
        }
    }
}

impl Memory {
    fn load(&self, addr: u32) -> u32 {
        self.data[(addr / 4) as usize]
    }

    fn store(&mut self, addr: u32, data: u32) {
        self.data[(addr / 4) as usize] = data;
    }

    fn load_mem(&mut self, data: &Vec<u32>) {
        let mut mem = [0; 1024 * 8];
        for (i, d) in data.iter().enumerate() {
            mem[i] = *d;
        }
        self.data = mem;
    }
}

impl Default for Register {
    fn default() -> Self {
        let mut regs = [0; 32];
        regs[2] = 0x7ffc; // stack point begin with 0x7ffc
        Self { regs }
    }
}

impl Index<u32> for Register {
    type Output = u32;

    // Because we have limit write operation to x0,
    // we can ignore dealing with x0 here.
    fn index(&self, index: u32) -> &Self::Output {
        &self.regs[index as usize]
    }
}

impl Register {
    pub fn set(&mut self, index: u32, value: u32) {
        self.regs[index as usize] = value;
    }
}

impl Display for Register {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 0..4 {
            for j in 0..8 {
                let index = i * 8 + j;
                write!(
                    f,
                    "{:>3}: {:08x}, ",
                    format!("x{}", index),
                    self.regs[index]
                )?;
            }
            writeln!(f, "")?;
        }
        Ok(())
    }
}

impl Display for TempState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ir: {}, ", Blue.paint(self.ir.to_string()))?;
        write!(f, "pc: {:08x}, ", self.pc)?;
        write!(f, "npc: {:08x}, ", self.npc)?;
        write!(f, "imm_a: {:08x}, ", self.imm_a)?;
        write!(f, "imm_b: {:08x}, ", self.imm_b)?;
        writeln!(f, "imm_src: {:08x}", self.imm_src)?;
        write!(f, "cond: {}, ", self.cond)?;
        write!(f, "alu_out: {:08x}, ", self.alu_out)?;
        write!(f, "mem_out: {:08x}, ", self.mem_out)?;
        writeln!(f, "write_out: {:08x}", self.write_out)?;
        Ok(())
    }
}

fn alu(a: u32, b: u32, op: AluType) -> u32 {
    match op {
        AluType::Add => a.wrapping_add(b),
        AluType::Sub => a.wrapping_sub(b),
        AluType::And => a & b,
        AluType::Or => a | b,
        AluType::Xor => a ^ b,
        AluType::Sll => a << b,
        AluType::Srl => a >> b,
        AluType::Sra => (a as i32 >> b) as u32,
        AluType::Slt => ((a as i32) < (b as i32)) as u32,
        AluType::Sltu => (a < b) as u32,
        AluType::Mul => a.wrapping_mul(b),
        AluType::Mulh => ((a as i32 as i64).wrapping_mul(b as i32 as i64) >> 32) as u32,
        AluType::Mulhu => ((a as u64).wrapping_mul(b as u64) >> 32) as u32,
        AluType::Bsel => b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alu() {
        assert_eq!(alu(1, 2, AluType::Add), 3);
        assert_eq!(alu(1, 2, AluType::Sub), 0xffff_ffff);
        assert_eq!(alu(1, 2, AluType::And), 0);
        assert_eq!(alu(1, 2, AluType::Or), 3);
        assert_eq!(alu(1, 2, AluType::Xor), 3);
        assert_eq!(alu(1, 2, AluType::Sll), 4);
        assert_eq!(alu(1, 2, AluType::Srl), 0);
        assert_eq!(alu(1, 2, AluType::Sra), 0);
        assert_eq!(alu(1, 2, AluType::Slt), 1);
        assert_eq!(alu(1, 2, AluType::Sltu), 1);
        assert_eq!(alu(1, 2, AluType::Mul), 2);
        assert_eq!(alu(0x7fff_ffff, 4, AluType::Mulh), 1);
        assert_eq!(alu(0x7fff_ffff, 4, AluType::Mulhu), 1);
        assert_eq!(alu(1, 2, AluType::Bsel), 2);
    }

    #[test]
    fn test_alu_signed() {
        assert_eq!(alu(0xffff_ffff, 1, AluType::Add), 0);
        assert_eq!(alu(0xffff_ffff, 1, AluType::Sub), 0xffff_fffe);
        assert_eq!(alu(0xffff_ffff, 1, AluType::Sll), 0xffff_fffe);
        assert_eq!(alu(0xffff_ffff, 1, AluType::Srl), 0x7fff_ffff);
        assert_eq!(alu(0xffff_ffff, 1, AluType::Sra), 0xffff_ffff);
        assert_eq!(alu(0xffff_ffff, 1, AluType::Slt), 1);
        assert_eq!(alu(0xffff_ffff, 1, AluType::Sltu), 0);
        assert_eq!(alu(0xffff_ffff, 2, AluType::Mul), 0xffff_fffe);
        assert_eq!(alu(0xffff_ffff, 2, AluType::Mulh), 0xffff_ffff);
        assert_eq!(alu(0xffff_ffff, 2, AluType::Mulhu), 1);
        assert_eq!(alu(0xffff_ffff, 1, AluType::Bsel), 1);
    }

    #[test]
    fn test_step() {
        let test_str = r"
        .globl main
        .text
        main:
        addi x1, x0, 1
        addi x2, x0, 2
        addi x3, x0, 3
        addi x4, x0, 4
        addi x5, x0, 5
        addi a0, x0, 17
        ecall
        ";
        let mut cpu = CpuState::default();
        let program = Program::from_buffer(test_str.as_bytes()).unwrap();
        cpu.load(&program);
        cpu.step().unwrap();
    }
}
