.globl main

.data
m0: .word 1 2 3 4 5 6 7 8 9
m1: .word 12 11 10 9 8 7 6 5 4 3 2 1
m2: .word 40 34 28 22 112 97 82 67 184 160 136 112

.text
# =======================================================
# FUNCTION: main
# if m2 == m0 * m1 then exit(0) else exit(1)
main:
	addi a0, x0, 0
	addi a1, x0, 3
	addi a2, x0, 3
	addi a3, x0, 36
	addi a4, x0, 3
	addi a5, x0, 4
	addi sp, sp, -48
	add a6, x0, sp
	jal ra, matmul
    addi a0, x0, 0
	addi a1, x0, 0
	addi a2, x0, 12
	add a3, x0, sp
	addi a4, x0, 84
main_loop:
	lw a5, 0(a3)
	lw a6, 0(a4)
	beq a5, a6, main_next_loop
	addi a0, a0, 1
	jal x0, main_end
main_next_loop:
	addi a3, a3, 4
	addi a4, a4, 4
	addi a1, a1, 1
	bne a1, a2, main_loop
main_end:
	jal ra, exit

# =======================================================
# FUNCTION: Matrix Multiplication of 2 integer matrices
#   d = matmul(m0, m1)
# Arguments:
#   a0 (int*)  is the pointer to the start of m0
#   a1 (int)   is the # of rows (height) of m0
#   a2 (int)   is the # of columns (width) of m0
#   a3 (int*)  is the pointer to the start of m1
#   a4 (int)   is the # of rows (height) of m1
#   a5 (int)   is the # of columns (width) of m1
#   a6 (int*)  is the pointer to the the start of d
# Returns:
#   None (void), sets d = matmul(m0, m1)
# Exceptions:
#   Make sure to check in top to bottom order!
#   - If the dimensions of m0 do not make sense,
#     this function terminates the program with exit code 38
#   - If the dimensions of m1 do not make sense,
#     this function terminates the program with exit code 38
#   - If the dimensions of m0 and m1 don't match,
#     this function terminates the program with exit code 38
# =======================================================
matmul:
	# Error checks
	addi t0, x0, 1
	blt a1, t0, matmul_error
	blt a2, t0, matmul_error
	blt a4, t0, matmul_error
	blt a5, t0, matmul_error
	bne a2, a4, matmul_error
	beq x0, x0, matmul_start
matmul_error:
	addi a0, x0, 38
	jal ra, exit
matmul_start:
	# Prologue
	addi sp, sp, -52
	sw s0, 0(sp)
	sw s1, 4(sp)
	sw s2, 8(sp)
	sw s3, 12(sp)
	sw s4, 16(sp)
	sw s5, 20(sp)
	sw s6, 24(sp)
	sw s7, 28(sp)
	sw s8, 32(sp)
	sw ra, 36(sp)
	sw s9, 40(sp)
	sw s10, 44(sp)
	sw s11, 48(sp)
	add s0, a0, x0
	add s1, a1, x0
	add s2, a2, x0
	add s3, a3, x0
	add s4, a4, x0
	add s5, a5, x0
	add s6, a6, x0
	slli s11, s2, 2
	addi s7, x0, 0
	add s9, s0, x0
matmul_outer_loop_start:
	addi s8, x0, 0
	add s10, s3, x0
matmul_inner_loop_start:
	add a0, s9, x0
	add a1, s10, x0
	add a2, s2, x0
	addi a3, x0, 1
	add a4, s5, x0
	jal ra, dot
	sw a0, 0(s6)
	addi s6, s6, 4
	addi s10, s10, 4
	addi s8, s8, 1
	bne s8, s5, matmul_inner_loop_start
matmul_inner_loop_end:
	add s9, s9, s11
	addi s7, s7, 1
	bne s7, s1, matmul_outer_loop_start
matmul_outer_loop_end:
	# Epilogue
	lw s0, 0(sp)
	lw s1, 4(sp)
	lw s2, 8(sp)
	lw s3, 12(sp)
	lw s4, 16(sp)
	lw s5, 20(sp)
	lw s6, 24(sp)
	lw s7, 28(sp)
	lw s8, 32(sp)
	lw ra, 36(sp)
	lw s9, 40(sp)
	lw s10, 44(sp)
	lw s11, 48(sp)
	addi sp, sp, 52
	jalr x0, ra, 0  # ret

# =======================================================
# FUNCTION: Dot product of 2 int arrays
# Arguments:
#   a0 (int*) is the pointer to the start of arr0
#   a1 (int*) is the pointer to the start of arr1
#   a2 (int)  is the number of elements to use
#   a3 (int)  is the stride of arr0
#   a4 (int)  is the stride of arr1
# Returns:
#   a0 (int)  is the dot product of arr0 and arr1
# Exceptions:
#   - If the length of the array is less than 1,
#     this function terminates the program with error code 36
#   - If the stride of either array is less than 1,
#     this function terminates the program with error code 37
# =======================================================
dot:
	addi t0, x0, 1
	bge a2, t0, dot_start0
	addi a0, x0, 36
	jal ra, exit
dot_start0:
	bge a3, t0, dot_start1
	addi a0, x0, 37
	jal ra, exit
dot_start1:
	bge a4, t0, dot_start2
	addi a0, x0, 37
	jal ra, exit
dot_start2:
	addi t0, x0, 0  # t0 is idx
	addi t1, a0, 0  # t1 is adr of arr0 elm
	addi t2, a1, 0  # t2 is adr of arr1 elm
	addi t3, x0, 0  # t3 is ans
	slli a5, a3, 2  # a5 is 4 * a3
	slli a6, a4, 2  # a6 is 4 * a4
dot_loop_start:
	lw t4, 0(t1)
	lw t5, 0(t2)
	mul t6, t4, t5
	add t3, t3, t6
	addi t0, t0, 1
	add t1, t1, a5
	add t2, t2, a6
	bne t0, a2, dot_loop_start
	add a0, x0, t3
	jalr x0, ra, 0  # ret

#================================================================
# void noreturn exit(int a0)
# Exits the program with error code a0.
# args:
#   a0 = Exit code.
# return:
#   This program does not return.
#================================================================
exit:
	add a1, a0, x0
	addi a0, x0, 17
	ecall