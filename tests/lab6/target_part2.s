  .data
a:
  .word 1

  .data
b:
  .word 0

  .text
  .globl main
main:
  addi sp, sp, -64      # prologue
mainEntry:
  li t0, 3
  sw t0, 60(sp)
  lw t0, 60(sp)
  sw t0, 56(sp)
  la t0, a
  lw t0, 0(t0)
  sw t0, 52(sp)
  lw t0, 56(sp)
  lw t1, 52(sp)
  add t0, t0, t1
  sw t0, 48(sp)
  lw t0, 48(sp)
  li t1, 1
  add t0, t0, t1
  sw t0, 44(sp)
  lw t0, 44(sp)
  la t1, b
  sw t0, 0(t1)
  li t0, 10
  sw t0, 40(sp)
  lw t0, 52(sp)
  sw t0, 36(sp)
  la t0, b
  lw t0, 0(t0)
  sw t0, 32(sp)
  lw t0, 36(sp)
  lw t1, 32(sp)
  add t0, t0, t1
  sw t0, 28(sp)
  lw t0, 60(sp)
  sw t0, 24(sp)
  lw t0, 28(sp)
  lw t1, 24(sp)
  add t0, t0, t1
  sw t0, 20(sp)
  lw t0, 40(sp)
  sw t0, 16(sp)
  lw t0, 20(sp)
  lw t1, 16(sp)
  add t0, t0, t1
  sw t0, 12(sp)
  lw a0, 12(sp)
  addi sp, sp, 64       # epilogue
  li a7, 93
  ecall