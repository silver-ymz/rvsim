.globl main
.data 
load_data_1: .float 1.2
load_data_2: .float 2.2
.text
main:
    flw f1, 0
    flw f2, 4
    fadd.s f0, f1, f2
    fsub.s f0, f1, f2
    fmul.s f0, f1, f2
    fdiv.s f0, f1, f2
    # addi a0, x0, 17
    ecall