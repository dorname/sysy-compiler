  .text                  # 指定接下来的代码位于文本段
  .globl main            # 声明 main 函数为全局可见
main:                    # main 函数的开始
  addi sp, sp, 0         # prologue
mainEntry:
  li a0, 1               # 将立即数 1 加载到寄存器 a0，用作 exit 的退出状态码
  addi sp, sp, 0         # epilogue
  li a7, 93              # 将立即数 93 加载到寄存器 a7，指定系统调用号为 exit
  ecall                  # 执行系统调用，根据 a7 的值调用 exit，结束程序