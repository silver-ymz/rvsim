use std::fmt::{self, Display};

#[derive(Clone)]
pub struct Instruction {
    binary: u32,
    inst_type: InstType,
    rs1: u32,
    rs2: u32,
    rd: u32,
    imm: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstType {
    R,
    I,
    S,
    B,
    U,
    J,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WBType {
    Mem,
    Alu,
    Pc,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MemType {
    Load,
    Store,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StationType {
    None,
    LoadStore,
    Integer,
    FAdd,
    FMul,
}

impl Instruction {
    pub fn nop() -> Self {
        Self::from_binary(0x00000033).unwrap() // add x0, x0, x0
    }

    pub fn from_binary(binary: u32) -> Result<Self, String> {
        let inst_type = match binary & 0x7f {
            0x33 | 0x53 => InstType::R,
            0x03 | 0x07 | 0x13 | 0x67 | 0x73 => InstType::I,
            0x23 | 0x27 => InstType::S,
            0x63 => InstType::B,
            0x37 | 0x17 => InstType::U,
            0x6f => InstType::J,
            _ => return Err(format!("Invalid instruction: {:08x}", binary)),
        };

        let rs1 = (binary >> 15) & 0x1f;
        let rs2 = (binary >> 20) & 0x1f;
        let rd = (binary >> 7) & 0x1f;

        let imm = match inst_type {
            InstType::I => sign_extend(binary >> 20, 12),
            InstType::S => sign_extend(((binary >> 7) & 0x1f) | ((binary >> 20) & 0xfe0), 12),
            InstType::B => sign_extend(
                (((binary >> 8) & 0xf) << 1)
                    | ((binary >> 20) & 0x7e0)
                    | ((binary << 4) & 0x800)
                    | ((binary >> 19) & 0x1000),
                13,
            ),
            InstType::U => binary & 0xfffff000,
            InstType::J => sign_extend(
                (((binary >> 21) & 0x3ff) << 1)
                    | ((binary >> 9) & 0x800)
                    | (binary & 0xff000)
                    | ((binary >> 11) & 0x100000),
                21,
            ),
            _ => 0,
        };

        Ok(Self {
            binary,
            inst_type,
            rs1,
            rs2,
            rd,
            imm,
        })
    }

    pub fn is_jump(&self) -> bool {
        match self.inst_type {
            InstType::B => true,
            InstType::J => true,
            InstType::I if (self.binary & 0x7f) == 0x67 => true,
            _ => false,
        }
    }

    pub fn rs1(&self) -> u32 {
        self.rs1
    }

    pub fn rs2(&self) -> u32 {
        self.rs2
    }

    pub fn rd(&self) -> u32 {
        self.rd
    }

    pub fn imm(&self) -> u32 {
        self.imm
    }

    pub fn alu_use_reg1(&self) -> bool {
        matches!(self.inst_type, InstType::R | InstType::I | InstType::S)
    }

    pub fn alu_use_reg2(&self) -> bool {
        matches!(self.inst_type, InstType::R)
    }

    pub(crate) fn alu_op(&self) -> AluType {
        match self.inst_type {
            InstType::R => {
                let mut code = (self.binary >> 12) & 0x7;
                code |= ((self.binary >> 30) & 0x1) * 0b1100;
                code |= ((self.binary >> 25) & 0x1) * 0b1000;
                code.into()
            }
            InstType::I if (self.binary & 0x7f) == 0x3 => AluType::Add,
            InstType::I => {
                let mut code = (self.binary >> 12) & 0x7;
                if code == 0b101 {
                    code |= ((self.binary >> 30) & 0x1) << 3;
                }
                code.into()
            }
            InstType::U => (((self.binary >> 5) & 0x1) * 0xf).into(),
            _ => 0.into(),
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
                let a = a;
                let b = b;
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
            InstType::J => true,
            InstType::I if (self.binary & 0x7f) == 0x67 => true,
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

    pub fn reg_write(&self) -> bool {
        matches!(
            self.inst_type,
            InstType::R | InstType::I | InstType::U | InstType::J
        ) && self.rd != 0
    }

    pub fn is_load(&self) -> bool {
        self.binary & 0x7f == 0x03
    }

    pub fn is_nop(&self) -> bool {
        self.binary == 0x33
    }

    pub fn is_ebreak(&self) -> bool {
        self.binary == 0x100073
    }

    pub fn is_ecall(&self) -> bool {
        self.binary == 0x73
    }

    pub fn is_float_point(&self) -> bool {
        self.binary & 0x7f == 0x07 || self.binary & 0x7f == 0x27 || self.binary & 0x7f == 0x53
    }

    // only distinguish loadstore, interger, float-add, float-mul now
    pub(crate) fn station(&self) -> StationType {
        match self.inst_type {
            InstType::I => {
                if (self.binary & 0x7f) == 0x03 || (self.binary & 0x7f) == 0x07 {
                    StationType::LoadStore
                } else {
                    StationType::Integer
                }
            }
            InstType::S => StationType::LoadStore,
            InstType::R => {
                if (self.binary & 0x7f) == 0x53 {
                    if (self.binary >> 12) & 0x7 == 0 || (self.binary >> 12) & 0x7 == 4 {
                        StationType::FAdd
                    } else if (self.binary >> 12) & 0x7 == 8 || (self.binary >> 12) & 0x7 == 0xc {
                        StationType::FMul
                    } else {
                        panic!("unknown float inst: {:x}", self.binary)
                    }
                } else {
                    StationType::Integer
                }
            }
            InstType::U => StationType::Integer,
            InstType::J => StationType::Integer,
            _ => StationType::None,
        }
    }

    // todo: print more user friendly info
    pub fn debug(&self) -> String {
        // disassemble
        let inst = match self.inst_type {
            InstType::R => {
                let opcode = self.binary & 0x7f;
                let func3 = (self.binary >> 12) & 0x7;
                let func7 = (self.binary >> 25) & 0x7f;
                match (opcode, func3, func7) {
                    (0x33, 0, 0) => format!("add x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 0, 0x20) => format!("sub x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 1, 0) => format!("sll x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 2, 0) => format!("slt x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 3, 0) => format!("sltu x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 4, 0) => format!("xor x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 5, 0) => format!("srl x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 5, 0x20) => format!("sra x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 6, 0) => format!("or x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 7, 0) => format!("and x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 0, 1) => format!("mul x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 1, 1) => format!("mulh x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 2, 1) => format!("mulhsu x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 3, 1) => format!("mulhu x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 4, 1) => format!("div x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 5, 1) => format!("divu x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 6, 1) => format!("rem x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x33, 7, 1) => format!("remu x{}, x{}, x{}", self.rd, self.rs1, self.rs2),
                    (0x53, _, 0) => format!("fadd.s f{}, f{}, f{}", self.rd, self.rs1, self.rs2),
                    (0x53, _, 0x04) => format!("fsub.s f{}, f{}, f{}", self.rd, self.rs1, self.rs2),
                    (0x53, _, 0x08) => format!("fmul.s f{}, f{}, f{}", self.rd, self.rs1, self.rs2),
                    (0x53, _, 0x0c) => format!("fdiv.s f{}, f{}, f{}", self.rd, self.rs1, self.rs2),
                    _ => format!("unknown"),
                }
            }
            InstType::I => {
                let opcode = self.binary & 0x7f;
                let func3 = (self.binary >> 12) & 0x7;
                match (opcode, func3) {
                    (0x13, 0) => format!("addi x{}, x{}, {}", self.rd, self.rs1, self.imm),
                    (0x13, 1) => format!("slli x{}, x{}, {}", self.rd, self.rs1, self.imm),
                    (0x13, 2) => format!("slti x{}, x{}, {}", self.rd, self.rs1, self.imm),
                    (0x13, 3) => format!("sltiu x{}, x{}, {}", self.rd, self.rs1, self.imm),
                    (0x13, 4) => format!("xori x{}, x{}, {}", self.rd, self.rs1, self.imm),
                    (0x13, 5) => format!("srli x{}, x{}, {}", self.rd, self.rs1, self.imm),
                    (0x13, 6) => format!("ori x{}, x{}, {}", self.rd, self.rs1, self.imm),
                    (0x13, 7) => format!("andi x{}, x{}, {}", self.rd, self.rs1, self.imm),
                    (0x03, 0) => format!("lb x{}, {}(x{})", self.rd, self.imm, self.rs1),
                    (0x03, 1) => format!("lh x{}, {}(x{})", self.rd, self.imm, self.rs1),
                    (0x03, 2) => format!("lw x{}, {}(x{})", self.rd, self.imm, self.rs1),
                    (0x03, 4) => format!("lbu x{}, {}(x{})", self.rd, self.imm, self.rs1),
                    (0x03, 5) => format!("lhu x{}, {}(x{})", self.rd, self.imm, self.rs1),
                    (0x67, 0) => format!("jalr x{}, {}(x{})", self.rd, self.imm, self.rs1),
                    (0x73, 0) => format!("ecall"),
                    (0x73, 1) => format!("ebreak"),
                    (0x07, 2) => format!("flw f{}, {}(x{})", self.rd, self.imm, self.rs1),
                    _ => format!("unknown"),
                }
            }
            InstType::S => {
                let opcode = self.binary & 0x7f;
                let func3 = (self.binary >> 12) & 0x7;
                match (opcode, func3) {
                    (0x23, 0) => format!("sb x{}, {}(x{})", self.rs2, self.imm, self.rs1),
                    (0x23, 1) => format!("sh x{}, {}(x{})", self.rs2, self.imm, self.rs1),
                    (0x23, 2) => format!("sw x{}, {}(x{})", self.rs2, self.imm, self.rs1),
                    (0x27, 2) => format!("fsw f{}, {}(x{})", self.rs2, self.imm, self.rs1),
                    _ => format!("unknown"),
                }
            }
            InstType::B => {
                let func3 = (self.binary >> 12) & 0x7;
                match func3 {
                    0 => format!("beq x{}, x{}, {}", self.rs1, self.rs2, self.imm),
                    1 => format!("bne x{}, x{}, {}", self.rs1, self.rs2, self.imm),
                    4 => format!("blt x{}, x{}, {}", self.rs1, self.rs2, self.imm),
                    5 => format!("bge x{}, x{}, {}", self.rs1, self.rs2, self.imm),
                    6 => format!("bltu x{}, x{}, {}", self.rs1, self.rs2, self.imm),
                    7 => format!("bgeu x{}, x{}, {}", self.rs1, self.rs2, self.imm),
                    _ => format!("unknown"),
                }
            }
            InstType::U => {
                let opcode = self.binary & 0x7;
                match opcode {
                    0x37 => format!("lui x{}, {}", self.rd, self.imm),
                    0x17 => format!("auipc x{}, {}", self.rd, self.imm),
                    _ => format!("unknown"),
                }
            }
            InstType::J => {
                format!("jal x{}, {}", self.rd, self.imm)
            }
        };

        inst
    }

    pub fn binary(&self) -> u32 {
        self.binary
    }
}

impl Default for Instruction {
    fn default() -> Self {
        Self::nop()
    }
}

impl Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.debug())
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

fn sign_extend(value: u32, bits: u32) -> u32 {
    let shift = 32 - bits;
    let sign = (value >> (bits - 1)) & 1;
    let mask = ((1 << shift) - 1) << bits;
    let sign_mask = sign * mask;
    (value << shift) >> shift | sign_mask
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_binary() {
        let inst = Instruction::from_binary(0x00000033).unwrap(); // add x0, x0, x0
        assert_eq!(inst.binary, 0x00000033);
        assert_eq!(inst.inst_type, InstType::R);
        assert_eq!(inst.is_jump(), false);
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
        assert_eq!(inst.reg_write(), false);

        let inst = Instruction::from_binary(0x00000013).unwrap(); // addi x0, x0, 0
        assert_eq!(inst.binary, 0x00000013);
        assert_eq!(inst.inst_type, InstType::I);
        assert_eq!(inst.is_jump(), false);
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
        assert_eq!(inst.reg_write(), false);

        let inst = Instruction::from_binary(0x00000023).unwrap(); // sb x0, 0(x0)
        assert_eq!(inst.binary, 0x00000023);
        assert_eq!(inst.inst_type, InstType::S);
        assert_eq!(inst.is_jump(), false);
        assert_eq!(inst.rs1(), 0);
        assert_eq!(inst.rs2(), 0);
        assert_eq!(inst.rd(), 0);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), true);
        assert_eq!(inst.alu_use_reg2(), false);
        assert_eq!(inst.alu_op(), AluType::Add);
        assert_eq!(inst.mem_op(), MemType::Store);
        assert_eq!(inst.write_back(), WBType::None);
        assert_eq!(inst.branch(0, 0), false);
        assert_eq!(inst.reg_write(), false);

        let inst = Instruction::from_binary(0x00000063).unwrap(); // beq x0, x0, 0
        assert_eq!(inst.binary, 0x00000063);
        assert_eq!(inst.inst_type, InstType::B);
        assert_eq!(inst.is_jump(), true);
        assert_eq!(inst.rs1(), 0);
        assert_eq!(inst.rs2(), 0);
        assert_eq!(inst.rd(), 0);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), false);
        assert_eq!(inst.alu_use_reg2(), false);
        assert_eq!(inst.alu_op(), AluType::Add);
        assert_eq!(inst.mem_op(), MemType::None);
        assert_eq!(inst.write_back(), WBType::None);
        assert_eq!(inst.branch(0, 0), true);
        assert_eq!(inst.reg_write(), false);

        let inst = Instruction::from_binary(0x000000b7).unwrap(); // lui x1, 0
        assert_eq!(inst.binary, 0x000000b7);
        assert_eq!(inst.inst_type, InstType::U);
        assert_eq!(inst.is_jump(), false);
        assert_eq!(inst.rs1(), 0);
        assert_eq!(inst.rs2(), 0);
        assert_eq!(inst.rd(), 1);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), false);
        assert_eq!(inst.alu_use_reg2(), false);
        assert_eq!(inst.alu_op(), AluType::Bsel);
        assert_eq!(inst.mem_op(), MemType::None);
        assert_eq!(inst.write_back(), WBType::Alu);
        assert_eq!(inst.branch(0, 0), false);
        assert_eq!(inst.reg_write(), true);

        let inst = Instruction::from_binary(0x00000017).unwrap(); // auipc x0, 0
        assert_eq!(inst.binary, 0x00000017);
        assert_eq!(inst.inst_type, InstType::U);
        assert_eq!(inst.is_jump(), false);
        assert_eq!(inst.rs1(), 0);
        assert_eq!(inst.rs2(), 0);
        assert_eq!(inst.rd(), 0);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), false);
        assert_eq!(inst.alu_use_reg2(), false);
        assert_eq!(inst.alu_op(), AluType::Add);
        assert_eq!(inst.mem_op(), MemType::None);
        assert_eq!(inst.write_back(), WBType::Alu);
        assert_eq!(inst.branch(0, 0), false);
        assert_eq!(inst.reg_write(), false);

        let inst = Instruction::from_binary(0x0000006f).unwrap(); // jal x0, 0
        assert_eq!(inst.binary, 0x0000006f);
        assert_eq!(inst.inst_type, InstType::J);
        assert_eq!(inst.is_jump(), true);
        assert_eq!(inst.rs1(), 0);
        assert_eq!(inst.rs2(), 0);
        assert_eq!(inst.rd(), 0);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), false);
        assert_eq!(inst.alu_use_reg2(), false);
        assert_eq!(inst.alu_op(), AluType::Add);
        assert_eq!(inst.mem_op(), MemType::None);
        assert_eq!(inst.write_back(), WBType::Pc);
        assert_eq!(inst.branch(0, 0), true);
        assert_eq!(inst.reg_write(), false);

        let inst = Instruction::from_binary(0x00000067).unwrap(); // jalr x0, x0, 0
        assert_eq!(inst.binary, 0x00000067);
        assert_eq!(inst.inst_type, InstType::I);
        assert_eq!(inst.is_jump(), true);
        assert_eq!(inst.rs1(), 0);
        assert_eq!(inst.rs2(), 0);
        assert_eq!(inst.rd(), 0);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), true);
        assert_eq!(inst.alu_use_reg2(), false);
        assert_eq!(inst.alu_op(), AluType::Add);
        assert_eq!(inst.mem_op(), MemType::None);
        assert_eq!(inst.write_back(), WBType::Pc);
        assert_eq!(inst.branch(0, 0), true);
        assert_eq!(inst.reg_write(), false);

        let inst = Instruction::from_binary(0x003100d3).unwrap(); // fadd.s f1, f2, f3
        assert_eq!(inst.binary, 0x003100d3);
        assert_eq!(inst.inst_type, InstType::R);
        assert_eq!(inst.is_jump(), false);
        assert_eq!(inst.rs1(), 2);
        assert_eq!(inst.rs2(), 3);
        assert_eq!(inst.rd(), 1);
        assert_eq!(inst.imm(), 0);
        assert_eq!(inst.alu_use_reg1(), true);
        assert_eq!(inst.alu_use_reg2(), true);
        assert_eq!(inst.is_float_point(), true);

        assert!(Instruction::from_binary(0x00000000).is_err()); // invalid instruction
    }

    #[test]
    fn test_branch() {
        let inst = Instruction::from_binary(0x00000063).unwrap(); // beq x0, x0, 0
        assert_eq!(inst.branch(0, 0), true);
        assert_eq!(inst.branch(0, 1), false);
        assert_eq!(inst.branch(1, 0), false);
        assert_eq!(inst.branch(1, 1), true);

        let inst = Instruction::from_binary(0x00001063).unwrap(); // bne x0, x0, 0
        assert_eq!(inst.branch(0, 0), false);
        assert_eq!(inst.branch(0, 1), true);
        assert_eq!(inst.branch(1, 0), true);
        assert_eq!(inst.branch(1, 1), false);

        let inst = Instruction::from_binary(0x00004063).unwrap(); // blt x0, x0, 0
        assert_eq!(inst.branch(0, 0), false);
        assert_eq!(inst.branch(0, 1), true);
        assert_eq!(inst.branch(1, 0), false);
        assert_eq!(inst.branch(1, 1), false);
        assert_eq!(inst.branch(u32::MAX, 0), true);
        assert_eq!(inst.branch(0, u32::MAX), false);

        let inst = Instruction::from_binary(0x00005063).unwrap(); // bge x0, x0, 0
        assert_eq!(inst.branch(0, 0), true);
        assert_eq!(inst.branch(0, 1), false);
        assert_eq!(inst.branch(1, 0), true);
        assert_eq!(inst.branch(1, 1), true);
        assert_eq!(inst.branch(u32::MAX, 0), false);
        assert_eq!(inst.branch(0, u32::MAX), true);

        let inst = Instruction::from_binary(0x00006063).unwrap(); // bltu x0, x0, 0
        assert_eq!(inst.branch(0, 0), false);
        assert_eq!(inst.branch(0, 1), true);
        assert_eq!(inst.branch(1, 0), false);
        assert_eq!(inst.branch(1, 1), false);
        assert_eq!(inst.branch(u32::MAX, 0), false);
        assert_eq!(inst.branch(0, u32::MAX), true);

        let inst = Instruction::from_binary(0x00007063).unwrap(); // bgeu x0, x0, 0
        assert_eq!(inst.branch(0, 0), true);
        assert_eq!(inst.branch(0, 1), false);
        assert_eq!(inst.branch(1, 0), true);
        assert_eq!(inst.branch(1, 1), true);
        assert_eq!(inst.branch(u32::MAX, 0), true);
        assert_eq!(inst.branch(0, u32::MAX), false);
    }
}
