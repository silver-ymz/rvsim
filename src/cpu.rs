use std::{collections::HashMap, ops::Index};

use crate::instruction::{MemType, WBType};

use super::{
    assembler::Program,
    instruction::{AluType, Instruction},
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
    inst_name: HashMap<u32, String>,
}

#[derive(Default)]
struct TempState {
    npc: u32,
    ir: Instruction,
    imm_a: u32,
    imm_b: u32,
    imm_src: u32,
    cond: bool,
    alu_out: u32,
    mem_out: u32,
}

struct Memory {
    data: [u32; 1024 * 8], // 32KB
}

struct Register {
    regs: [u32; 32],
}

impl CpuState {
    fn if_cycle(&mut self) -> Result<(), String> {
        self.if_id.ir = Instruction::from_binary(self.mem.load(self.pc))?;
        if self.ex_mem.cond && self.ex_mem.ir.is_cond() {
            self.if_id.npc = self.ex_mem.alu_out;
        } else {
            self.if_id.npc = self.pc + 4;
        }
        self.pc = self.if_id.npc;

        Ok(())
    }

    fn id_cycle(&mut self) {
        self.id_ex.npc = self.if_id.npc;
        self.id_ex.ir = self.if_id.ir.clone();
        self.id_ex.imm_a = self.regs[self.if_id.ir.rs1()];
        self.id_ex.imm_b = self.regs[self.if_id.ir.rs2()];
        self.id_ex.imm_src = self.if_id.ir.imm();
    }

    fn ex_cycle(&mut self) {
        self.ex_mem.npc = self.id_ex.npc;
        self.ex_mem.ir = self.id_ex.ir.clone();
        self.ex_mem.imm_a = self.id_ex.imm_a;
        self.ex_mem.imm_b = self.id_ex.imm_b;
        self.ex_mem.imm_src = self.id_ex.imm_src;

        let alu_in_a = if self.id_ex.ir.alu_use_reg1() {
            self.id_ex.imm_a
        } else {
            self.pc
        };

        let alu_in_b = if self.id_ex.ir.alu_use_reg2() {
            self.id_ex.imm_b
        } else {
            self.id_ex.imm_src
        };

        self.ex_mem.alu_out = alu(alu_in_a, alu_in_b, self.id_ex.ir.alu_op());
        self.ex_mem.cond = self.id_ex.ir.branch(self.ex_mem.imm_a, self.ex_mem.imm_b);
    }

    fn mem_cycle(&mut self) {
        self.mem_wb.npc = self.ex_mem.npc;
        self.mem_wb.ir = self.ex_mem.ir.clone();
        self.mem_wb.imm_a = self.ex_mem.imm_a;
        self.mem_wb.imm_b = self.ex_mem.imm_b;
        self.mem_wb.imm_src = self.ex_mem.imm_src;
        self.mem_wb.alu_out = self.ex_mem.alu_out;

        match self.ex_mem.ir.mem_op() {
            MemType::Load => {
                self.mem_wb.mem_out = self.mem.load(self.ex_mem.alu_out);
            }
            MemType::Store => {
                self.mem.store(self.ex_mem.alu_out, self.ex_mem.imm_b);
            }
            MemType::None => {}
        }
    }

    fn wb_cycle(&mut self) {
        match self.mem_wb.ir.write_back() {
            WBType::Mem => self.regs.set(self.mem_wb.ir.rd(), self.mem_wb.mem_out),
            WBType::Alu => self.regs.set(self.mem_wb.ir.rd(), self.mem_wb.alu_out),
            WBType::Pc => self.regs.set(self.mem_wb.ir.rd(), self.mem_wb.npc),
            WBType::None => {}
        }
    }

    pub fn step(&mut self) -> Result<(), String> {
        self.if_cycle()?;
        self.id_cycle();
        self.ex_cycle();
        self.mem_cycle();
        self.wb_cycle();

        Ok(())
    }

    pub fn load(&mut self, program: &Program) {
        self.mem.load_mem(program.mem());
        self.inst_name = program.inst_name().clone();
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
        regs[29] = 0x7ffc; // stack point begin with 0x7ffc
        Self { regs }
    }
}

impl Index<u32> for Register {
    type Output = u32;

    fn index(&self, index: u32) -> &Self::Output {
        &self.regs[index as usize]
    }
}

impl Register {
    pub fn set(&mut self, index: u32, value: u32) {
        if index == 0 {
            return;
        }

        self.regs[index as usize] = value;
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
        let test_str = r".text
        addi x1, x0, 1
        addi x2, x0, 2
        addi x3, x0, 3
        addi x4, x0, 4
        addi x5, x0, 5
        ";
        let mut cpu = CpuState::default();
        let program = Program::from_buffer(test_str.as_bytes()).unwrap();
        cpu.load(&program);
        cpu.step().unwrap();
    }
}
