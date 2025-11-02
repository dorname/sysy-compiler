  .text
  .global main
main:
mainEntry:
  li t3, 5
  li t1, 3
  sgt t4, t1, t5
  mv t6, t5
  sub s0, t6, x0
  snez s0, t2
  bne s0, x0, if_false
  j if_true
if_true:
  li t1, 10
  slt s1, t1, s2
  mv s3, s2
  sub s4, s3, x0
  snez s4, t2
  bne s4, x0, if_false4
  j if_true2
if_next:
  mv a0, x0
  li a7, 93
  ecall
if_false:
  li a0, 3
  li a7, 93
  ecall
if_true2:
  li a0, 1
  li a7, 93
  ecall
if_next3:
  j if_next
if_false4:
  li a0, 2
  li a7, 93
  ecall
