use pest::iterators::Pair;
use std::io;
use std::io::{stdout, Write};
use pest::Parser;
use pest::iterators::Pairs;
use pest_derive::Parser;


#[derive(Parser)]
#[grammar = "pests/parser.pest"]
pub struct FParser;
fn parse_file(input: &'_ str) -> Option<Pairs<'_, Rule>> {
    match FParser::parse(Rule::File, input) {
        Ok(pairs) => Some(pairs),
        Err(_) => {
            None
        }
    }
}

pub struct Formatter<'a,W: Write> {
    deep: usize,
    input: &'a str,
    writer: &'a mut W,
    output: String,
    err_output:String,
    forbid_newline: bool,
    forbid_semicolon_newline: bool,
    is_first_function: bool,
}

impl<'a,W:Write> Formatter<'a, W>{
    pub fn new(deep: usize, input: &'a str, writer: &'a mut W) -> Self {
        Self {
            deep,
            input,
            output: String::new(),
            err_output: String::new(),
            writer,
            forbid_newline: false,
            forbid_semicolon_newline: false,
            is_first_function: true,
        }
    }

    pub fn fmt(&mut self,pair: Pair<Rule>) {
        fn build_next_line_str(count:usize)-> String {
            if count == 0 {
                return "\n".to_string();
            }
            format!("\n{0:1$}","", 4*count)
        }
        match pair.as_rule() {
                Rule::FuncDef => {
                    // 处理函数定义前的空行
                    if !self.is_first_function {
                        self.output.push_str("\n");
                    }
                    self.is_first_function = false;
                    
                    let inner = pair.into_inner();
                    for p in inner {
                        self.fmt(p);
                    }
                }
                Rule::VarDecl |
                Rule::VarDef |
                Rule::ConstDecl |
                Rule::ConstDef |
                Rule::ArrayDims |
                Rule::ConstInitVal |
                Rule::FuncFParams |
                Rule::FuncFParam |
                Rule::InitVal |
                Rule::BlockItem |
                // Rule::Stmt |
                Rule::LVal |
                Rule::PrimaryExp |
                Rule::CallExp |
                Rule::FuncRParams |
                Rule::UnaryExp |
                Rule::MulExp |
                Rule::AddExp |
                Rule::RelExp |
                Rule::EqExp |
                Rule::LAndExp |
                Rule::LOrExp |
                Rule::Number |
                Rule::Block=> {
                    let inner = pair.into_inner();
                    for p in inner {
                        self.fmt(p);
                    }
                }
                Rule::Stmt => {
                    let inner = pair.into_inner();
                    let first_pair = inner.peek().iter().next().unwrap().as_rule();
                    match first_pair {
                        Rule::Block => {
                            for p in inner {
                                self.fmt(p);
                            }
                        }
                        _ => {
                            // 1、如果前置是") "
                            //    缩进加1且挪到下一行
                            if self.output.ends_with(") ") {
                                self.deep += 1;
                                self.forbid_semicolon_newline = true;
                                self.output.push_str(&build_next_line_str(self.deep));
                                for p in inner {
                                    let _q = p.as_str();
                                    self.fmt(p);
                                }
                                self.deep -= 1;
                                self.output.push_str(&build_next_line_str(self.deep));
                                self.forbid_semicolon_newline = false;
                            } else if self.output.ends_with("else ") && first_pair.ne(&Rule::If) {
                                // 处理else后面跟非if语句的情况，需要缩进
                                self.deep += 1;
                                self.output.push_str(&build_next_line_str(self.deep));
                                self.deep -= 1;
                                for p in inner {
                                    let _q = p.as_str();
                                    self.fmt(p);
                                }
                            } else {
                                for p in inner {
                                    self.fmt(p);
                                }
                            }
                        }
                    }
                }
                Rule::Decl => {
                    let inner = pair.into_inner();
                    self.forbid_newline = true;
                    for p in inner {
                        self.fmt(p);
                    }
                    self.forbid_newline = false;
                }
                Rule::Exp |
                Rule::Cond |
                Rule::ConstExp=> {
                    let p = pair.into_inner().next().unwrap();
                    self.fmt(p);
                }
                Rule::Semicolon => {
                    while self.output.ends_with(" ") {
                        self.output.pop();
                    }
                    let input = format!("{}",pair.as_str());
                    self.output.push_str(&input);
                    if !self.forbid_semicolon_newline {
                        self.output.push_str(&build_next_line_str(self.deep));
                    }
                }
                Rule::CloseParen => {
                    // 如果结尾是"return ",则去掉空格再添加分号
                    // 否则直接添加分号和换行
                    while self.output.ends_with(" ") {
                        self.output.pop();
                    }
                    let input = format!("{} ",pair.as_str());
                    self.output.push_str(&input);
                }
                Rule::Plus |
                Rule::Minus |
                Rule::Mul |
                Rule::Div |
                Rule::Mod |
                Rule::Assign |
                Rule::Equal |
                Rule::NotEqual |
                Rule::Less |
                Rule::LessEqual |
                Rule::Greater |
                Rule::GreaterEqual |
                Rule::And |
                Rule::Or |
                Rule::Not => {
                    let input = format!(" {} ",pair.as_str());
                    self.output.push_str(&input);
                }
                Rule::UnaryOp |
                Rule::Ident |
                Rule::OpenParen |
                Rule::OpenBracket |
                Rule::CloseBracket |
                Rule::DecConst |
                Rule::HexConst |
                Rule::OctConst => {
                    let input = format!("{}",pair.as_str());
                    self.output.push_str(&input);
                }
                Rule::OpenBrace => {
                    let input = format!("{}",pair.as_str());
                    self.output.push_str(&input);
                    if !self.forbid_newline {
                        self.deep+=1;
                        self.output.push_str(&build_next_line_str(self.deep));
                    }
                }
                Rule::CloseBrace => {
                    let input = format!("{}",pair.as_str());
                    // 只有允许换行的情况才需要把前面空格吃掉
                    if !self.forbid_newline {
                        let pop_count = 4usize;
                        for _ in 0..pop_count {
                            self.output.pop();
                        }
                    }
                    self.output.push_str(&input);
                    if !self.forbid_newline {
                        self.deep-=1;
                        self.output.push_str(&build_next_line_str(self.deep));
                    }
                }
                Rule::ErrorStmt => {
                    let input = format!("Error type A at Line {}: {}.\n",pair.line_col().0,pair.as_str());
                    self.err_output.push_str(&input);
                }
                _ => {
                    let input = format!("{} ",pair.as_str());
                    self.output.push_str(&input);
                }
        }
    }
    /// 清理字符串中的多余空行，仅保留符合条件的空行。
    ///
    /// # 参数
    /// - `input`: 需要处理的多行字符串。
    ///
    /// # 返回值
    /// 返回一个新的字符串：
    /// - 移除了不必要的空行。
    /// - 保留了函数之间的空行（由 [`should_keep_blank_line`] 决定）。
    /// - 去掉了末尾多余的空行。
    ///
    /// # 行为说明
    /// 1. 遍历输入的每一行：
    ///    - 如果行非空，直接保留。
    ///    - 如果行为空，调用 [`should_keep_blank_line`] 判断是否保留该空行。
    /// 2. 在拼接结果前，移除末尾连续的空行。
    ///
    /// # 示例
    /// ```rust
    /// # struct Cleaner;
    /// # impl Cleaner {
    /// #     fn should_keep_blank_line(&self, _lines: &[&str], _i: usize) -> bool { true }
    /// #     fn clean_extra_blank_lines(&self, input: &str) -> String {
    /// #         let lines: Vec<&str> = input.lines().collect();
    /// #         let mut result = Vec::new();
    /// #         for (i, line) in lines.iter().enumerate() {
    /// #             if line.trim().is_empty() {
    /// #                 if self.should_keep_blank_line(&lines, i) {
    /// #                     result.push(*line);
    /// #                 }
    /// #             } else {
    /// #                 result.push(*line);
    /// #             }
    /// #         }
    /// #         while let Some(last_line) = result.last() {
    /// #             if last_line.trim().is_empty() {
    /// #                 result.pop();
    /// #             } else {
    /// #                 break;
    /// #             }
    /// #         }
    /// #         result.join("\n")
    /// #     }
    /// # }
    /// let cleaner = Cleaner;
    /// let input = "fn foo() {}\n\nfn bar() {}\n\n";
    /// let output = cleaner.clean_extra_blank_lines(input);
    /// assert!(output.ends_with("fn bar() {}"));
    /// ```
    ///
    /// [`should_keep_blank_line`]: Self::should_keep_blank_line
    fn clean_extra_blank_lines(&self, input: &str) -> String {
        let lines: Vec<&str> = input.lines().collect();
        let mut result = Vec::new();
        
        for (i, line) in lines.iter().enumerate() {
            if line.trim().is_empty() {
                // 检查是否应该保留这个空行
                if self.should_keep_blank_line(&lines, i) {
                    result.push(*line);
                }
                // 否则跳过这个空行
            } else {
                result.push(*line);
            }
        }
        
        // 移除末尾的空行
        while let Some(last_line) = result.last() {
            if last_line.trim().is_empty() {
                result.pop();
            } else {
                break;
            }
        }
        
        result.join("\n")
    }

    /// 判断某个空行是否应该被保留。
    ///
    /// # 参数
    /// - `lines`: 源代码的所有行，按顺序存放的字符串切片数组。
    /// - `blank_line_index`: 当前空行在 `lines` 中的索引。
    ///
    /// # 返回值
    /// - `true`：如果该空行应该被保留；
    /// - `false`：否则移除该空行。
    ///
    /// # 保留规则
    /// - 当空行前一行以 `}` 结尾，且后一行是函数定义时，保留该空行。
    ///   - 例如：
    ///   ```c
    ///   void foo() {
    ///       // ...
    ///   }
    ///
    ///   int bar() {
    ///       // ...
    ///   }
    ///   ```
    ///   上例中的空行会被保留，以便区分函数。
    ///
    /// # 其他情况
    /// - 所有不符合上述条件的空行，都会被移除。
    fn should_keep_blank_line(&self, lines: &[&str], blank_line_index: usize) -> bool {
        // 查找前一个非空行
        let mut prev_non_empty = None;
        for i in (0..blank_line_index).rev() {
            if !lines[i].trim().is_empty() {
                prev_non_empty = Some(i);
                break;
            }
        }
        
        // 查找后一个非空行
        let mut next_non_empty = None;
        for i in (blank_line_index + 1)..lines.len() {
            if !lines[i].trim().is_empty() {
                next_non_empty = Some(i);
                break;
            }
        }
        
        // 如果前后都有非空行，检查是否应该保留空行
        if let (Some(prev_idx), Some(next_idx)) = (prev_non_empty, next_non_empty) {
            let prev_line = lines[prev_idx].trim();
            let next_line = lines[next_idx].trim();
            
            // 保留函数间的空行（前一行以}结尾，后一行是函数定义）
            if prev_line.ends_with("}") && 
               (next_line.starts_with("int ") || next_line.starts_with("void ") || next_line.starts_with("const ")) {
                return true;
            }
        }
        
        // 其他情况不保留空行
        false
    }

    pub fn format_code(&mut self) -> io::Result<()> {
        let pairs = parse_file(self.input);
        if pairs.is_none() {
            return Ok(());
        }
        // 把File规则的第一个pair拿出来
        let pairs = pairs.unwrap().next().unwrap();
        // 继续把File规则的内容拿出来
        let pairs = pairs.into_inner().next().unwrap();
        // 把编译单元的内容拿出来
        let pairs = pairs.into_inner();
        dbg!(&pairs);
        pairs.for_each(|pair| {
            self.fmt(pair);
        });
        if self.err_output.len() != 0 {
            writeln!(self.writer, "{}", self.err_output)?;
        }else {
            // 清除由于递归引起的多余空白行，但保留函数间的单个空行
            let cleaned_output = self.clean_extra_blank_lines(&self.output);
            writeln!(self.writer, "{}", cleaned_output)?;
        }
        Ok(())
    }
}

pub fn fmt(input: &str){
    let mut binding = stdout();
    let mut formatter = Formatter::new(0usize, input, &mut binding);
    formatter.format_code().unwrap();
}



#[cfg(test)]
mod tests {
    const FILE_PATH: &str = "tests/lab2/";
    const OTHER_FILE_PATH: &str = "tests/lab3/";
    use super::*;

    #[test]
    fn test_example_1() {
        // 1、把内容输出内存缓冲区
        let filename = FILE_PATH.to_string() + "lab2_example1.txt";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut formatter = Formatter::new(0usize, &file, &mut binding);
        formatter.format_code().unwrap();
    }

    #[test]
    fn test_example_2() {
        // 1、把内容输出内存缓冲区
        let filename = FILE_PATH.to_string() + "lab2_example2.txt";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut formatter = Formatter::new(0usize, &file, &mut binding);
        formatter.format_code().unwrap();
    }

    #[test]
    fn test_example_3() {
        // 1、把内容输出内存缓冲区
        let filename = FILE_PATH.to_string() + "lab2_example3.txt";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut formatter = Formatter::new(0usize, &file, &mut binding);
        formatter.format_code().unwrap();
    }

    #[test]
    #[ignore]
    fn test_format_file() {
        let mut fmt_str = String::new();
        fmt_str.push_str("fn main() {return ");
        // 如果上一个字符串是return,则直接添加分号
        if fmt_str.ends_with("return ") {
            fmt_str.pop();
            fmt_str.push(';');
        } else {
            let input = format!("{}\n",";");
            fmt_str.push_str(&input);
        }
        println!("{}", fmt_str);
    }


    #[test]
    fn test_lab2_in1(){
        let filename = FILE_PATH.to_string() + "lab2_in1.txt";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut formatter = Formatter::new(0usize, &file, &mut binding);
        formatter.format_code().unwrap();
    }

    #[test]
    fn test_lab2_in2(){
        let filename = FILE_PATH.to_string() + "lab2_in2.txt";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut formatter = Formatter::new(0usize, &file, &mut binding);
        formatter.format_code().unwrap();
    }

    #[test]
    fn test_lab2_in3(){
        let filename = FILE_PATH.to_string() + "lab2_in3.txt";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut formatter = Formatter::new(0usize, &file, &mut binding);
        formatter.format_code().unwrap();
    }

    #[test]
    fn test_lab2_in4(){
        let filename = FILE_PATH.to_string() + "lab2_in4.txt";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut formatter = Formatter::new(0usize, &file, &mut binding);
        formatter.format_code().unwrap();
    }

    #[test]
    fn test_lab2_in5(){
        let filename = FILE_PATH.to_string() + "lab2_in5.txt";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut formatter = Formatter::new(0usize, &file, &mut binding);
        formatter.format_code().unwrap();
    }

    #[test]
    #[ignore]
    fn check(){
        let filename = OTHER_FILE_PATH.to_string() + "normaltest01-1.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut formatter = Formatter::new(0usize, &file, &mut binding);
        formatter.format_code().unwrap();
    }


}
