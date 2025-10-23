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

    pub fn op2(&mut self, op: &str, dest: &str, lhs: &str, rhs: &str) {
        let _ = writeln!(self.buf, "  {} {}, {}, {}", op, dest, lhs, rhs);
    }

    pub fn addi(&mut self, dest: &str, src: &str, imm: i32) {
        let _ = writeln!(self.buf, "  addi {}, {}, {}", dest, src, imm);
    }

    pub fn emit(&self) -> &str { &self.buf }
}