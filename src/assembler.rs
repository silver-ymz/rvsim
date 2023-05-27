use std::{
    collections::HashMap,
    error::Error,
    fs::File,
    io::{BufRead, BufReader, Write},
    path::Path,
};

use lazy_static::lazy_static;
use regex::Regex;

#[derive(Default)]
pub struct Program {
    mem: Vec<u32>,
    inst_name: HashMap<u32, String>,
    entry_addr: u32,
}

impl Program {
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let reader = BufReader::new(file);

        Self::from_buffer(reader)
    }

    pub(crate) fn from_buffer<T>(reader: T) -> Result<Self, String>
    where
        T: BufRead,
    {
        let buf = reader
            .lines()
            .map(|l| l.unwrap().trim().to_string())
            .collect::<Vec<_>>();

        let mut mem = Vec::with_capacity(1024);
        let mut inst_name = HashMap::new();

        let main_addr = Self::assembly(&buf, &mut mem, &mut inst_name)?;

        Ok(Self {
            mem,
            inst_name,
            entry_addr: main_addr,
        })
    }

    fn assembly(
        buf: &Vec<String>,
        mem: &mut Vec<u32>,
        inst_name: &mut HashMap<u32, String>,
    ) -> Result<u32, String> {
        let mut symbol: HashMap<String, u32> = HashMap::new();
        let mut empty_labels: HashMap<u32, String> = HashMap::new();
        let mut mem_addr: u32 = 0;
        let mut text_section = false;
        let mut data_section = false;
        let mut main_label = String::new();

        for line in buf {
            if line.starts_with("#") || line.is_empty() {
                continue;
            }

            if line.starts_with(".globl") {
                main_label = line.split_whitespace().nth(1).unwrap().to_owned();
            }

            if line.starts_with(".text") {
                text_section = true;
                data_section = false;
                continue;
            }

            if line.starts_with(".data") {
                data_section = true;
                text_section = false;
                continue;
            }

            if text_section {
                if let Some(caps) = LABEL_REGEX.captures(line) {
                    let label = caps.name("label").unwrap().as_str();
                    if symbol.insert(label.to_string(), mem_addr).is_some() {
                        return Err(format!("duplicate label: {}", label));
                    }
                }

                for (as_type, regex) in INSTRUCTION_REGEX.iter() {
                    if let Some(caps) = regex.captures(line) {
                        inst_name.insert(mem_addr, line.to_string());
                        let op = caps.name("op").unwrap().as_str();
                        let opcode = OPCODE_MAP
                            .get(op)
                            .ok_or(format!("invalid opcode: {} in {}", op, line))?;

                        let instruction = match as_type {
                            AssemblyType::RdRs1Rs2 => {
                                let rd = caps.name("rd").unwrap().as_str();
                                let rs1 = caps.name("rs1").unwrap().as_str();
                                let rs2 = caps.name("rs2").unwrap().as_str();

                                let rd = parse_reg_name(rd)
                                    .ok_or(format!("invalid register name: {} in {}", rd, line))?;
                                let rs1 = parse_reg_name(rs1)
                                    .ok_or(format!("invalid register name: {} in {}", rs1, line))?;
                                let rs2 = parse_reg_name(rs2)
                                    .ok_or(format!("invalid register name: {} in {}", rs2, line))?;

                                opcode | (rd << 7) | (rs1 << 15) | (rs2 << 20)
                            }
                            AssemblyType::RdRs1Imm => {
                                let rd = caps.name("rd").unwrap().as_str();
                                let rs1 = caps.name("rs1").unwrap().as_str();
                                let imm = caps.name("imm").unwrap().as_str();

                                let rd = parse_reg_name(rd)
                                    .ok_or(format!("invalid register name: {} in {}", rd, line))?;
                                let rs1 = parse_reg_name(rs1)
                                    .ok_or(format!("invalid register name: {} in {}", rs1, line))?;

                                let imm = parse_imm(imm)?;

                                opcode | (rd << 7) | (rs1 << 15) | (imm << 20)
                            }
                            AssemblyType::RgImmRs1 => {
                                let rg = caps.name("rg").unwrap().as_str();
                                let imm = caps.name("imm").unwrap().as_str();
                                let rs1 = caps.name("rs1").unwrap().as_str();

                                let rg = parse_reg_name(rg)
                                    .ok_or(format!("invalid register name: {} in {}", rg, line))?;
                                let rs1 = parse_reg_name(rs1)
                                    .ok_or(format!("invalid register name: {} in {}", rs1, line))?;

                                let imm = parse_imm(imm)?;

                                if ["sb", "sh", "sw"].contains(&op) {
                                    opcode
                                        | (rg << 20)
                                        | (rs1 << 15)
                                        | ((imm & 0x1f) << 7)
                                        | ((imm & 0xfe0) << 20)
                                } else {
                                    opcode | (rg << 7) | (rs1 << 15) | (imm << 20)
                                }
                            }
                            AssemblyType::Rs1Rs2Label => {
                                let rs1 = caps.name("rs1").unwrap().as_str();
                                let rs2 = caps.name("rs2").unwrap().as_str();
                                let label = caps.name("label").unwrap().as_str();

                                let rs1 = parse_reg_name(rs1)
                                    .ok_or(format!("invalid register name: {} in {}", rs1, line))?;
                                let rs2 = parse_reg_name(rs2)
                                    .ok_or(format!("invalid register name: {} in {}", rs2, line))?;

                                empty_labels.insert(mem_addr, label.to_owned());

                                opcode | (rs1 << 15) | (rs2 << 20)
                            }
                            AssemblyType::RdLabel => {
                                let rd = caps.name("rd").unwrap().as_str();
                                let label = caps.name("label").unwrap().as_str();

                                let rd = parse_reg_name(rd)
                                    .ok_or(format!("invalid register name: {} in {}", rd, line))?;

                                empty_labels.insert(mem_addr, label.to_owned());

                                opcode | (rd << 7)
                            }

                            AssemblyType::RdImm => {
                                let rd = caps.name("rd").unwrap().as_str();
                                let imm = caps.name("imm").unwrap().as_str();

                                let rd = parse_reg_name(rd)
                                    .ok_or(format!("invalid register name: {} in {}", rd, line))?;

                                let imm = parse_imm(imm)?;

                                opcode | (rd << 7) | (imm << 20)
                            }
                            AssemblyType::OnlyOp => opcode.clone(),
                        };

                        mem.push(instruction);

                        mem_addr += 4;

                        break;
                    }
                }
            }

            if data_section {
                if let Some(caps) = LABEL_REGEX.captures(line) {
                    let label = caps.name("label").unwrap().as_str();
                    if symbol.insert(label.to_string(), mem_addr).is_some() {
                        return Err(format!("duplicate label: {}", label));
                    }
                }

                for regex in DATA_REGEX.iter() {
                    if let Some(caps) = regex.captures(line) {
                        let data_type = caps.name("type").unwrap().as_str();
                        let data = caps.name("data").unwrap().as_str();

                        match data_type {
                            "string" => {
                                let mut bytes = data.as_bytes().to_vec();
                                bytes.push(0);
                                let mut size = bytes.len();
                                if size % 4 != 0 {
                                    size += 4 - size % 4;
                                }
                                bytes.resize(size, 0);
                                let mut word = 0;
                                for i in 0..size {
                                    word = (word << 8) | (bytes[i] as u32);
                                    if i % 4 == 3 {
                                        mem.push(word);
                                        word = 0;
                                    }
                                }
                                mem_addr += size as u32;
                            }
                            "word" => {
                                for word in data.split_ascii_whitespace() {
                                    mem.push(parse_imm(word)?);
                                }
                                mem_addr += 4 * data.split_whitespace().count() as u32;
                            }
                            "byte" => {
                                let mut bytes = data
                                    .split_ascii_whitespace()
                                    .map(|b| b.parse::<u8>().unwrap())
                                    .collect::<Vec<_>>();
                                let mut size = bytes.len();
                                if size % 4 != 0 {
                                    size += 4 - size % 4;
                                }
                                bytes.resize(size, 0);
                                let mut word = 0;
                                for i in 0..size {
                                    word = (word << 8) | (bytes[i] as u32);
                                    if i % 4 == 3 {
                                        mem.push(word);
                                        word = 0;
                                    }
                                }
                                mem_addr += size as u32;
                            }
                            "half" => {
                                let mut bytes = data
                                    .split_ascii_whitespace()
                                    .map(|b| b.parse::<u16>().unwrap())
                                    .collect::<Vec<_>>();
                                let mut size = bytes.len();
                                if size % 2 != 0 {
                                    size += 2 - size % 2;
                                }
                                bytes.resize(size, 0);
                                let mut word = 0;
                                for i in 0..size {
                                    word = (word << 16) | (bytes[i] as u32);
                                    if i % 2 == 1 {
                                        mem.push(word);
                                        word = 0;
                                    }
                                }
                                mem_addr += (size * 2) as u32;
                            }
                            "float" => {
                                for word in data.split_ascii_whitespace() {
                                    mem.push(word.parse::<f32>().unwrap().to_bits());
                                }
                                mem_addr += 4 * data.split_whitespace().count() as u32;
                            }
                            _ => {
                                return Err(format!("unknown data type: {}", data_type));
                            }
                        }
                    }
                }
            }
        }

        for (addr, label) in empty_labels {
            let offset = (*symbol.get(&label).ok_or(format!(
                "undefined label {} in {}",
                label,
                inst_name.get(&addr).unwrap()
            ))? as i32
                - addr as i32) as u32;
            let mut inst = mem[addr as usize / 4];
            if inst & 0x7f == 0x6f {
                inst |= ((offset & 0x100000) << 11)
                    | ((offset & 0x7fe) << 20)
                    | ((offset & 0x800) << 9)
                    | (offset & 0xff000);
            } else {
                inst |= ((offset & 0x1000) << 19)
                    | ((offset & 0x7e0) << 20)
                    | ((offset & 0x800) >> 4)
                    | ((offset & 0x1e) << 7);
            }
            mem[addr as usize / 4] = inst;
        }

        symbol
            .get(&main_label)
            .ok_or("program entry not found".to_string())
            .copied()
    }

    // fixme: solve endian problem
    pub fn write_file(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        let mut file = File::create(path)?;

        for word in self.mem.iter() {
            file.write_all(&word.to_be_bytes())?;
        }
        Ok(())
    }

    pub fn print_stdout(&self) {
        for (addr, data) in self.mem.iter().enumerate() {
            println!("{:08x}: {:08x}", addr * 4, data);
        }
    }

    pub fn mem(&self) -> &Vec<u32> {
        &self.mem
    }

    pub fn inst_name(&self) -> &HashMap<u32, String> {
        &self.inst_name
    }

    pub fn entry(&self) -> u32 {
        self.entry_addr
    }
}

fn parse_reg_name(name: &str) -> Option<u32> {
    match name {
        "zero" => Some(0),
        "ra" => Some(1),
        "sp" => Some(2),
        "gp" => Some(3),
        "tp" => Some(4),
        "t0" => Some(5),
        "t1" => Some(6),
        "t2" => Some(7),
        "s0" => Some(8),
        "s1" => Some(9),
        "a0" => Some(10),
        "a1" => Some(11),
        "a2" => Some(12),
        "a3" => Some(13),
        "a4" => Some(14),
        "a5" => Some(15),
        "a6" => Some(16),
        "a7" => Some(17),
        "s2" => Some(18),
        "s3" => Some(19),
        "s4" => Some(20),
        "s5" => Some(21),
        "s6" => Some(22),
        "s7" => Some(23),
        "s8" => Some(24),
        "s9" => Some(25),
        "s10" => Some(26),
        "s11" => Some(27),
        "t3" => Some(28),
        "t4" => Some(29),
        "t5" => Some(30),
        "t6" => Some(31),
        "x0" => Some(0),
        "x1" => Some(1),
        "x2" => Some(2),
        "x3" => Some(3),
        "x4" => Some(4),
        "x5" => Some(5),
        "x6" => Some(6),
        "x7" => Some(7),
        "x8" => Some(8),
        "x9" => Some(9),
        "x10" => Some(10),
        "x11" => Some(11),
        "x12" => Some(12),
        "x13" => Some(13),
        "x14" => Some(14),
        "x15" => Some(15),
        "x16" => Some(16),
        "x17" => Some(17),
        "x18" => Some(18),
        "x19" => Some(19),
        "x20" => Some(20),
        "x21" => Some(21),
        "x22" => Some(22),
        "x23" => Some(23),
        "x24" => Some(24),
        "x25" => Some(25),
        "x26" => Some(26),
        "x27" => Some(27),
        "x28" => Some(28),
        "x29" => Some(29),
        "x30" => Some(30),
        "x31" => Some(31),
        "f0" => Some(0),
        "f1" => Some(1),
        "f2" => Some(2),
        "f3" => Some(3),
        "f4" => Some(4),
        "f5" => Some(5),
        "f6" => Some(6),
        "f7" => Some(7),
        "f8" => Some(8),
        "f9" => Some(9),
        "f10" => Some(10),
        "f11" => Some(11),
        "f12" => Some(12),
        "f13" => Some(13),
        "f14" => Some(14),
        "f15" => Some(15),
        "f16" => Some(16),
        "f17" => Some(17),
        "f18" => Some(18),
        "f19" => Some(19),
        "f20" => Some(20),
        "f21" => Some(21),
        "f22" => Some(22),
        "f23" => Some(23),
        "f24" => Some(24),
        "f25" => Some(25),
        "f26" => Some(26),
        "f27" => Some(27),
        "f28" => Some(28),
        "f29" => Some(29),
        "f30" => Some(30),
        "f31" => Some(31),
        _ => None,
    }
}

fn parse_imm(imm: &str) -> Result<u32, String> {
    if imm.starts_with("-") {
        let imm = &imm[1..];
        let num = if imm.starts_with("0x") {
            u32::from_str_radix(&imm[2..], 16)
                .map(|x| !x + 1)
                .map_err(|e| e.to_string())
        } else if imm.starts_with("0b") {
            u32::from_str_radix(&imm[2..], 2)
                .map(|x| !x + 1)
                .map_err(|e| e.to_string())
        } else {
            imm.parse::<u32>()
                .map(|x| !x + 1)
                .map_err(|e| e.to_string())
        };
        return num;
    }

    if imm.starts_with("0x") {
        u32::from_str_radix(&imm[2..], 16).map_err(|e| e.to_string())
    } else if imm.starts_with("0b") {
        u32::from_str_radix(&imm[2..], 2).map_err(|e| e.to_string())
    } else {
        imm.parse::<u32>().map_err(|e| e.to_string())
    }
}
enum AssemblyType {
    RdRs1Rs2,    // add rd, rs1, rs2
    RdRs1Imm,    // addi rd, rs1, imm
    RgImmRs1,    // lb rd, imm(rs1) and sb rs2, imm(rs1)
    Rs1Rs2Label, // beq rs1, rs2, label
    RdLabel,     // jal rd, label
    RdImm,       // auipc rd, imm
    OnlyOp,      // ecall and ebreak
}

lazy_static! {
    static ref LABEL_REGEX: Regex = Regex::new(r"(?P<label>\w+):").unwrap();

    static ref DATA_REGEX: Vec<Regex> = vec![
        Regex::new(r#"\.(?P<type>string)\s+"(?P<data>.*)""#).unwrap(),      // .string
        Regex::new(r"\.(?P<type>word)\s+(?P<data>[0-9]+)").unwrap(),      // .word
        Regex::new(r"\.(?P<type>byte)\s+(?P<data>[0-9]+)").unwrap(),      // .byte
        Regex::new(r"\.(?P<type>half)\s+(?P<data>[0-9]+)").unwrap(),      // .half
        Regex::new(r"\.(?P<type>float)\s+(?P<data>[0-9]+(\.[0-9]+)?)").unwrap(),     // .float
    ];

    static ref INSTRUCTION_REGEX: Vec<(AssemblyType, Regex)> = {
        use AssemblyType::*;
        vec![
            (RdRs1Rs2, Regex::new(r"(?P<op>[a-z\.]+)\s+(?P<rd>[a-z][0-9]+|zero|sp|ra|gp|tp),?\s+(?P<rs1>([a-z][0-9]+)|zero|sp|ra|gp|tp),?\s+(?P<rs2>([a-z][0-9]+)|zero|sp|ra|gp|tp)").unwrap()),
            (RdRs1Imm, Regex::new(r"(?P<op>[a-z\.]+)\s+(?P<rd>[a-z][0-9]+|zero|sp|ra|gp|tp),?\s+(?P<rs1>([a-z][0-9]+)|zero|sp|ra|gp|tp),?\s+(?P<imm>-?(0x)?[0-9]+)").unwrap()),
            (RgImmRs1, Regex::new(r"(?P<op>[a-z\.]+)\s+(?P<rg>[a-z][0-9]+|zero|sp|ra|gp|tp),?\s+(?P<imm>-?(0x)?[0-9]+)\((?P<rs1>[a-z][0-9]+|zero|sp|ra|gp|tp)\)").unwrap()),
            (Rs1Rs2Label, Regex::new(r"(?P<op>[a-z\.]+)\s+(?P<rs1>[a-z][0-9]+|zero|sp|ra|gp|tp),?\s+(?P<rs2>[a-z][0-9]+|zero|sp|ra|gp|tp),?\s+(?P<label>[a-z][a-z_0-9]+)").unwrap()),
            (RdLabel, Regex::new(r"(?P<op>[a-z\.]+)\s+(?P<rd>[a-z][0-9]+|zero|sp|ra|gp|tp),?\s+(?P<label>[a-z][a-z_0-9]+)").unwrap()),
            (RdImm, Regex::new(r"(?P<op>[a-z\.]+)\s+(?P<rd>[a-z][0-9]+|zero|sp|ra|gp|tp),?\s+(?P<imm>-?(0x)?[0-9]+)").unwrap()),
            (OnlyOp, Regex::new(r"(?P<op>(ecall|ebreak))").unwrap())
        ]
    };

    static ref OPCODE_MAP: HashMap<String, u32> = HashMap::from([
        ("add".to_string(), 0x00000033),
        ("mul".to_string(), 0x02000033),
        ("sub".to_string(), 0x40000033),
        ("sll".to_string(), 0x00001033),
        ("mulh".to_string(), 0x02001033),
        ("mulhsu".to_string(), 0x02002033),
        ("mulhu".to_string(), 0x02003033),
        ("slt".to_string(), 0x00002033),
        ("sltu".to_string(), 0x00003033),
        ("xor".to_string(), 0x00004033),
        ("srl".to_string(), 0x00005033),
        ("sra".to_string(), 0x40005033),
        ("or".to_string(), 0x00006033),
        ("and".to_string(), 0x00007033),
        ("addi".to_string(), 0x00000013),
        ("slli".to_string(), 0x00001013),
        ("slti".to_string(), 0x00002013),
        ("sltiu".to_string(), 0x00003013),
        ("xori".to_string(), 0x00004013),
        ("srli".to_string(), 0x00005013),
        ("srai".to_string(), 0x40005013),
        ("ori".to_string(), 0x00006013),
        ("andi".to_string(), 0x00007013),
        ("lb".to_string(), 0x00000003),
        ("lh".to_string(), 0x00001003),
        ("lw".to_string(), 0x00002003),
        ("lbu".to_string(), 0x00004003),
        ("lhu".to_string(), 0x00005003),
        ("sb".to_string(), 0x00000023),
        ("sh".to_string(), 0x00001023),
        ("sw".to_string(), 0x00002023),
        ("beq".to_string(), 0x00000063),
        ("bne".to_string(), 0x00001063),
        ("blt".to_string(), 0x00004063),
        ("bge".to_string(), 0x00005063),
        ("bltu".to_string(), 0x00006063),
        ("bgeu".to_string(), 0x00007063),
        ("jal".to_string(), 0x0000006f),
        ("jalr".to_string(), 0x00000067),
        ("lui".to_string(), 0x00000037),
        ("auipc".to_string(), 0x00000017),
        ("ecall".to_string(), 0x00000073),
        ("ebreak".to_string(), 0x00100073),

        ("flw".to_string(), 0x00002007),
        ("fsw".to_string(), 0x000020a7),
        ("fadd.s".to_string(), 0x00000053),
        ("fsub.s".to_string(), 0x08000053),
        ("fmul.s".to_string(), 0x10000053),
        ("fdiv.s".to_string(), 0x18000053),
    ]);

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data() {
        let test_str = r#"
        .globl end
        .data
        test_str: .string "Hello, world!"
        test_word:
            .word 1 2 3 4
        test_byte:
            .byte 1 2 3 4 5
        test_half:
            .half 1 2 3 4 5
        end:
        "#;

        let program = Program::from_buffer(test_str.as_bytes()).unwrap();

        assert_eq!(
            program.mem,
            vec![
                0x48656c6c, 0x6f2c2077, 0x6f726c64, 0x21000000, 0x00000001, 0x00000002, 0x00000003,
                0x00000004, 0x01020304, 0x05000000, 0x00010002, 0x00030004, 0x00050000
            ]
        );
    }

    #[test]
    fn test_text_without_label() {
        let test_str = r#"
        .globl main
        .text
        main:
        add x0, x0, x0
        addi x0, x0, 1
        lb x0, 1(x0)
        sb x0, 0x21(x0)
        auipc x0, 0
        "#;

        let program = Program::from_buffer(test_str.as_bytes()).unwrap();

        assert_eq!(
            program.mem,
            vec![0x00000033, 0x00100013, 0x00100003, 0x020000a3, 0x00000017]
        );
        assert_eq!(
            program.inst_name,
            HashMap::from([
                (0 * 4, "add x0, x0, x0".to_string()),
                (1 * 4, "addi x0, x0, 1".to_string()),
                (2 * 4, "lb x0, 1(x0)".to_string()),
                (3 * 4, "sb x0, 0x21(x0)".to_string()),
                (4 * 4, "auipc x0, 0".to_string()),
            ])
        );
    }

    #[test]
    fn test_text_with_label() {
        let test_str = r#"
        .globl main
        .text
        add x0, x0, x0
        add x0, x0, x0
        main:
        add x0, x0, x0
        beq x0, x0, main
        jal x0, end
        end:
        "#;

        let program = Program::from_buffer(test_str.as_bytes()).unwrap();

        assert_eq!(
            program.mem,
            vec![0x00000033, 0x00000033, 0x00000033, 0xfe000ee3, 0x0040006f]
        );
        assert_eq!(
            program.inst_name,
            HashMap::from([
                (0 * 4, "add x0, x0, x0".to_string()),
                (1 * 4, "add x0, x0, x0".to_string()),
                (2 * 4, "add x0, x0, x0".to_string()),
                (3 * 4, "beq x0, x0, main".to_string()),
                (4 * 4, "jal x0, end".to_string()),
            ])
        );
    }

    #[test]
    fn test_all() {
        let test_str = r#"
        .globl main
        .data
        test_str: .string "Hello, world!"
        test_word:
            .word 1 2 3 4
        test_byte:
            .byte 1 2 3 4 5
        test_half:
            .half 1 2 3 4 5
        .text
        add x0, x0, x0
        add x0, x0, x0
        main:
        add x0, x0, x0
        beq x0, x0, main
        jal x0, end
        end:
        "#;

        let program = Program::from_buffer(test_str.as_bytes()).unwrap();

        assert_eq!(
            program.mem,
            vec![
                0x48656c6c, 0x6f2c2077, 0x6f726c64, 0x21000000, 0x00000001, 0x00000002, 0x00000003,
                0x00000004, 0x01020304, 0x05000000, 0x00010002, 0x00030004, 0x00050000, 0x00000033,
                0x00000033, 0x00000033, 0xfe000ee3, 0x0040006f
            ]
        );
        assert_eq!(
            program.inst_name,
            HashMap::from([
                (13 * 4, "add x0, x0, x0".to_string()),
                (14 * 4, "add x0, x0, x0".to_string()),
                (15 * 4, "add x0, x0, x0".to_string()),
                (16 * 4, "beq x0, x0, main".to_string()),
                (17 * 4, "jal x0, end".to_string()),
            ])
        );
    }
}
