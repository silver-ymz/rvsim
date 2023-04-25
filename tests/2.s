.globl main
.data 
load_data: .word 1
.text
main:
    addi x1, x0, 1
    addi x1, x1, 1
    addi x1, x1, 1
    addi x1, x1, 1
    ebreak
    add x0, x0, x0
    add x0, x0, x0
    add x0, x0, x0
    lw  x1, 0
    addi x1, x1, 1
    sw  x1, 0
    addi a0, x0, 17
    ecall