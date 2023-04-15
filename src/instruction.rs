use std::collections::HashMap;

use lazy_static::lazy_static;
use regex::Regex;

#[derive(Clone)]
pub struct Instruction {
    binary: u32,
    reg_write: bool,
    inst_type: InstType,
    branch_unsigned: bool,
    a_sel: bool,
    b_sel: bool,
    alu_type: AluType,
    mem_write: bool,
    write_back: WBType,
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
enum AluType {
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
    Mulhsu = 10,
    Mulhu = 11,
    Sub = 12,
    Sra = 13,
    Bsel = 15,
}

#[derive(Clone, PartialEq, Debug)]
enum WBType {
    Mem,
    Alu,
    Pc,
}

impl Instruction {
    pub fn nop() -> Self {
        Self::from_binary(0xfe00707f).unwrap() // add x0, x0, x0
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

        let reg_write = matches!(
            inst_type,
            InstType::R | InstType::I | InstType::U | InstType::J
        );

        let branch_unsigned = match inst_type {
            InstType::B => {
                (binary & 0x00007000) == 0x00007000 || (binary & 0x00007000) == 0x00006000
            }
            _ => false,
        };

        let a_sel = matches!(inst_type, InstType::R | InstType::I | InstType::S);

        let b_sel = matches!(inst_type, InstType::R);

        let alu_type = {
            if inst_type == InstType::U {
                (((binary >> 5) & 0x1) * 0xf).into()
            } else {
                ((((binary >> 12) & 0x7)
                    | (((binary >> 25) & 0x1) << 3)
                    | (((binary >> 30) & 0x1) * 0xc)) & (((binary >> 4) & 0x1) * 0xf))
                    .into()
            }
        };

        let mem_write = matches!(inst_type, InstType::S);

        let write_back = match inst_type {
            InstType::I => {
                if (binary & 0x7f) == 0x3 {
                    WBType::Mem
                } else if (binary & 0x7f) == 0x67 {
                    WBType::Pc
                } else {
                    WBType::Alu
                }
            }
            InstType::R | InstType::S | InstType::U | InstType::B => WBType::Alu,
            InstType::J => WBType::Pc,
        };

        Ok(Self {
            binary,
            reg_write,
            inst_type,
            branch_unsigned,
            a_sel,
            b_sel,
            alu_type,
            mem_write,
            write_back,
        })
    }
}

impl From<u32> for AluType {
    fn from(value: u32) -> Self {
        match value {
            0 => AluType::Add,
            1 => AluType::Sll,
            2 => AluType::Slt,
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
        assert_eq!(inst.reg_write, true);
        assert_eq!(inst.inst_type, InstType::R);
        assert_eq!(inst.branch_unsigned, false);
        assert_eq!(inst.a_sel, true);
        assert_eq!(inst.b_sel, true);
        assert_eq!(inst.alu_type, AluType::Add);
        assert_eq!(inst.mem_write, false);
        assert_eq!(inst.write_back, WBType::Alu);

        let inst = Instruction::from_binary(0x00000013).unwrap(); // addi x0, x0, 0
        assert_eq!(inst.binary, 0x00000013);
        assert_eq!(inst.reg_write, true);
        assert_eq!(inst.inst_type, InstType::I);
        assert_eq!(inst.branch_unsigned, false);
        assert_eq!(inst.a_sel, true);
        assert_eq!(inst.b_sel, false);
        assert_eq!(inst.alu_type, AluType::Add);
        assert_eq!(inst.mem_write, false);
        assert_eq!(inst.write_back, WBType::Alu);

        let inst = Instruction::from_binary(0x00000023).unwrap(); // sb x0, 0(x0)
        assert_eq!(inst.binary, 0x00000023);
        assert_eq!(inst.reg_write, false);
        assert_eq!(inst.inst_type, InstType::S);
        assert_eq!(inst.branch_unsigned, false);
        assert_eq!(inst.a_sel, true);
        assert_eq!(inst.b_sel, false);
        assert_eq!(inst.alu_type, AluType::Add);
        assert_eq!(inst.mem_write, true);
        assert_eq!(inst.write_back, WBType::Alu);

        let inst = Instruction::from_binary(0x00000063).unwrap(); // beq x0, x0, 0
        assert_eq!(inst.binary, 0x00000063);
        assert_eq!(inst.reg_write, false);
        assert_eq!(inst.inst_type, InstType::B);
        assert_eq!(inst.branch_unsigned, false);
        assert_eq!(inst.a_sel, false);
        assert_eq!(inst.b_sel, false);
        assert_eq!(inst.alu_type, AluType::Add);
        assert_eq!(inst.mem_write, false);
        assert_eq!(inst.write_back, WBType::Alu);

        let inst = Instruction::from_binary(0x00006063).unwrap(); // bltu x0, x0, 0
        assert_eq!(inst.binary, 0x00006063);
        assert_eq!(inst.reg_write, false);
        assert_eq!(inst.inst_type, InstType::B);
        assert_eq!(inst.branch_unsigned, true);
        assert_eq!(inst.a_sel, false);
        assert_eq!(inst.b_sel, false);
        assert_eq!(inst.alu_type, AluType::Add);
        assert_eq!(inst.mem_write, false);
        assert_eq!(inst.write_back, WBType::Alu);

        let inst = Instruction::from_binary(0x00000037).unwrap(); // lui x0, 0
        assert_eq!(inst.binary, 0x00000037);
        assert_eq!(inst.reg_write, true);
        assert_eq!(inst.inst_type, InstType::U);
        assert_eq!(inst.branch_unsigned, false);
        assert_eq!(inst.a_sel, false);
        assert_eq!(inst.b_sel, false);
        assert_eq!(inst.alu_type, AluType::Bsel);
        assert_eq!(inst.mem_write, false);
        assert_eq!(inst.write_back, WBType::Alu);

        let inst = Instruction::from_binary(0x00000017).unwrap(); // auipc x0, 0
        assert_eq!(inst.binary, 0x00000017);
        assert_eq!(inst.reg_write, true);
        assert_eq!(inst.inst_type, InstType::U);
        assert_eq!(inst.branch_unsigned, false);
        assert_eq!(inst.a_sel, false);
        assert_eq!(inst.b_sel, false);
        assert_eq!(inst.alu_type, AluType::Add);
        assert_eq!(inst.mem_write, false);
        assert_eq!(inst.write_back, WBType::Alu);

        let inst = Instruction::from_binary(0x0000006f).unwrap(); // jal x0, 0
        assert_eq!(inst.binary, 0x0000006f);
        assert_eq!(inst.reg_write, true);
        assert_eq!(inst.inst_type, InstType::J);
        assert_eq!(inst.branch_unsigned, false);
        assert_eq!(inst.a_sel, false);
        assert_eq!(inst.b_sel, false);
        assert_eq!(inst.alu_type, AluType::Add);
        assert_eq!(inst.mem_write, false);
        assert_eq!(inst.write_back, WBType::Pc);

        let inst = Instruction::from_binary(0x00000067).unwrap(); // jalr x0, x0, 0
        assert_eq!(inst.binary, 0x00000067);
        assert_eq!(inst.reg_write, true);
        assert_eq!(inst.inst_type, InstType::I);
        assert_eq!(inst.branch_unsigned, false);
        assert_eq!(inst.a_sel, true);
        assert_eq!(inst.b_sel, false);
        assert_eq!(inst.alu_type, AluType::Add);
        assert_eq!(inst.mem_write, false);
        assert_eq!(inst.write_back, WBType::Pc);

        assert!(Instruction::from_binary(0x00000000).is_err()); // invalid instruction
    }
}
