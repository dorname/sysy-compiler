use std::fmt::Write;
pub struct AsmBuilder{
    buf: String,
}
impl AsmBuilder {
    pub fn new() -> Self {
        Self {
            buf: String::new(),
        }
    }
  
   /// 段和符号声明指令
   pub fn emit_data_section(&mut self) {
    let _ = writeln!(self.buf, "  .data");  // 数据段
   }
   pub fn emit_text_section(&mut self) {
    let _ = writeln!(self.buf, "  .text");  // 代码段
   }
   pub fn emit_global_symbol(&mut self, symbol: &str) {
    let _ = writeln!(self.buf, "  .globl  {}", symbol);  // 全局符号表
   }
   pub fn emit_label(&mut self, label: &str) {
    let _ = writeln!(self.buf, "{}:", label);  // 标签
   }
   pub fn emit_word(&mut self, value: i32) {
    let _ = writeln!(self.buf, "  .word {}", value);
   }

   /// 函数框架指令
   pub fn emit_function_prologue(&mut self, stack_size: usize) {
    let _ = writeln!(self.buf, "  addi sp, sp, -{}", stack_size);  // 函数序言
   }

   pub fn emit_function_epilogue(&mut self, stack_size: usize) {
    let _ = writeln!(self.buf, "  addi sp, sp, {}", stack_size);  // 函数尾声
   }

   pub fn emit_exit_syscall(&mut self) {
    let _ = writeln!(self.buf, "  li a7, 93");  // 设置系统调用号
    let _ = writeln!(self.buf, "  ecall");  // 执行系统调用
   }

   /// 立即数加载指令
   pub fn emit_li(&mut self, reg: &str, value: i32) {
    let _ = writeln!(self.buf, "  li {}, {}", reg, value);  // 立即数加载
   }
   pub fn emit_la(&mut self, reg: &str, label: &str) {
    let _ = writeln!(self.buf, "  la {}, {}", reg, label);  // 地址加载
   }

   /// 算术运算指令
   pub fn emit_add(&mut self, dest: &str, src1: &str, src2: &str) {
    let _ = writeln!(self.buf, "  add {}, {}, {}", dest, src1, src2);  // 加法运算
   }
   pub fn emit_sub(&mut self, dest: &str, src1: &str, src2: &str) {
    let _ = writeln!(self.buf, "  sub {}, {}, {}", dest, src1, src2);  // 减法运算
   }
   pub fn emit_mul(&mut self, dest: &str, src1: &str, src2: &str) {
    let _ = writeln!(self.buf, "  mul {}, {}, {}", dest, src1, src2);  // 乘法运算
   }
   pub fn emit_div(&mut self, dest: &str, src1: &str, src2: &str) {
    let _ = writeln!(self.buf, "  div {}, {}, {}", dest, src1, src2);  // 除法运算
   }
   pub fn emit_rem(&mut self, dest: &str, src1: &str, src2: &str) {
    let _ = writeln!(self.buf, "  rem {}, {}, {}", dest, src1, src2);  // 取余运算
   }
   pub fn emit_addi(&mut self, dest: &str, src: &str, imm: i32) {
    let _ = writeln!(self.buf, "  addi {}, {}, {}", dest, src, imm);  // 立即数加法
   }
   pub fn emit_subi(&mut self, dest: &str, src: &str, imm: i32) {
    let _ = writeln!(self.buf, "  subi {}, {}, {}", dest, src, imm);  // 立即数减法 
   }

   /// 内存访问指令

   pub fn emit_lw(&mut self, dest: &str, offset: i32, base: &str) {
    let _ = writeln!(self.buf, "  lw {}, {}({})", dest, offset, base);  // 从内存加载数据
   }
   pub fn emit_sw(&mut self, src: &str, offset: i32, base: &str) {
    let _ = writeln!(self.buf, "  sw {}, {}({})", src, offset, base);  // 存储数据到内存
   }

   /// 比较和分支指令

   pub fn emit_beq(&mut self, src1: &str, src2: &str, label: &str) {
    let _ = writeln!(self.buf, "  beq {}, {}, {}", src1, src2, label);  // 相等比较分支
   }
   pub fn emit_bne(&mut self, src1: &str, src2: &str, label: &str) {
    let _ = writeln!(self.buf, "  bne {}, {}, {}", src1, src2, label);  // 不等比较分支
   }
   pub fn emit_blt(&mut self, src1: &str, src2: &str, label: &str) {
    let _ = writeln!(self.buf, "  blt {}, {}, {}", src1, src2, label);  // 小于比较分支
   }
   pub fn emit_bgt(&mut self, src1: &str, src2: &str, label: &str) {
    let _ = writeln!(self.buf, "  bgt {}, {}, {}", src1, src2, label);  // 大于比较分支
   }
   pub fn emit_ble(&mut self, src1: &str, src2: &str, label: &str) {
    let _ = writeln!(self.buf, "  ble {}, {}, {}", src1, src2, label);  // 小于等于比较分支
   }
   pub fn emit_bge(&mut self, src1: &str, src2: &str, label: &str) {
    let _ = writeln!(self.buf, "  bge {}, {}, {}", src1, src2, label);  // 大于等于比较分支 
   }

    pub fn emit_sgt(&mut self, src1: &str, src2: &str, src3: &str) {
        let _ = writeln!(self.buf, "  sgt {}, {}, {}", src1, src2, src3);
    }

    pub fn emit_seqz(&mut self,src1: &str, src2: &str) {
        let _ = writeln!(self.buf, "  seqz {}, {}", src1, src2);
    }

    pub fn emit_snez(&mut self,src1: &str, src2: &str) {
        let _ = writeln!(self.buf, "  snez {}, {}", src1, src2);
    }
    pub fn emit_slt(&mut self, src1: &str, src2: &str, src3: &str) {
        let _ = writeln!(self.buf, "  slt {}, {}, {}", src1, src2, src3);
    }

    pub fn emit_sle(&mut self, src1: &str, src2: &str, src3: &str) {
        let _ = writeln!(self.buf, "  sle {}, {}, {}", src1, src2, src3);
    }


    pub fn emit_sge(&mut self, src1: &str, src2: &str, src3: &str) {
        let _ = writeln!(self.buf, "  sge {}, {}, {}", src1, src2, src3);
    }
   /// 跳转指令
   pub fn emit_j(&mut self, label: &str) {
    let _ = writeln!(self.buf, "  j {}", label); // 无条件跳转
   }
   pub fn emit_jal(&mut self, label: &str) {
    let _ = writeln!(self.buf, "  jal {}", label); // 跳转并链接
   }
   pub fn emit_jr(&mut self, reg: &str) {
    let _ = writeln!(self.buf, "  jr {}", reg); // 寄存器跳转
   }

   /// 数据移动指令
   pub fn emit_mv(&mut self, dest: &str, src: &str) {
    let _ = writeln!(self.buf, "  mv {}, {}", dest, src); // 数据移动
   }

   /// 系统调用指令
   pub fn emit_ecall(&mut self) {
    let _ = writeln!(self.buf, "  ecall"); // 执行系统调用
   }
   
   /// 系统调用指令
   pub fn emit_syscall(&mut self) {
    let _ = writeln!(self.buf, "  syscall"); // 系统调用
   }

   /// 获取汇编代码
   pub fn emit(&self) -> String {
    self.buf.clone()
   }

   /// 添加空行
   pub fn emit_empty_line(&mut self) {
    let _ = writeln!(self.buf);
   }
}