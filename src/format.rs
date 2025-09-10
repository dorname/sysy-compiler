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
}

impl<'a,W:Write> Formatter<'a, W>{
    pub fn new(deep: usize, input: &'a str, writer: &'a mut W) -> Self {
        Self {
            deep,
            input,
            output: String::new(),
            err_output: String::new(),
            writer,
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
                Rule::FuncDef |
                Rule::Decl |
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
                Rule::Stmt |
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
                Rule::Exp |
                Rule::Cond |
                Rule::ConstExp=> {
                    let p = pair.into_inner().next().unwrap();
                    self.fmt(p);
                }
                Rule::Semicolon => {
                    // 如果结尾是"return ",则去掉空格再添加分号
                    // 否则直接添加分号和换行
                    while self.output.ends_with(" ") {
                        self.output.pop();
                    }
                    let input = format!("{}",pair.as_str());
                    self.output.push_str(&input);
                    self.output.push_str(&build_next_line_str(self.deep));
                }
                Rule::CloseParen => {
                    // 如果结尾是"return ",则去掉空格再添加分号
                    // 否则直接添加分号和换行
                    while self.output.ends_with(" ") {
                        self.output.pop();
                    }
                    let input = format!("{}",pair.as_str());
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
                    self.deep+=1;
                    self.output.push_str(&input);
                    self.output.push_str(&build_next_line_str(self.deep));

                }
                Rule::CloseBrace => {
                    let input = format!("{}",pair.as_str());
                    let pop_count = 4usize;
                    for _ in 0..pop_count {
                        self.output.pop();
                    }
                    self.output.push_str(&input);
                    self.deep-=1;
                    self.output.push_str(&build_next_line_str(self.deep));
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
        // dbg!(&pairs);
        pairs.for_each(|pair| {
            self.fmt(pair);
        });
        if self.err_output.len() != 0 {
            writeln!(self.writer, "{}", self.err_output)?;
        }else {
            writeln!(self.writer, "{}", self.output)?;
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
}
