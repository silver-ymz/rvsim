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
    lw x1, 0x10(x0)
    addi x1, x1, 1
    addi x2, x1, 1
    addi x3, x2, 1
    addi x4, x3, 1
    addi x5, x4, 1
    addi x6, x5, 1
    addi x7, x6, 1
    addi a0, x0, 17
    ecall
end:

lw x1, x0, x1d # if | id | ex | me | wb 
addi x2, x1, 1 #    | if | id | ex | me | wb
               # shoule nop
addi x3, x2, 1 #    |    | if | id | ex | me | wb
addi x4, x3, 1 #    |    |    | if | id | ex | me | wb