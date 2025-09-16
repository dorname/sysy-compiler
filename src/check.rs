use std::io;
use pest::iterators::Pair;
use std::borrow::Cow;
use std::fmt::Display;
use std::io::Write;
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
    fn syn_check(&mut self) -> io::Result<()> {
        if let Some(mut pairs) = parse_file(self.input) {
            // 把File规则的第一个pair拿出来
            let pairs = pairs.next().unwrap();
            // 继续把File规则的内容拿出来
            let pairs = pairs.into_inner().next().unwrap();
            // 把编译单元的内容拿出来
            let pairs = pairs.into_inner();
            for pair in pairs {
                self.check(pair);
            }
            writeln!(self.writer, "{}", self.output)?;
        } else {
            writeln!(self.writer, "Syntax error")?;
        }
        Ok(())
    }

    fn check(&mut self,pair:Pair<Rule>) {
        match pair.as_rule() {
            _ => { /* todo 语义检查逻辑 */ }
        }
    }
}


struct PairCheckResult<'a> {
    line_no: String,
    err_types: Vec<CheckError>,
    output: Cow<'a,str>,
}

impl <'a> PairCheckResult<'a> {
    fn build_str(&mut self) {
        self.output = if self.err_types.is_empty() {
            Cow::Borrowed("")
        } else {
            // todo 后面可能要改成filter_map
            Cow::Owned(self.err_types.iter()
                .map(|e| format!("Error type {} at line{}: {}", e.get_kind(), self.line_no, e.build_str()))
                .collect::<Vec<String>>().join("\n"))
        };
    }
}



#[derive(Debug,Clone,Copy)]
#[repr(u8)]
pub enum ErrorKind {
    UndefinedVal = 1,
    UndefinedFunc = 2,
    RedefineVal = 3,
    RedefineFunc = 4,
    TypeMismatch = 5,
    ParamMismatch = 6,
    ReturnMismatch = 7,
    UnexpectedType = 8,
    UnexpectedOperator = 9,
    UnlegalFuncCall = 10,
    UnexpectedAssign = 11,
    Other = 0,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", *self as u8)
    }
}


#[derive(Debug)]
pub enum CheckError {
    UndefinedVal(ErrorKind, &'static str,Option<String>),
    UndefinedFunc(ErrorKind, &'static str,Option<String>),
    RedefineVal(ErrorKind, &'static str,Option<String>),
    RedefineFunc(ErrorKind, &'static str,Option<String>),
    TypeMismatch(ErrorKind, &'static str,Option<String>),
    ParamMismatch(ErrorKind, &'static str,Option<String>),
    ReturnMismatch(ErrorKind, &'static str,Option<String>),
    UnexpectedType(ErrorKind, &'static str,Option<String>),
    UnexpectedOperator(ErrorKind, &'static str,Option<String>),
    UnlegalFuncCall(ErrorKind, &'static str,Option<String>),
    UnexpectedAssign(ErrorKind, &'static str,Option<String>),
    Other(ErrorKind, &'static str,Option<String>),
}


impl CheckError {
    fn new(kind: ErrorKind,error_tip:Option<String>) -> Self {
        match kind {
            ErrorKind::UndefinedVal => CheckError::UndefinedVal(kind, "Undefined variable",error_tip),
            ErrorKind::UndefinedFunc => CheckError::UndefinedFunc(kind, "Undefined function",error_tip),
            ErrorKind::RedefineVal => CheckError::RedefineVal(kind, "Redefined variable",error_tip),
            ErrorKind::RedefineFunc => CheckError::RedefineFunc(kind, "Redefined function",error_tip),
            ErrorKind::TypeMismatch => CheckError::TypeMismatch(kind, "Type mismatch",error_tip),
            ErrorKind::ParamMismatch => CheckError::ParamMismatch(kind, "Parameter mismatch",error_tip),
            ErrorKind::ReturnMismatch => CheckError::ReturnMismatch(kind, "Return type mismatch",error_tip),
            ErrorKind::UnexpectedType => CheckError::UnexpectedType(kind, "Unexpected type",error_tip),
            ErrorKind::UnexpectedOperator => CheckError::UnexpectedOperator(kind, "Unexpected operator",error_tip),
            ErrorKind::UnlegalFuncCall => CheckError::UnlegalFuncCall(kind, "Illegal function call",error_tip),
            ErrorKind::UnexpectedAssign => CheckError::UnexpectedAssign(kind, "Unexpected assignment",error_tip),
            _ => CheckError::Other(kind, "Other error",error_tip)

        }
    }

    fn get_kind(&self) -> ErrorKind {
        match self {
            CheckError::UndefinedVal(kind, _, _) |
            CheckError::UndefinedFunc(kind, _, _)  |
            CheckError::RedefineVal(kind, _, _)  |
            CheckError::RedefineFunc(kind, _, _)  |
            CheckError::TypeMismatch(kind, _, _)  |
            CheckError::ParamMismatch(kind, _, _)  |
            CheckError::ReturnMismatch(kind, _, _)  |
            CheckError::UnexpectedType(kind, _, _)  |
            CheckError::UnexpectedOperator(kind, _, _) |
            CheckError::UnlegalFuncCall(kind, _, _)  |
            CheckError::UnexpectedAssign(kind, _, _)  |
            CheckError::Other(kind, _, _) => *kind,
        }
    }

    fn get_tip(&self) -> Option<String> {
        match self {
            CheckError::UndefinedVal(_, _, tip) |
            CheckError::UndefinedFunc(_, _, tip)  |
            CheckError::RedefineVal(_, _, tip)  |
            CheckError::RedefineFunc(_, _, tip)  |
            CheckError::TypeMismatch(_, _, tip)  |
            CheckError::ParamMismatch(_, _, tip)  |
            CheckError::ReturnMismatch(_, _, tip)  |
            CheckError::UnexpectedType(_, _, tip)  |
            CheckError::UnexpectedOperator(_, _, tip) |
            CheckError::UnlegalFuncCall(_, _, tip)  |
            CheckError::UnexpectedAssign(_, _, tip)  |
            CheckError::Other(_, _, tip) => tip.clone(),
        }
    }

    fn get_msg(&self) -> &'static str {
        match self {
            CheckError::UndefinedVal(_, msg, _) |
            CheckError::UndefinedFunc(_, msg, _)  |
            CheckError::RedefineVal(_, msg, _)  |
            CheckError::RedefineFunc(_, msg, _)  |
            CheckError::TypeMismatch(_, msg, _)  |
            CheckError::ParamMismatch(_, msg, _)  |
            CheckError::ReturnMismatch(_, msg, _)  |
            CheckError::UnexpectedType(_, msg, _)  |
            CheckError::UnexpectedOperator(_, msg, _) |
            CheckError::UnlegalFuncCall(_, msg, _)  |
            CheckError::UnexpectedAssign(_, msg, _)  |
            CheckError::Other(_, msg, _) => msg,
        }
    }

    fn build_str(&self) -> String {
        let tip = self.get_tip();
        if let Some(t) = tip {
            format!("{}:{}", self.get_msg(),t)
        }else {
            format!("{}", self.get_msg())
        }
    }
}
