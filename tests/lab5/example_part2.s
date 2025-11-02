  .data
a:
  .word 1

  .data
b:
  .word 0

  .text
  .global main
main:
mainEntry:
  li t3, 3
  la t1, a
  lw t1, 0(t1)
  add t6, t4, t1
  li t1, 1
  add s0, t6, t1
  la t2, b
  sw s0, 0(t2)
  li s1, 10
  la t1, b
  lw t1, 0(t1)
  add s4, s2, t1
  add s6, s4, s5
  add s8, s6, s7
  mv a0, s8
  li a7, 93
  ecall
