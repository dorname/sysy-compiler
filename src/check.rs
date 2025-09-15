use std::io::Write;
use clap::builder::Str;
use pest::iterators::Pairs;
use pest_derive::Parser;
use pest::Parser;

#[derive(Parser)]
#[grammar = "pests/parser.pest"]
pub struct CParser;
fn parse_file(input: &'_ str) -> Option<Pairs<'_, Rule>> {
    match CParser::parse(Rule::File, input) {
        Ok(pairs) => Some(pairs),
        Err(_) => {
            None
        }
    }
}

pub struct Checker<'a,W:Write>{
    input: &'a str,
    writer: &'a mut W,
    output: String,
}

impl<'a,W: Write> Checker<'a,W> {
    fn new(input: &'a str, writer: &'a mut W) -> Self {
        Checker {
            input,
            writer,
            output: String::new(),
        }
    }
    fn check(&mut self) {
        if let Some(pairs) = parse_file(self.input) {
           //todo()
        } else {
            writeln!(self.writer, "Syntax error").unwrap();
        }
    }
}


struct PairCheckResult {
    errors: Vec<String>,
    line_no: String,
    err_type: CheckError
}

pub enum CheckError {
    UndefinedVal,
    UndefinedFunc,
    RedefineVal,
    RedefineFunc,
    TypeMismatch,
    ParamMismatch,
    ReturnMismatch,
    UnexpectedType,
    UnexpectedOperator,
    UnlegalFuncCall,
    UnexpectedAssign,
    Other(String),
}