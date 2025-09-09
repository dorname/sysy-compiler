use pest::iterators::Pair;
use std::io;
use std::io::Write;
use pest::Parser;
use pest::iterators::Pairs;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "pests/parser.pest"]
pub struct Formatter;
fn parse_file(input: &'_ str) -> Option<Pairs<'_, Rule>> {
    match Formatter::parse(Rule::File, input) {
        Ok(pairs) => Some(pairs),
        Err(_) => {
            None
        }
    }
}

pub fn fmt(input: &str){
    let _ = format_code(input, io::stdout());
}

pub fn format_code<W:Write>(input: &str,mut w: W) -> io::Result<()> {
    let pairs = parse_file(input);
    if pairs.is_none() {
        return Ok(());
    }
    // 把File规则的第一个pair拿出来
    let pairs = pairs.unwrap().next().unwrap();
    // 继续把File规则的内容拿出来
    let pairs = pairs.into_inner().next().unwrap();
    // 把编译单元的内容拿出来
    let pairs = pairs.into_inner();
    let mut fmt_str: String = String::new();
    dbg!(&pairs);
    fn fmt(pair: Pair<Rule>,fmt_str: &mut String) {
        match pair.as_rule() {
            Rule::FuncDef => {
                let inner = pair.into_inner().next().unwrap();
                fmt(inner,fmt_str);
            }
            Rule::Decl => {
                let inner = pair.into_inner().next().unwrap();
                fmt(inner,fmt_str);

            }
            Rule::FuncType => {
                let inner = pair.into_inner().next().unwrap();
                fmt(inner,fmt_str);
            }
            Rule::ConstDecl => {
                let inner = pair.into_inner().next().unwrap();
                fmt(inner,fmt_str);
            },
            Rule::Semicolon => {
                // 如果结尾是"return ",则去掉空格再添加分号
                // 否则直接添加分号和换行
                if fmt_str.ends_with("return ") {
                    fmt_str.pop();
                    fmt_str.push(';');
                } else {
                    let input = format!("{}\n",pair.as_str());
                    fmt_str.push_str(&input);
                }
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
                fmt_str.push_str(&input);
            }
            _ => {
                let input = format!("{} ",pair.as_str());
                fmt_str.push_str(&input);
            }
        }
    }
    pairs.for_each(|pair| {
        let _ = fmt(pair,&mut fmt_str);
    });
    writeln!(w, "{}", fmt_str)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    const FILE_PATH: &str = "tests/lab2/";
    use super::*;

    #[test]
    fn test_format_code() {
        // 1、把内容输出内存缓冲区
        let filename = FILE_PATH.to_string() + "lab2_example2.txt";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = format_code(&file, io::stdout());
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
}
