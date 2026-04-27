use pest::iterators::{Pair, Pairs};
use std::fmt::{Display, Formatter};
use std::io;
use pest::Parser;
use pest_derive::Parser;
use crate::lexer::IntegerConst::{Hex, Octal};
use crate::utils::{hex_to_int, oct_to_int};
use std::io::Write;
#[derive(Parser)]
#[grammar = "pests/lexer.pest"]
pub struct ExpressionParser;

#[derive(Debug, PartialEq,Clone)]
pub enum Token {
    Identifier(String),
    Operator(Operator),
    Type(Type),
    Flow(Flow),
    IntegerConst(IntegerConst),
    ErrorSyntax(ErrorSyntax)
}



impl Display for Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Identifier(s) => write!(f, "IDENT {}", s),
            Token::Operator(op) => write!(f, "{}", op),
            Token::Type(t) => write!(f, "{}", t),
            Token::Flow(flow) => write!(f, "{}", flow),
            Token::IntegerConst(i) => write!(f, "{}", i),
            Token::ErrorSyntax(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, PartialEq,Clone)]
pub enum ErrorSyntax {
    VarError(String),
    InvalidHex(String),
    InvalidOctal(String),
    InvalidInteger(String),
    InvalidOperator(String),
    UnKnownError(String),
}

impl Display for ErrorSyntax {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorSyntax::VarError(s) => write!(f, "VAR_ERROR {}", s),
            ErrorSyntax::InvalidHex(s) => write!(f, "INVALID_HEX {}", s),
            ErrorSyntax::InvalidOctal(s) => write!(f, "INVALID_OCTAL {}", s),
            ErrorSyntax::InvalidInteger(s) => write!(f, "INVALID_INTEGER {}", s),
            ErrorSyntax::InvalidOperator(s) => write!(f, "INVALID_OPERATOR {}", s),
            ErrorSyntax::UnKnownError(s) => write!(f, "UN_KNOWN_ERROR {}", s),
        }
    }
}

#[derive(Debug, PartialEq,Clone)]
pub enum IntegerConst {
    Hex(String), // 十六进制
    Octal(String), // 八进制
    Dec(String), // 十进制
}

impl Display for IntegerConst {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Hex(s) => {
                // 十六进制转十进制
                let s = hex_to_int(s).expect("invalid hex");
                write!(f, "INTEGER_CONST {}", s)
            },
            Octal(s) =>{
                let s = oct_to_int(s).expect("invalid hex");
                write!(f, "INTEGER_CONST {}", s)
            },
            IntegerConst::Dec(s) => write!(f, "INTEGER_CONST {}", s),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Flow {
    If,
    Else,
    While,
    Break,
    Continue,
    Return,
}

impl Display for Flow {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Flow::If => write!(f, "IF if"),
            Flow::Else => write!(f, "ELSE else"),
            Flow::While => write!(f, "WHILE while"),
            Flow::Break => write!(f, "BREAK break"),
            Flow::Continue => write!(f, "CONTINUE continue"),
            Flow::Return => write!(f, "RETURN return"),
        }
    }
}

#[derive(Debug, PartialEq,Clone)]
pub enum Type {
    Int,
    Float,
    Void,
    Const
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Int => write!(f, "INT int"),
            Type::Float => write!(f, "FLOAT float"),
            Type::Void => write!(f, "VOID void"),
            Type::Const => write!(f, "CONST const"),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Operator {
    Plus,
    Minus,
    Mul,
    Div,
    Mod,
    Assign,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
    Not,
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
    OpenBracket,
    CloseBracket,
    Comma,
    Semicolon,
}

impl Display for Operator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Operator::Plus => write!(f, "PLUS +"),
            Operator::Minus => write!(f, "MINUS -"),
            Operator::Mul => write!(f, "MUL *"),
            Operator::Div => write!(f, "DIV /"),
            Operator::Mod => write!(f, "MOD %"),
            Operator::Assign => write!(f, "ASSIGN ="),
            Operator::Equal => write!(f, "EQ =="),
            Operator::NotEqual => write!(f, "NEQ !="),
            Operator::Less => write!(f, "LT <"),
            Operator::LessEqual => write!(f, "LE <="),
            Operator::Greater => write!(f, "GT >"),
            Operator::GreaterEqual => write!(f, "GE >="),
            Operator::And => write!(f, "AND &&"),
            Operator::Or => write!(f, "OR ||"),
            Operator::Not => write!(f, "NOT !"),
            Operator::OpenParen => write!(f, "L_PAREN ("),
            Operator::CloseParen => write!(f, "R_PAREN )"),
            Operator::OpenBrace => write!(f, "L_BRACE {{"),
            Operator::CloseBrace => write!(f, "R_BRACE }}"),
            Operator::Comma => write!(f, "COMMA ,"),
            Operator::Semicolon => write!(f, "SEMICOLON ;"),
            Operator::OpenBracket => write!(f, "L_BRACKT ["),
            Operator::CloseBracket => write!(f, "R_BRACKT ]"),
        }
    }
}

fn parse_file(input: &'_ str) -> Option<Pairs<'_, Rule>> {

    match ExpressionParser::parse(Rule::File, input) {
        Ok(pairs) => Some(pairs),
        Err(_) => {
            eprintln!("error parse");
            // let line_no =  if let pest::error::LineColLocation::Pos((line,_)) = e.line_col {
            //     line
            // } else {
            //     0
            // };
            // // 提取e中的错误信息
            // // 只取错误信息，不要自动生成的“期望 …”提示
            // let err_msg = e.to_string();
            // eprintln!("Error type A at Line {}:\n {}.",line_no,err_msg);
            None
        }
    }
}

pub fn tokenizer<W:Write>(input: &str,mut w:W) ->  io::Result<()>  {
    let parse_result = parse_file(input);
    if parse_result.is_none() {
        return Ok(());
    }
    let pair =
        parse_result.unwrap()
            .next()
            .unwrap(); // 取第一个匹配
    let line_tokens= pair.into_inner()
        .filter_map(|p|{
            if p.as_rule() == Rule::EOI {
                None
            } else {
                Some((p.line_col().0,Token::from(p)))
            }
        }).collect::<Vec<(usize,Token)>>();
    let error_tokens:Vec<(usize,Token)>= line_tokens.iter().cloned().filter(|(_,token)|{
        matches!(token,Token::ErrorSyntax(_))
    }).collect();
    if error_tokens.len() != 0 {
        for (line,token) in error_tokens {
            writeln!(w, "Error type A at Line {}: {}.", line, token)?;
        }
    }else {
        for (line,token) in line_tokens {
            writeln!(w, "{} at Line {}.", token,line)?;
        }
    }
    Ok(())
}

pub fn tokenize(input: &str)  {
    let _ = tokenizer(input,io::stderr());
}


impl From<Pair<'_, Rule>> for Token {
    fn from(p: Pair<Rule>) -> Self {
        match p.as_rule() {
            Rule::Operator => {
                let op = p.into_inner().next().expect("Operator must have one child");
                match op.as_rule() {
                    Rule::Plus => Token::Operator(Operator::Plus),
                    Rule::Minus => Token::Operator(Operator::Minus),
                    Rule::Mul => Token::Operator(Operator::Mul),
                    Rule::Div => Token::Operator(Operator::Div),
                    Rule::Mod => Token::Operator(Operator::Mod),
                    Rule::Assign => Token::Operator(Operator::Assign),
                    Rule::Equal => Token::Operator(Operator::Equal),
                    Rule::NotEqual => Token::Operator(Operator::NotEqual),
                    Rule::Less => Token::Operator(Operator::Less),
                    Rule::LessEqual => Token::Operator(Operator::LessEqual),
                    Rule::Greater => Token::Operator(Operator::Greater),
                    Rule::GreaterEqual => Token::Operator(Operator::GreaterEqual),
                    Rule::And => Token::Operator(Operator::And),
                    Rule::Or => Token::Operator(Operator::Or),
                    Rule::Not => Token::Operator(Operator::Not),
                    Rule::OpenParen => Token::Operator(Operator::OpenParen),
                    Rule::CloseParen => Token::Operator(Operator::CloseParen),
                    Rule::OpenBrace => Token::Operator(Operator::OpenBrace),
                    Rule::CloseBrace => Token::Operator(Operator::CloseBrace),
                    Rule::OpenBracket => Token::Operator(Operator::OpenBracket),
                    Rule::CloseBracket => Token::Operator(Operator::CloseBracket),
                    Rule::Comma => Token::Operator(Operator::Comma),
                    Rule::Semicolon => Token::Operator(Operator::Semicolon),
                    _ => panic!("expected operator,found {}", op.as_str()),
                }
            },
            Rule::Flow => {
                let op = p.into_inner().next().expect("Operator must have one child");
                match op.as_rule() {
                    Rule::If => Token::Flow(Flow::If),
                    Rule::Else => Token::Flow(Flow::Else),
                    Rule::While => Token::Flow(Flow::While),
                    Rule::Break => Token::Flow(Flow::Break),
                    Rule::Continue => Token::Flow(Flow::Continue),
                    Rule::Return => Token::Flow(Flow::Return),
                    _ => panic!("expected flow,found {}", op.as_str()),
                }
            },
            Rule::Type => {
                let op = p.into_inner().next().expect("Operator must have one child");
                match op.as_rule() {
                    Rule::Int => Token::Type(Type::Int),
                    Rule::Float => Token::Type(Type::Float),
                    Rule::Void => Token::Type(Type::Void),
                    Rule::Const => Token::Type(Type::Const),
                    _ => panic!("expected type,found {}", op.as_str()),
                }
            },
            Rule::IntegerConst => {
                let op = p.into_inner().next().expect("Operator must have one child");
                match op.as_rule() {
                    Rule::Hex => Token::IntegerConst(Hex(op.as_str().to_string())),
                    Rule::Octal => Token::IntegerConst(Octal(op.as_str().to_string())),
                    Rule::Dec => Token::IntegerConst(IntegerConst::Dec(op.as_str().to_string())),
                    _ => panic!("expected integer const,found {}", op.as_str()),
                }
            },
            Rule::ErrorSyntax => {
                let op = p.into_inner().next().expect("Operator must have one child");
                match op.as_rule() {
                    // Rule::VarError => Token::ErrorSyntax(ErrorSyntax::VarError(op.as_str().to_string())),
                    Rule::InvalidHex => Token::ErrorSyntax(ErrorSyntax::InvalidHex(op.as_str().to_string())),
                    Rule::InvalidOctal => Token::ErrorSyntax(ErrorSyntax::InvalidOctal(op.as_str().to_string())),
                    // Rule::InvalidInteger => Token::ErrorSyntax(ErrorSyntax::InvalidInteger(op.as_str().to_string())),
                    Rule::InvalidOperator => Token::ErrorSyntax(ErrorSyntax::InvalidOperator(op.as_str().to_string())),
                    Rule::UnKnownError => Token::ErrorSyntax(ErrorSyntax::UnKnownError(op.as_str().to_string())),
                    _ => panic!("expected error syntax,found {}", op.as_str()),
                }
            },
            _ => {
                Token::Identifier(p.as_str().to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    const FILE_PATH: &str = "tests/lexer/";
    use crate::lexer::IntegerConst::{Hex, Octal};
    use super::*;

    #[test]
    #[ignore]
    fn test_eprintln() {
        eprintln!("{}",Operator::Plus);
        eprintln!("{}",Hex("0X12".to_string()));
        eprintln!("{}",Octal("012".to_string()));
    }


    #[test]
    fn test_arrays_and_radix_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "arrays_and_radix.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"arrays_and_radix.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_comments_and_hex_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "comments_and_hex.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"comments_and_hex.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_complex_errors_test_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "complex_errors_test.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"complex_errors_test.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_complex_expressions_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "complex_expressions.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"complex_expressions.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_comprehensive_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "comprehensive.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"comprehensive.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_edge_case_test_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "edge_case_test.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"edge_case_test.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_empty_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "empty.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"empty.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_error_invalid_char_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "error_invalid_char.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"error_invalid_char.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_invalid_character_error_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "invalid_character_error.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"invalid_character_error.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_keywords_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "keywords.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"keywords.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }
    #[test]
    fn test_leading_zeros_test_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "leading_zeros_test.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"leading_zeros_test.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_numbers_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "numbers.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"numbers.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_octal_edge_case_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "octal_edge_case.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"octal_edge_case.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_operators_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "operators.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"operators.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_sample1_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "sample1.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"sample1.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }


    #[test]
    fn test_sample2_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "sample2.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"sample2.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_sample3_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "sample3.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"sample3.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_simple_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "simple.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"simple.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }


    #[test]
    fn test_single_ampersand_test_files() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "single_ampersand_test.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string()+"single_ampersand_test.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }
}