# RISC-V 5-Stage Simulator

This is a 5-stage pipeline RISC-V simulator written in Rust.

> This is an out-of-order implementation based on tomasulo alogorithm.
> **It hasn't completed now.** The 5-stage pipeline implementation is on `master` branch.

## Support Instructions

1. All RV32I base instruction set.
2. Some RV32M instructions, such as `mul`, `mulh`, `mulhu`.

## Developing progress

- [x] RV32I base instruction set
- [x] Assembler
- [x] 5-stage pipeline
- [x] data hazard and control hazard
- [x] Visualization(debug)
- [x] more instructions
- [ ] F extension
- [x] Out of order (tomasulo)
- [ ] Branch Predictor

## Building

To build the simulator, you will need to have Rust installed. You can
get Rust from [rustup](https://rustup.rs/).

Once you have Rust installed, you can build the simulator by running
`cargo build` in the root of the repository.

## Usage

```
Usage: rvsim [OPTIONS] <PATH>

Arguments:
  <PATH>  Input assembly file

Options:
  -v, --verbose   Print pipeline info for each cycle
  -a, --analysis  Print analysis info
  -s, --step      Step running
  -h, --help      Print help
  -V, --version   Print version
```

## Explanation

### Pipeline
It uses data **forwarding** and stalling to solve data hazard and control hazard. And it will stall one cycle when branch instruction occurs and load instruction hazard.

### Special Instructions
1. `ebreak` will stop the program and need you to press enter to continue
2. `ecall` only supports `exit` now. And it will check whether `a0` is `17`, and take `a1` as exit code.

### Assembler
1. It doesn't support pseudo instruction now.
2. It doesn't support multi file linking now. And `.globl` will take its first label as entry point.
3. It supports `.data` and `.code`. `.data` can only put `.string`, `.word`, `.half` and `.byte` now. And `.code` can only put instructions now.
4. Its output endian is little endian.

## Examples
You can see some examples in `tests` directory.
1. `tests/1.s` is a simple example without any hazards.
2. `tests/2.s` is a simple example with RAW data hazard.
3. `tests/3.s` is a simple example with branch jump.
4. `tests/matrix.s` is a matrix multiplication example.