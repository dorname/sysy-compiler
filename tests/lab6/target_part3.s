  .data
x:
  .word 1

  .data
y:
  .word 2

  .data
z:
  .word 3

  .data
a:
  .word 4

  .data
b:
  .word 5

  .data
c:
  .word 6

  .data
d:
  .word 7

  .data
e:
  .word 8

  .data
f:
  .word 9

  .data
g:
  .word 10

  .data
h:
  .word 11

  .data
i:
  .word 12

  .data
j:
  .word 13

  .data
k:
  .word 14

  .data
l:
  .word 15

  .data
m:
  .word 16

  .data
n:
  .word 17

  .data
o:
  .word 18

  .data
p:
  .word 19

  .data
q:
  .word 20

  .text
  .globl main
main:
  addi sp, sp, -16
mainEntry:
  li t0, 1
  mv t3, t0
  li t0, 2
  mv s0, t0
  li t0, 3
  mv s1, t0
  li t0, 4
  mv s2, t0
  li t0, 5
  mv s3, t0
  li t0, 6
  mv s4, t0
  li t0, 7
  mv s5, t0
  li t0, 8
  mv s6, t0
  li t0, 9
  mv s7, t0
  li t0, 10
  mv s8, t0
  li t0, 11
  mv s9, t0
  li t0, 12
  mv s10, t0
  li t0, 13
  mv s11, t0
  li t0, 14
  mv a0, t0
  li t0, 15
  mv a1, t0
  li t0, 16
  mv a2, t0
  li t0, 17
  mv a3, t0
  li t0, 18
  mv a4, t0
  li t0, 19
  mv a5, t0
  li t0, 20
  mv a6, t0
  la t2, x
  lw t0, 0(t2)
  mv a7, t0
  li t1, 1
  add t0, a7, t1
  sw t6, 12(sp)
  mv t6, t0
  la t2, x
  sw t6, 0(t2)
  la t2, y
  lw t0, 0(t2)
  mv t6, t0
  li t1, 2
  add t0, t6, t1
  mv a7, t0
  la t2, y
  sw a7, 0(t2)
  la t2, z
  lw t0, 0(t2)
  mv a7, t0
  li t1, 3
  add t0, a7, t1
  mv t6, t0
  la t2, z
  sw t6, 0(t2)
  la t2, a
  lw t0, 0(t2)
  mv t6, t0
  li t1, 4
  add t0, t6, t1
  mv a7, t0
  la t2, a
  sw a7, 0(t2)
  la t2, b
  lw t0, 0(t2)
  mv a7, t0
  li t1, 5
  add t0, a7, t1
  mv t6, t0
  la t2, b
  sw t6, 0(t2)
  la t2, c
  lw t0, 0(t2)
  mv t6, t0
  li t1, 6
  add t0, t6, t1
  mv a7, t0
  la t2, c
  sw a7, 0(t2)
  la t2, d
  lw t0, 0(t2)
  mv a7, t0
  li t1, 7
  add t0, a7, t1
  mv t6, t0
  la t2, d
  sw t6, 0(t2)
  la t2, e
  lw t0, 0(t2)
  mv t6, t0
  li t1, 8
  add t0, t6, t1
  mv a7, t0
  la t2, e
  sw a7, 0(t2)
  la t2, f
  lw t0, 0(t2)
  mv a7, t0
  li t1, 9
  add t0, a7, t1
  mv t6, t0
  la t2, f
  sw t6, 0(t2)
  la t2, g
  lw t0, 0(t2)
  mv t6, t0
  li t1, 10
  add t0, t6, t1
  mv a7, t0
  la t2, g
  sw a7, 0(t2)
  la t2, h
  lw t0, 0(t2)
  mv a7, t0
  li t1, 11
  add t0, a7, t1
  mv t6, t0
  la t2, h
  sw t6, 0(t2)
  la t2, i
  lw t0, 0(t2)
  mv t6, t0
  li t1, 12
  add t0, t6, t1
  mv a7, t0
  la t2, i
  sw a7, 0(t2)
  la t2, j
  lw t0, 0(t2)
  mv a7, t0
  li t1, 13
  add t0, a7, t1
  mv t6, t0
  la t2, j
  sw t6, 0(t2)
  la t2, k
  lw t0, 0(t2)
  mv t6, t0
  li t1, 14
  add t0, t6, t1
  mv a7, t0
  la t2, k
  sw a7, 0(t2)
  la t2, l
  lw t0, 0(t2)
  mv a7, t0
  li t1, 15
  add t0, a7, t1
  mv t6, t0
  la t2, l
  sw t6, 0(t2)
  la t2, m
  lw t0, 0(t2)
  mv t6, t0
  li t1, 16
  add t0, t6, t1
  mv a7, t0
  la t2, m
  sw a7, 0(t2)
  la t2, n
  lw t0, 0(t2)
  mv a7, t0
  li t1, 17
  add t0, a7, t1
  mv t6, t0
  la t2, n
  sw t6, 0(t2)
  la t2, o
  lw t0, 0(t2)
  mv t6, t0
  li t1, 18
  add t0, t6, t1
  mv a7, t0
  la t2, o
  sw a7, 0(t2)
  la t2, p
  lw t0, 0(t2)
  mv a7, t0
  li t1, 19
  add t0, a7, t1
  mv t6, t0
  la t2, p
  sw t6, 0(t2)
  la t2, q
  lw t0, 0(t2)
  mv t6, t0
  li t1, 20
  add t0, t6, t1
  mv a7, t0
  la t2, q
  sw a7, 0(t2)
  mv a7, t3
  li t1, 2
  mul t0, a7, t1
  mv t6, t0
  mv t3, t6
  mv t6, s0
  li t1, 2
  mul t0, t6, t1
  mv a7, t0
  mv s0, a7
  mv a7, s1
  li t1, 2
  mul t0, a7, t1
  mv t6, t0
  mv s1, t6
  mv t6, s2
  li t1, 2
  mul t0, t6, t1
  mv a7, t0
  mv s2, a7
  mv a7, s3
  li t1, 2
  mul t0, a7, t1
  mv t6, t0
  mv s3, t6
  mv t6, s4
  li t1, 2
  mul t0, t6, t1
  mv a7, t0
  mv s4, a7
  mv a7, s5
  li t1, 2
  mul t0, a7, t1
  mv t6, t0
  mv s5, t6
  mv t6, s6
  li t1, 2
  mul t0, t6, t1
  mv a7, t0
  mv s6, a7
  mv a7, s7
  li t1, 2
  mul t0, a7, t1
  mv t6, t0
  mv s7, t6
  mv t6, s8
  li t1, 2
  mul t0, t6, t1
  mv a7, t0
  mv s8, a7
  mv a7, s9
  li t1, 2
  mul t0, a7, t1
  mv t6, t0
  mv s9, t6
  mv t6, s10
  li t1, 2
  mul t0, t6, t1
  mv a7, t0
  mv s10, a7
  mv a7, s11
  li t1, 2
  mul t0, a7, t1
  mv t6, t0
  mv s11, t6
  mv t6, a0
  li t1, 2
  mul t0, t6, t1
  mv a7, t0
  mv a0, a7
  mv a7, a1
  li t1, 2
  mul t0, a7, t1
  mv t6, t0
  mv a1, t6
  mv t6, a2
  li t1, 2
  mul t0, t6, t1
  mv a7, t0
  mv a2, a7
  mv a7, a3
  li t1, 2
  mul t0, a7, t1
  mv t6, t0
  mv a3, t6
  mv t6, a4
  li t1, 2
  mul t0, t6, t1
  mv a7, t0
  mv a4, a7
  mv a7, a5
  li t1, 2
  mul t0, a7, t1
  mv t6, t0
  mv a5, t6
  mv t6, a6
  li t1, 2
  mul t0, t6, t1
  mv a7, t0
  mv a6, a7
  mv a7, t3
  mv t3, s0
  add t0, a7, t3
  mv s0, t0
  mv t3, s1
  add t0, s0, t3
  mv s1, t0
  mv t3, s2
  add t0, s1, t3
  mv s2, t0
  mv t3, s3
  add t0, s2, t3
  mv s3, t0
  mv t3, s4
  add t0, s3, t3
  mv s4, t0
  mv t3, s5
  add t0, s4, t3
  mv s5, t0
  mv t3, s6
  add t0, s5, t3
  mv s6, t0
  mv t3, s7
  add t0, s6, t3
  mv s7, t0
  mv t3, s8
  add t0, s7, t3
  mv s8, t0
  mv t3, s9
  add t0, s8, t3
  mv s9, t0
  mv t3, s10
  add t0, s9, t3
  mv s10, t0
  mv t3, s11
  add t0, s10, t3
  mv s11, t0
  mv t3, a0
  add t0, s11, t3
  mv a0, t0
  mv s11, a1
  add t0, a0, s11
  mv a1, t0
  mv s11, a2
  add t0, a1, s11
  mv a2, t0
  mv s11, a3
  add t0, a2, s11
  mv a3, t0
  mv s11, a4
  add t0, a3, s11
  mv a4, t0
  mv s11, a5
  add t0, a4, s11
  mv a5, t0
  mv a4, a6
  add t0, a5, a4
  mv a6, t0
  la t2, x
  lw t0, 0(t2)
  mv a5, t0
  add t0, a6, a5
  mv a4, t0
  la t2, y
  lw t0, 0(t2)
  mv a5, t0
  add t0, a4, a5
  mv a6, t0
  la t2, z
  lw t0, 0(t2)
  mv a5, t0
  add t0, a6, a5
  mv a4, t0
  la t2, a
  lw t0, 0(t2)
  mv a5, t0
  add t0, a4, a5
  mv a6, t0
  la t2, b
  lw t0, 0(t2)
  mv a5, t0
  add t0, a6, a5
  mv a4, t0
  la t2, c
  lw t0, 0(t2)
  mv a5, t0
  add t0, a4, a5
  mv a6, t0
  la t2, d
  lw t0, 0(t2)
  mv a5, t0
  add t0, a6, a5
  mv a4, t0
  la t2, e
  lw t0, 0(t2)
  mv a5, t0
  add t0, a4, a5
  mv a6, t0
  la t2, f
  lw t0, 0(t2)
  mv a5, t0
  add t0, a6, a5
  mv a4, t0
  la t2, g
  lw t0, 0(t2)
  mv a5, t0
  add t0, a4, a5
  mv a6, t0
  la t2, h
  lw t0, 0(t2)
  mv a5, t0
  add t0, a6, a5
  mv a4, t0
  la t2, i
  lw t0, 0(t2)
  mv a5, t0
  add t0, a4, a5
  mv a6, t0
  la t2, j
  lw t0, 0(t2)
  mv a5, t0
  add t0, a6, a5
  mv a4, t0
  la t2, k
  lw t0, 0(t2)
  mv a5, t0
  add t0, a4, a5
  mv a6, t0
  la t2, l
  lw t0, 0(t2)
  mv a5, t0
  add t0, a6, a5
  mv a4, t0
  la t2, m
  lw t0, 0(t2)
  mv a5, t0
  add t0, a4, a5
  mv a6, t0
  la t2, n
  lw t0, 0(t2)
  mv a5, t0
  add t0, a6, a5
  mv a4, t0
  la t2, o
  lw t0, 0(t2)
  mv a5, t0
  add t0, a4, a5
  mv a6, t0
  la t2, p
  lw t0, 0(t2)
  mv a5, t0
  add t0, a6, a5
  mv a4, t0
  la t2, q
  lw t0, 0(t2)
  mv a5, t0
  add t0, a4, a5
  sw t0, 12(sp)
  lw a0, 12(sp)
  addi sp, sp, 16
  li a7, 93
  ecall
