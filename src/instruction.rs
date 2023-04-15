#[derive(Clone)]
pub struct Instruction {
    binary: u32,
    inst_type: InstType,
}

#[derive(Clone, PartialEq, Debug)]
enum InstType {
    R,
    I,
    S,
    B,
    U,
    J,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum AluType {
    Add = 0,
    Sll = 1,
    Slt = 2,
    Sltu = 3,
    Xor = 4,
    Srl = 5,
    Or = 6,
    And = 7,
    Mul = 8,
    Mulh = 9,
    Mulhu = 11,
    Sub = 12,
    Sra = 13,
    Bsel = 15,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum WBType {
    Mem,
    Alu,
    Pc,
    None,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum MemType {
    Load,
    Store,
    None,
}

impl Instruction {
    pub fn nop() -> Self {
        Self::from_binary(0x00000033).unwrap() // add x0, x0, x0
    }

    pub fn from_binary(binary: u32) -> Result<Self, String> {
        let inst_type = match binary & 0x7f {
            0x33 => InstType::R,
            0x03 | 0x13 | 0x67 => InstType::I,
            0x23 => InstType::S,
            0x63 => InstType::B,
            0x37 | 0x17 => InstType::U,
            0x6f => InstType::J,
            _ => return Err(format!("Invalid instruction: {:08x}", binary)),
        };

        Ok(Self { binary, inst_type })
    }

    pub fn is_cond(&self) -> bool {
        matches!(self.inst_type, InstType::B)
    }

    pub fn rs1(&self) -> u32 {
        (self.binary >> 15) & 0x1f
    }

    pub fn rs2(&self) -> u32 {
        (self.binary >> 20) & 0x1f
    }

    pub fn rd(&self) -> u32 {
        (self.binary >> 7) & 0x1f
    }

    pub fn imm(&self) -> u32 {
        match self.inst_type {
            InstType::I => (self.binary >> 20) as i32 as u32,
            InstType::S => ((self.binary >> 7) & 0x1f) | ((self.binary >> 20) & 0xfe0),
            InstType::B => {
                (((self.binary >> 8) & 0xf) << 1)
                    | ((self.binary >> 20) & 0x7e0)
                    | ((self.binary << 4) & 0x800)
                    | ((self.binary >> 19) & 0x1000)
            }
            InstType::U => self.binary & 0xfffff000,
            InstType::J => {
                (((self.binary >> 21) & 0x3ff) << 1)
                    | ((self.binary >> 9) & 0x800)
                    | (self.binary & 0xff000)
                    | ((self.binary >> 11) & 0x100000)
            }
            _ => 0,
        }
    }

    pub fn alu_use_reg1(&self) -> bool {
        matches!(self.inst_type, InstType::R | InstType::I | InstType::S)
    }

    pub fn alu_use_reg2(&self) -> bool {
        matches!(self.inst_type, InstType::R)
    }

    pub(crate) fn alu_op(&self) -> AluType {
        if self.inst_type == InstType::U {
            (((self.binary >> 5) & 0x1) * 0xf).into()
        } else {
            ((((self.binary >> 12) & 0x7)
                | (((self.binary >> 25) & 0x1) << 3)
                | (((self.binary >> 30) & 0x1) * 0xc))
                & (((self.binary >> 4) & 0x1) * 0xf))
                .into()
        }
    }

    pub(crate) fn write_back(&self) -> WBType {
        match self.inst_type {
            InstType::I => {
                if (self.binary & 0x7f) == 0x3 {
                    WBType::Mem
                } else if (self.binary & 0x7f) == 0x67 {
                    WBType::Pc
                } else {
                    WBType::Alu
                }
            }
            InstType::R | InstType::U => WBType::Alu,
            InstType::J => WBType::Pc,
            _ => WBType::None,
        }
    }

    pub fn branch(&self, a: u32, b: u32) -> bool {
        match self.inst_type {
            InstType::B => {
                let a = a as i32;
                let b = b as i32;
                match (self.binary >> 12) & 0x7 {
                    0 => a == b,
                    1 => a != b,
                    4 => (a as i32) < (b as i32),
                    5 => (a as i32) >= (b as i32),
                    6 => a < b,
                    7 => a >= b,
                    _ => false,
                }
            }
            _ => false,
        }
    }

    pub(crate) fn mem_op(&self) -> MemType {
        match self.inst_type {
            InstType::I => {
                if (self.binary & 0x7f) == 0x3 {
                    MemType::Load
                } else {
                    MemType::None
                }
            }
            InstType::S => MemType::Store,
            _ => MemType::None,
        }
    }
}

impl Default for Instruction {
    fn default() -> Self {
        Self::nop()
    }
}

impl From<u32> for AluType {
    fn from(value: u32) -> Self {
        match value {
            0 => AluType::Add,
            1 => AluType::Sll,
            2 => AluType::Slt,
            3 => AluType::Sltu,
            4 => AluType::Xor,
            5 => AluType::Srl,
            6 => AluType::Or,
            7 => AluType::And,
            8 => AluType::Mul,
            9 => AluType::Mulh,
            11 => AluType::Mulhu,
            12 => AluType::Sub,
            13 => AluType::Sra,
            15 => AluType::Bsel,
            _ => panic!("Invalid alu type: {}", value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_binary() {
        let inst = Instruction::from_binary(0x00000033).unwrap(); // add x0, x0, x0
        assert_eq!(inst.binary, 0x00000033);
        assert_eq!(inst.inst_type, InstType::R);
        assert_eq!(inst.is_cond(), false);
        assert_eq!(inst.rs1(), 0);
        assert_eq!(inst.rs2(), 0);
        assert_eq!(inst.rd(), 0);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), true);
        assert_eq!(inst.alu_use_reg2(), true);
        assert_eq!(inst.alu_op(), AluType::Add);
        assert_eq!(inst.mem_op(), MemType::None);
        assert_eq!(inst.write_back(), WBType::Alu);
        assert_eq!(inst.branch(0, 0), false);

        let inst = Instruction::from_binary(0x00000013).unwrap(); // addi x0, x0, 0
        assert_eq!(inst.binary, 0x00000013);
        assert_eq!(inst.inst_type, InstType::I);
        assert_eq!(inst.is_cond(), false);
        assert_eq!(inst.rs1(), 0);
        assert_eq!(inst.rs2(), 0);
        assert_eq!(inst.rd(), 0);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), true);
        assert_eq!(inst.alu_use_reg2(), false);
        assert_eq!(inst.alu_op(), AluType::Add);
        assert_eq!(inst.mem_op(), MemType::None);
        assert_eq!(inst.write_back(), WBType::Alu);
        assert_eq!(inst.branch(0, 0), false);

        let inst = Instruction::from_binary(0x00000023).unwrap(); // sb x0, 0(x0)
        assert_eq!(inst.binary, 0x00000023);
        assert_eq!(inst.inst_type, InstType::S);
        assert_eq!(inst.is_cond(), false);
        assert_eq!(inst.rs1(), 0);
        assert_eq!(inst.rs2(), 0);
        assert_eq!(inst.rd(), 0);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), true);
        assert_eq!(inst.alu_use_reg2(), false);
        assert_eq!(inst.alu_op(), AluType::Add);
        assert_eq!(inst.mem_op(), MemType::Store);
        assert_eq!(inst.write_back(), WBType::None);

        let inst = Instruction::from_binary(0x00000063).unwrap(); // beq x0, x0, 0
        assert_eq!(inst.binary, 0x00000063);
        assert_eq!(inst.inst_type, InstType::B);
        assert_eq!(inst.is_cond(), true);
        assert_eq!(inst.rs1(), 0);
        assert_eq!(inst.rs2(), 0);
        assert_eq!(inst.rd(), 0);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), false);
        assert_eq!(inst.alu_use_reg2(), false);
        assert_eq!(inst.alu_op(), AluType::Add);
        assert_eq!(inst.mem_op(), MemType::None);
        assert_eq!(inst.write_back(), WBType::None);

        let inst = Instruction::from_binary(0x00000037).unwrap(); // lui x0, 0
        assert_eq!(inst.binary, 0x00000037);
        assert_eq!(inst.inst_type, InstType::U);
        assert_eq!(inst.is_cond(), false);
        assert_eq!(inst.rs1(), 0);
        assert_eq!(inst.rs2(), 0);
        assert_eq!(inst.rd(), 0);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), false);
        assert_eq!(inst.alu_use_reg2(), false);
        assert_eq!(inst.alu_op(), AluType::Bsel);
        assert_eq!(inst.mem_op(), MemType::None);
        assert_eq!(inst.write_back(), WBType::Alu);

        let inst = Instruction::from_binary(0x00000017).unwrap(); // auipc x0, 0
        assert_eq!(inst.binary, 0x00000017);
        assert_eq!(inst.inst_type, InstType::U);
        assert_eq!(inst.is_cond(), false);
        assert_eq!(inst.rs1(), 0);
        assert_eq!(inst.rs2(), 0);
        assert_eq!(inst.rd(), 0);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), false);
        assert_eq!(inst.alu_use_reg2(), false);
        assert_eq!(inst.alu_op(), AluType::Add);
        assert_eq!(inst.mem_op(), MemType::None);
        assert_eq!(inst.write_back(), WBType::Alu);

        let inst = Instruction::from_binary(0x0000006f).unwrap(); // jal x0, 0
        assert_eq!(inst.binary, 0x0000006f);
        assert_eq!(inst.inst_type, InstType::J);
        assert_eq!(inst.is_cond(), false);
        assert_eq!(inst.rs1(), 0);
        assert_eq!(inst.rs2(), 0);
        assert_eq!(inst.rd(), 0);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), false);
        assert_eq!(inst.alu_use_reg2(), false);
        assert_eq!(inst.alu_op(), AluType::Add);
        assert_eq!(inst.mem_op(), MemType::None);
        assert_eq!(inst.write_back(), WBType::Pc);

        let inst = Instruction::from_binary(0x00000067).unwrap(); // jalr x0, x0, 0
        assert_eq!(inst.binary, 0x00000067);
        assert_eq!(inst.inst_type, InstType::I);
        assert_eq!(inst.is_cond(), false);
        assert_eq!(inst.rs1(), 0);
        assert_eq!(inst.rs2(), 0);
        assert_eq!(inst.rd(), 0);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), true);
        assert_eq!(inst.alu_use_reg2(), false);
        assert_eq!(inst.alu_op(), AluType::Add);
        assert_eq!(inst.mem_op(), MemType::None);
        assert_eq!(inst.write_back(), WBType::Pc);

        assert!(Instruction::from_binary(0x00000000).is_err()); // invalid instruction
    }
}
