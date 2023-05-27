use super::{
    assembler::Program,
    instruction::{Instruction, StationType},
};

use nu_ansi_term::Color::Blue;

use std::{
    cell::UnsafeCell,
    collections::{HashMap, VecDeque},
    fmt::{self, Display},
    rc::Rc,
};

pub use cdb::Cdb;
pub use station::{FaddStation, FmulStation, IntegerStation, LDStation, Station};

mod cdb;
mod station;
mod utils;

pub struct CpuState {
    ld_station: LDStation,
    integer_station: IntegerStation,
    fadd_station: FaddStation,
    fmul_station: FmulStation,
    form: AppointForm,
    wait_insts: VecDeque<Instruction>,
    cdb: Cdb,
    regs: Register,
    mem: SharedMemory,
    pc: u32,
    inst_name: HashMap<u32, String>,
    cycle: u32,
    exit: bool,
}

struct Memory {
    data: [u32; 1024 * 8], // 32KB
}

#[derive(Default, Clone)]
pub struct SharedMemory(Rc<UnsafeCell<Memory>>);

pub struct Register {
    regs: [u32; 64], // 0-31 is int, 32-63 is float
}

pub struct AppointForm {
    map: [u8; 64],
}

#[derive(Debug, Clone, Copy)]
pub enum RunState {
    Running,
    Exit(u32),
    Break,
}

impl CpuState {
    pub fn new() -> Self {
        let mem = SharedMemory::default();

        Self {
            ld_station: LDStation::new(mem.clone()),
            integer_station: IntegerStation::default(),
            fadd_station: FaddStation::default(),
            fmul_station: FmulStation::default(),
            form: AppointForm::default(),
            wait_insts: VecDeque::with_capacity(10),
            cdb: Cdb::default(),
            regs: Register::default(),
            mem,
            pc: 0,
            inst_name: HashMap::new(),
            cycle: 0,
            exit: false,
        }
    }

    pub fn step(&mut self) -> Result<RunState, String> {
        let mut state = RunState::Running;

        self.write_back();
        self.execute();

        if !self.exit {
            state = self.issue()?;
            self.pc += 4;
        }

        if self.exit
            && self.ld_station.done()
            && self.integer_station.done()
            && self.fadd_station.done()
            && self.fmul_station.done()
            && self.wait_insts.is_empty()
        {
            state = RunState::Exit(self.regs.get(11));
        }

        self.cycle += 1;

        if self.cycle > 10000 {
            return Err("too many cycles".to_string());
        }

        Ok(state)
    }

    pub fn issue(&mut self) -> Result<RunState, String> {
        let inst = Instruction::from_binary(self.mem.load(self.pc))?;
        if inst.station() == StationType::None {
            self.exit = true;
            return Ok(RunState::Running);
        }

        self.wait_insts.push_back(inst);

        for _ in 0..self.wait_insts.len() {
            let inst = self.wait_insts.pop_front().unwrap();

            let mut station: Box<dyn Station> = match inst.station() {
                StationType::LoadStore => Box::new(&mut self.ld_station),
                StationType::Integer => Box::new(&mut self.integer_station),
                StationType::FAdd => Box::new(&mut self.fadd_station),
                StationType::FMul => Box::new(&mut self.fmul_station),
                _ => unimplemented!(),
            };

            match station.try_send_inst(inst.clone(), &mut self.form, &self.regs, self.pc) {
                Some(id) => self.form.set(inst.rd(), id),
                None => self.wait_insts.push_back(inst),
            }
        }

        Ok(RunState::Running)
    }

    pub fn execute(&mut self) {
        self.ld_station.execute(&mut self.cdb);
        self.integer_station.execute(&mut self.cdb);
        self.fadd_station.execute(&mut self.cdb);
        self.fmul_station.execute(&mut self.cdb);
        self.cdb.exec();
    }

    pub fn write_back(&mut self) {
        for i in 0..64 {
            let id = self.form.check(i);
            if id != 0 {
                if let Some(val) = self.cdb.get_station(id) {
                    self.regs.set(i, val);
                    self.form.clear(i);
                } else if let Some(val) = self.cdb.get_reg(i as u8) {
                    self.regs.set(i, val);
                }
            }
        }
    }

    pub fn load(&mut self, program: &Program) {
        self.mem.load_mem(program.mem());
        self.inst_name = program.inst_name().clone();
        self.pc = program.entry();
    }

    pub fn cycle(&self) -> u32 {
        self.cycle
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
        writeln!(f, "pc: {}", self.pc)?;
        writeln!(f, "-- registers")?;
        write!(f, "{}", self.regs)?;
        //writeln!(f, "mem: {:?}", self.mem)?;
        writeln!(f, "-- station state")?;
        write!(f, "load-store station: {}", self.ld_station)?;
        write!(f, "integer station: {}", self.integer_station)?;
        write!(f, "fadd station: {}", self.fadd_station)?;
        write!(f, "fmul station: {}", self.fmul_station)?;
        write!(f, "cdb: {}", self.cdb)?;
        writeln!(f, "-- appoint form")?;
        writeln!(f, "{}", self.form)?;
        writeln!(f, "-- wait inst")?;

        let mut empty = true;
        for inst in self.wait_insts.iter() {
            writeln!(f, "{}", inst)?;
            empty = false;
        }
        if empty {
            writeln!(f, "empty")?;
        }

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

// safety: Because we use the memory in a reference count, we can ensure the memory won't
//         be droped before using.
impl SharedMemory {
    fn load(&self, addr: u32) -> u32 {
        let mem = unsafe { &*self.0.get() };
        mem.data[(addr / 4) as usize]
    }

    fn store(&mut self, addr: u32, data: u32) {
        let mem = unsafe { &mut *self.0.get() };
        mem.data[(addr / 4) as usize] = data;
    }

    fn load_mem(&mut self, data: &Vec<u32>) {
        let mut new_mem = [0; 1024 * 8];
        for (i, d) in data.iter().enumerate() {
            new_mem[i] = *d;
        }
        let mem = unsafe { &mut *self.0.get() };
        mem.data = new_mem;
    }
}

impl Default for Register {
    fn default() -> Self {
        let mut regs = [0; 64];
        regs[2] = 0x7ffc; // stack point begin with 0x7ffc
        Self { regs }
    }
}

impl Register {
    // Because we have limit write operation to x0,
    // we can ignore dealing with x0 here.
    pub fn get(&self, index: u32) -> u32 {
        self.regs[index as usize]
    }

    pub fn set(&mut self, index: u32, value: u32) {
        self.regs[index as usize] = value;
    }
}

impl Default for AppointForm {
    fn default() -> Self {
        Self { map: [0; 64] }
    }
}

impl AppointForm {
    pub fn check(&self, index: u32) -> u8 {
        self.map[index as usize]
    }

    pub fn set(&mut self, index: u32, value: u8) {
        self.map[index as usize] = value;
    }

    pub fn clear(&mut self, index: u32) {
        self.map[index as usize] = 0;
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
        //writeln!(f, "")?;
        for i in 0..8 {
            for j in 0..4 {
                let index = i * 4 + j;
                write!(
                    f,
                    "{:>3}: {:08x}({:.4}), ",
                    format!("f{}", index),
                    self.regs[32 + index],
                    f32::from_bits(self.regs[32 + index])
                )?;
            }
            writeln!(f, "")?;
        }
        Ok(())
    }
}

impl Display for AppointForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 0..4 {
            for j in 0..8 {
                let index = i * 8 + j;
                write!(f, "{:>3}: {:02}, ", format!("x{}", index), self.map[index])?;
            }
            writeln!(f, "")?;
        }
        //writeln!(f, "")?;
        for i in 0..4 {
            for j in 0..8 {
                let index = i * 8 + j;
                write!(
                    f,
                    "{:>3}: {:02}, ",
                    format!("f{}", index),
                    self.map[32 + index]
                )?;
            }
            writeln!(f, "")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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