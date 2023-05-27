use std::fmt::{self, Display, Write};

use crate::{instruction::AluType, Instruction};

use super::{utils::is_fp, AppointForm, Cdb, Register, SharedMemory};

// station id
// load-store: 1-5, integer: 6-8, fadd: 9-10, fmul: 11-12

#[derive(Debug, Clone, Copy)]
pub enum Source {
    Station(u8),
    Value(u32),
}

pub trait Station {
    fn try_send(
        &mut self,
        alu_op: u8,
        source1: Source,
        source2: Source,
        dest_reg: u8,
    ) -> Option<u8>;

    fn execute(&mut self, cdb: &mut Cdb);

    fn done(&self) -> bool;

    fn try_send_inst(
        &mut self,
        inst: Instruction,
        form: &mut AppointForm,
        regs: &Register,
        pc: u32,
    ) -> Option<u8> {
        let rs1 = inst.rs1();
        let mut source1 = match form.check(rs1) {
            0 => Source::Value(regs.get(rs1)),
            id => Source::Station(id),
        };

        let rs2 = inst.rs2();
        let mut source2 = match form.check(rs2) {
            0 => Source::Value(regs.get(rs2)),
            id => Source::Station(id),
        };

        let alu_op = inst.alu_op() as u8;

        if !inst.alu_use_reg1() {
            source1 = Source::Value(pc);
        }

        if !inst.alu_use_reg2() {
            source2 = Source::Value(inst.imm());
        }

        let res = self.try_send(alu_op, source1, source2, inst.rd() as u8);

        if let Some(id) = res {
            form.set(inst.rd(), id);
        }

        res
    }
}

impl<T: Station> Station for &mut T {
    fn try_send(
        &mut self,
        alu_op: u8,
        source1: Source,
        source2: Source,
        dest_reg: u8,
    ) -> Option<u8> {
        (*self).try_send(alu_op, source1, source2, dest_reg)
    }

    fn execute(&mut self, cdb: &mut Cdb) {
        (*self).execute(cdb)
    }

    fn done(&self) -> bool {
        (*self).done()
    }
}

#[derive(Debug, Clone)]
struct ReserveStation<const BUFFER_SIZE: usize> {
    buffer: [ReserveStationData; BUFFER_SIZE],
}

#[derive(Debug, Default, Clone, Copy)]
struct ReserveStationData {
    tag: u8, // busy: 0, executing: 1, alu_op: 2-5
    source1: Source,
    source2: Source,
    dest_reg: u8,
}

impl Default for Source {
    fn default() -> Self {
        Source::Value(0)
    }
}

impl Source {
    fn get_value(&self) -> Option<u32> {
        match self {
            Source::Station(_) => None,
            Source::Value(value) => Some(*value),
        }
    }
}

impl ReserveStationData {
    fn new(alu_op: u8, source1: Source, source2: Source, dest_reg: u8) -> Self {
        ReserveStationData {
            tag: (alu_op << 2) | 0b1,
            source1,
            source2,
            dest_reg,
        }
    }

    fn busy(&self) -> bool {
        self.tag & 0b1 == 0b1
    }

    fn executing(&self) -> bool {
        self.tag & 0b10 == 0b10
    }

    fn execute(&mut self) {
        self.tag |= 0b10;
    }

    fn clear(&mut self) {
        self.tag &= 0b11111100;
    }
}

impl<const BUFFER_SIZE: usize> Default for ReserveStation<BUFFER_SIZE> {
    fn default() -> Self {
        ReserveStation {
            buffer: [Default::default(); BUFFER_SIZE],
        }
    }
}

impl<const BUFFER_SIZE: usize> ReserveStation<BUFFER_SIZE> {
    fn try_send(
        &mut self,
        alu_op: u8,
        source1: Source,
        source2: Source,
        dest_reg: u8,
    ) -> Option<u8> {
        for (i, data) in self.buffer.iter_mut().enumerate() {
            if !data.busy() {
                *data = ReserveStationData::new(alu_op, source1, source2, dest_reg);
                return Some(i as u8);
            }
        }
        None
    }

    // return (id, alu_op, source1, source2, dest_reg)
    fn exec_source(&mut self, cdb: &Cdb) -> Vec<(u8, u8, u32, u32, u8)> {
        let mut result = Vec::new();

        for (i, data) in self.buffer.iter_mut().enumerate() {
            if !data.busy() || data.executing() {
                continue;
            }

            let mut is_ready = true;

            if let Source::Station(station_id) = data.source1 {
                if let Some(value) = cdb.get_station(station_id) {
                    data.source1 = Source::Value(value);
                } else {
                    is_ready = false;
                }
            }

            if let Source::Station(station_id) = data.source2 {
                if let Some(value) = cdb.get_station(station_id) {
                    data.source2 = Source::Value(value);
                } else {
                    is_ready = false;
                }
            }

            if is_ready {
                let source1 = data.source1.get_value().unwrap();
                let source2 = data.source2.get_value().unwrap();
                let alu_op = data.tag >> 2;
                let dest_reg = data.dest_reg;
                data.execute();
                result.push((i as u8, alu_op, source1, source2, dest_reg));
            }
        }

        result
    }

    fn done(&self) -> bool {
        self.buffer.iter().all(|data| !data.busy())
    }
}

#[derive(Debug, Default)]
pub struct IntegerStation(ReserveStation<3>);

impl Station for IntegerStation {
    fn try_send(
        &mut self,
        alu_op: u8,
        source1: Source,
        source2: Source,
        dest_reg: u8,
    ) -> Option<u8> {
        assert!(!is_fp(dest_reg), "IntegerStation: dest_reg is not integer");

        self.0
            .try_send(alu_op, source1, source2, dest_reg)
            .map(|id| id + 6)
    }

    fn execute(&mut self, cdb: &mut Cdb) {
        for (id, alu_op, source1, source2, dest) in self.0.exec_source(cdb) {
            let result = alu(source1, source2, AluType::from(alu_op as u32));
            cdb.send(id + 6, dest, result);
            self.0.buffer[id as usize].clear();
        }
    }

    fn done(&self) -> bool {
        self.0.done()
    }
}

#[derive(Debug, Default)]
pub struct FaddStation {
    reserve: ReserveStation<2>,
    result: [(u32, u8, u8); 2], // (result, dest_reg, remain_cycle)
}

#[derive(Debug, Default)]
pub struct FmulStation {
    reserve: ReserveStation<2>,
    result: [(u32, u8, u8); 2], // (result, dest_reg, remain_cycle)
}

impl Station for FaddStation {
    fn try_send(
        &mut self,
        alu_op: u8,
        source1: Source,
        source2: Source,
        dest_reg: u8,
    ) -> Option<u8> {
        assert!(
            is_fp(dest_reg),
            "FaddStation: dest_reg is not floating point"
        );

        self.reserve
            .try_send(alu_op, source1, source2, dest_reg)
            .map(|id| id + 9)
    }

    fn execute(&mut self, cdb: &mut Cdb) {
        for (i, data) in self.result.iter_mut().enumerate() {
            data.2 = data.2.saturating_sub(1);

            if data.2 == 1 {
                cdb.send(i as u8 + 9, data.1, data.0);
                self.reserve.buffer[i as usize].clear();
            }
        }

        for (id, alu_op, source1, source2, dest) in self.reserve.exec_source(cdb) {
            let result = fadd(source1, source2, alu_op);
            self.result[id as usize] = (result, dest, 3);
        }
    }

    fn done(&self) -> bool {
        self.reserve.done() && self.result.iter().all(|data| data.2 == 0)
    }
}

impl Station for FmulStation {
    fn try_send(
        &mut self,
        alu_op: u8,
        source1: Source,
        source2: Source,
        dest_reg: u8,
    ) -> Option<u8> {
        assert!(
            is_fp(dest_reg),
            "FmulStation: dest_reg is not floating point"
        );

        self.reserve
            .try_send(alu_op, source1, source2, dest_reg)
            .map(|id| id + 11)
    }

    fn execute(&mut self, cdb: &mut Cdb) {
        for (i, data) in self.result.iter_mut().enumerate() {
            data.2 = data.2.saturating_sub(1);

            if data.2 == 1 {
                cdb.send(i as u8 + 11, data.1, data.0);
                self.reserve.buffer[i as usize].clear();
            }
        }

        for (id, alu_op, source1, source2, dest) in self.reserve.exec_source(cdb) {
            let result = fmul(source1, source2, alu_op);
            let remain = match alu_op {
                0 => 10,
                1 => 40,
                _ => unreachable!(),
            };
            self.result[id as usize] = (result, dest, remain + 1);
        }
    }

    fn done(&self) -> bool {
        self.reserve.done() && self.result.iter().all(|data| data.2 == 0)
    }
}

pub struct LDStation {
    mem: SharedMemory,
    reserve: ReserveStation<5>,
    result: [(u32, u8, u8); 5], // (result, dest_reg, remain_cycle)
}

impl LDStation {
    pub fn new(mem: SharedMemory) -> Self {
        Self {
            mem,
            reserve: Default::default(),
            result: [(0, 0, 0); 5],
        }
    }
}

impl Station for LDStation {
    fn try_send(
        &mut self,
        mem_op: u8,
        source1: Source,
        source2: Source,
        dest_reg: u8,
    ) -> Option<u8> {
        assert!(
            matches!(source2, Source::Value(_)),
            "LDStation: source2 is not value"
        );

        self.reserve
            .try_send(mem_op, source1, source2, dest_reg)
            .map(|id| id + 1)
    }

    fn execute(&mut self, cdb: &mut Cdb) {
        for (i, data) in self.result.iter_mut().enumerate() {
            data.2 = data.2.saturating_sub(1);

            if data.2 == 1 {
                cdb.send(i as u8 + 1, data.1, data.0);
                self.reserve.buffer[i as usize].clear();
            }
        }

        for (id, mem_op, source1, source2, dest) in self.reserve.exec_source(cdb) {
            let addr = source1.wrapping_add(source2);

            match mem_op {
                0 => {
                    let result = self.mem.load(addr);
                    self.result[id as usize] = (result, dest, 2);
                }
                1 => {
                    // complete store part
                    todo!()
                }
                _ => unreachable!(),
            }
        }
    }

    fn done(&self) -> bool {
        self.reserve.done() && self.result.iter().all(|data| data.2 == 0)
    }
}

impl Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = String::with_capacity(10);
        match self {
            Source::Value(v) => write!(buf, "{:08x}", v)?,
            Source::Station(r) => write!(buf, "Stat {}", r)?,
        };
        write!(f, "{:<8}", buf)
    }
}

impl Display for ReserveStationData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tag: {:08b}, source1: {}, source2: {}, dest_reg: {}",
            self.tag, self.source1, self.source2, self.dest_reg
        )?;
        Ok(())
    }
}

impl Display for LDStation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "LDStation:")?;
        for (i, data) in self.reserve.buffer.iter().enumerate() {
            writeln!(f, "    {:>2}: {}", i + 1, data)?;
        }
        Ok(())
    }
}

impl Display for IntegerStation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "IntegerStation:")?;
        for (i, data) in self.0.buffer.iter().enumerate() {
            writeln!(f, "    {:>2}: {}", i + 6, data)?;
        }
        Ok(())
    }
}

impl Display for FaddStation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "FaddStation:")?;
        for (i, data) in self.reserve.buffer.iter().enumerate() {
            writeln!(f, "    {:>2}: {}", i + 9, data)?;
        }
        Ok(())
    }
}

impl Display for FmulStation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "FmulStation:")?;
        for (i, data) in self.reserve.buffer.iter().enumerate() {
            writeln!(f, "    {:>2}: {}", i + 11, data)?;
        }
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

fn fadd(a: u32, b: u32, op: u8) -> u32 {
    let a = f32::from_bits(a);
    let b = f32::from_bits(b);
    let result = match op {
        0 => a + b,
        1 => a - b,
        _ => unreachable!(),
    };
    result.to_bits()
}

fn fmul(a: u32, b: u32, op: u8) -> u32 {
    let a = f32::from_bits(a);
    let b = f32::from_bits(b);
    let result = match op {
        0 => a * b,
        1 => a / b,
        _ => unreachable!(),
    };
    result.to_bits()
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
    fn test_fadd() {
        assert_eq!(fadd(0x400c_0000, 0x3f88_0000, 0), 0x4050_0000); // 2.1875 + 1.0625 = 3.25
        assert_eq!(fadd(0x400c_0000, 0x3f88_0000, 1), 0x3f90_0000); // 2.1875 - 1.0625 = 1.125
    }

    #[test]
    fn test_fmul() {
        assert_eq!(fmul(0x4024_0000, 0x4000_0000, 0), 0x40a4_0000); // 2.5625 * 2.0 = 5.125
        assert_eq!(fmul(0x4024_0000, 0x4000_0000, 1), 0x3fa4_0000); // 2.5625 / 2.0 = 1.28125
    }
}
