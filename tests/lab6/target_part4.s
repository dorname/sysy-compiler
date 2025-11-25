  .data
x:
  .word 56

  .data
y:
  .word 98

    .text
    .globl main
main:
    addi sp, sp, 0
mainEntry:
    la t2, x
    lw t0, 0(t2)

    la t2, y
    lw t1, 0(t2)

    mv t2, t0
    mv t3, t1

loop:
    beq t3, x0, done

    bgt t2, t3, a_gt_b
    sub t3, t3, t2
    j loop

a_gt_b:
    sub t2, t2, t3
    j loop

done:
    mv a0, t2
    addi sp, sp, 0
    li a7, 93
    ecall