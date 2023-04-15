# RISC-V 5-Stage Simulator

This is a 5-stage RISC-V simulator written in Rust. It is a work in
progress, and hasn't yet completed.

## Support Instructions

1. All RV32I base instruction set, except for `fence`, `ecall`, `ebreak`.
2. Some RV32M instructions, such as `mul`, `mulh`, `mulhu`.

## Developing progress

- [x] RV32I base instruction set
- [x] Assembler
- [ ] 5-stage pipeline
- [ ] Visualization
- [ ] more instructions

## Building

To build the simulator, you will need to have Rust installed. You can
get Rust from [rustup](https://rustup.rs/).

Once you have Rust installed, you can build the simulator by running
`cargo build` in the root of the repository.

## Usage

