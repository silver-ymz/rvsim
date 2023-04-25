.globl main
.text
main:
    addi x1, x0, 10
    addi x2, x0, 0
loop:
    add  x2, x2, x1
    addi x1, x1, -1
    bne  x1, x0, loop
    addi a0, x0, 17
    ecall