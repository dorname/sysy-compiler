use crate::utils::eq_option_string;
use std::io;
use pest::iterators::Pair;
use std::borrow::Cow;
use std::fmt::Display;
use std::io::Write;
use pest::iterators::Pairs;
use pest_derive::Parser;
use pest::Parser;
use crate::utils::add_option_string;

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
    variable_del: Vec<VariableDel>,
    func_del: Vec<FuncDef>,
    current_func: Option<String>,
}

impl<'a,W: Write> Checker<'a,W> {
    fn new(input: &'a str, writer: &'a mut W) -> Self {
        Checker {
            input,
            writer,
            output: String::new(),
            variable_del: Vec::new(),
            func_del: Vec::new(),
            current_func: None,
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
            Rule::Decl => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.check(inner_pair);
                }
            }
            Rule::FuncDef => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    let mut func_def = FuncDef::new(inner_pair.line_col().0);
                    self.walk_func_def(inner_pair,&mut func_def);
                }
            }
            Rule::ConstDecl | Rule::VarDecl => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    match inner_pair.as_rule() {
                        Rule::ConstDef => {
                            let mut val =  VariableDel::new(inner_pair.line_col().0);
                            val.is_const = true;
                            val.var_type = Some("int".to_string());
                            val.belongs_to = self.current_func.clone();
                            self.walk_val_def(inner_pair,&mut val);
                            self.variable_del.push(val);
                        }
                        Rule::VarDef => {
                            let mut val =  VariableDel::new(inner_pair.line_col().0);
                            val.is_const = false;
                            val.var_type = Some("int".to_string());
                            val.belongs_to = self.current_func.clone();
                            self.walk_val_def(inner_pair,&mut val);
                            self.variable_del.push(val);
                        }
                        _=> {}
                    }
                }
            }
            _ => { /* todo 语义检查逻辑 */ }
        }
    }

    fn walk_func_def(&mut self,pair:Pair<Rule>,func_def:&mut FuncDef) {
        fn walk_func_param(pair:Pair<Rule>,param:&mut FuncParam) {
            match pair.as_rule() {
                Rule::Ident => {
                    param.name = Some(pair.as_str().to_string());
                },
                Rule::ArrayDims => {
                    let inner_pairs = pair.into_inner();
                    for p in inner_pairs  {
                        match p.as_rule() {
                            Rule::ConstExp => {
                                param.array_dims.push(p.as_str().to_string());
                            },
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        match pair.as_rule() {
            Rule::FuncType => {
                func_def.return_type = Some(pair.as_str().to_string());
            },
            Rule::Ident => {
                func_def.name = Some(pair.as_str().to_string());
            },
            Rule::FuncFParams => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.walk_func_def(inner_pair, func_def);
                }
            },
            Rule::FuncFParam => {
                
            }
            Rule::Block => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                }
            },
            _ => {}
        }
    }


    fn walk_val_def(&mut self,pair:Pair<Rule>,def:&mut VariableDel) {
        // fn single_def(&mut self,pair)
        match pair.as_rule() {
            Rule::ConstDef => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.walk_val_def(inner_pair, def);
                }
            },
            Rule::VarDef => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.walk_val_def(inner_pair, def);
                }
            },
            Rule::Ident => {
                def.name = Some(pair.as_str().to_string());
            },
            Rule::ArrayDims => {
                let inner_pairs = pair.into_inner();
                for p in inner_pairs  {
                    self.walk_array_dims(p, def);
                }
            }
            Rule::ConstInitVal | Rule::InitVal => {
                let value = pair.as_str().to_string();
                if !value.starts_with("{")
                    && self.is_var_defined(&def.name,&def.belongs_to){
                    // 判断是否存在已经定义的变量
                    def.error_kind = Some(ErrorKind::RedefineVal);
                }
                def.value = Some(pair.as_str().to_string());
            }
            _ => {}
        }
    }
    
    fn walk_array_dims(&mut self,pair:Pair<Rule>,def:&mut VariableDel) {
        match pair.as_rule() {
            Rule::ConstExp => {
                def.array_dims.push(pair.as_str().to_string());
            },
            _ => {}
        }
    }

    fn is_var_defined(&self,name:&Option<String>,belong_to: &Option<String>) -> bool {
        for var in &self.variable_del {
           if eq_option_string(name,&var.name)
               && eq_option_string(belong_to,&var.belongs_to) {
               return true;
           }
        }
        false
    }
}


#[derive(Debug)]
pub struct FuncDef {
    name: Option<String>,
    return_type: Option<String>,
    params: Vec<FuncParam>,
    line_no: usize,
    error_kind: Option<ErrorKind>,
}

impl FuncDef {
    fn new(line_no: usize) -> Self {
        FuncDef {
            name: None,
            return_type: None,
            params: Vec::new(),
            line_no,
            error_kind: None,
        }
    }
}

#[derive(Debug)]
struct FuncParam {
    name: Option<String>,
    var_type: Option<String>,
    array_dims: Vec<String>,
}


impl FuncParam {
    fn new() -> Self {
        FuncParam {
            name: None,
            var_type: None,
            array_dims: Vec::new(),
        }
    }
}

#[derive(Debug,Clone)]
pub struct VariableDel {
    name: Option<String>,
    var_type: Option<String>,
    line_no: usize,
    is_const : bool,
    array_dims: Vec<String>,
    value: Option<String>,
    belongs_to: Option<String>, // 该变量属于哪个函数
    error_kind: Option<ErrorKind>,
}

impl VariableDel {
    fn new(line_no: usize) -> Self {
        VariableDel {
            name: None,
            var_type:None,
            line_no,
            is_const:false,
            array_dims: Vec::new(),
            value: None,
            belongs_to: None,
            error_kind:None
        }
    }
   fn is_array_type(&self) -> bool {
        !self.array_dims.is_empty()
   }

    fn is_common_type(&self) -> bool {
        !self.is_array_type()
    }

    fn is_const_var(&self) -> bool {
        self.is_const
    }

    fn get_ident(&self) -> Option<String> {
       if self.is_array_type() {
           return add_option_string(self.name.clone(),Some(self.array_dims.join("")));
       }
        self.name.clone()
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

#[cfg(test)]
mod tests {
    use std::io::stdout;
    use crate::check::Checker;
    use crate::format::Formatter;
    const FILE_PATH: &str = "tests/lab3/";
    #[test]
    #[ignore]
    /// 递归测试
    /// &mut Vec<String> 的递归传递 不会丧失所有权
    /// 并且值会被正确修改
    /// 证明了 &mut T 的传递是安全的
    /// 这点和 &T 不同，&T 的传递会丧失所有权
    /// 因为 &T 是不可变引用，无法修改值
    /// 所以需要使用 &mut T 来传递可变引用
    fn test_recursive() {
        fn test( arr_s:&mut Vec<String>, count: u8) {
            if count == 8 {
                return;
            }
            arr_s.push("test".to_string());
            test(arr_s,count+1);
        }
        let mut arr_s:Vec<_> = Vec::<String>::new();
        test(&mut arr_s,0);
        println!("{:?}", arr_s);
    }

    #[test]
    #[ignore]
    fn test_vec_append() {
        let mut a = vec![1, 2, 3];
        let mut b = vec![4, 5, 6];
        a.append(&mut b);
        println!("a: {:?}, b: {:?}", a, b); // a: [1, 2, 3, 4, 5, 6], b: []
    }

    #[test]
    fn test_lab3_01(){
        let filename = FILE_PATH.to_string() + "test.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut checker = Checker::new(&file, &mut binding);
        checker.syn_check().unwrap();
        dbg!(checker.variable_del);
        // println!("{:?}",checker.variable_del.len());
    }
}
