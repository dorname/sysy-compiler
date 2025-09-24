use crate::utils::add_option_string;
use crate::utils::eq_option_string;
use pest::Parser;
use pest::iterators::Pair;
use pest::iterators::Pairs;
use pest_derive::Parser;
use std::borrow::Cow;
use std::fmt::Display;
use std::io;
use std::io::Write;
use std::ops::Add;
use std::process::id;
use clap::builder::Str;
use pest::pratt_parser::Op;

#[derive(Parser)]
#[grammar = "pests/check.pest"]
pub struct CParser;
fn parse_file(input: &'_ str) -> Option<Pairs<'_, Rule>> {
    match CParser::parse(Rule::File, input) {
        Ok(pairs) => Some(pairs),
        Err(_) => None,
    }
}

#[derive(Debug)]
pub struct Checker<'a, W: Write> {
    input: &'a str,
    writer: &'a mut W,
    output: String,
    variable_dels: Vec<VariableDel>,
    func_dels: Vec<FuncDef>,
    context_stack: Vec<(String, Option<Vec<FuncParam>>)>, // 作用域级别
    // 作用域
    block_no: usize,
    check_results: Vec<PairCheckResult<'a>>,
    // 当前函数调用
    call_func: Option<String>,
}

impl<'a, W: Write> Checker<'a, W> {
    pub fn new(input: &'a str, writer: &'a mut W) -> Self {
        let mut context_stack = Vec::new();
        context_stack.push(("global".to_string(), None));
        Checker {
            input,
            writer,
            output: String::new(),
            variable_dels: Vec::new(),
            func_dels: Vec::new(),
            context_stack,
            check_results: Vec::new(),
            block_no: 0,
            call_func: None,
        }
    }
    pub fn syn_check(&mut self) -> io::Result<()> {
        if let Some(mut pairs) = parse_file(self.input) {
            // 把File规则的第一个pair拿出来
            let pairs = pairs.next().unwrap();
            // 继续把File规则的内容拿出来
            let pairs = pairs.into_inner().next().unwrap();
            // 把编译单元的内容拿出来
            let pairs = pairs.into_inner();
            // dbg!(&pairs);
            for pair in pairs {
                self.check(pair);
            }
            // 构建检查结果
            for err in &mut self.check_results {
                let _ = err.build_str();
                let out_str = err.output.to_string() + "\n";
                self.output.push_str(&out_str);
            }
            if self.check_results.is_empty() {
                writeln!(self.writer, "{}", "No semantic errors in the program!")?;
            }else {
                write!(self.writer, "{}", self.output)?;
            }
        } else {
            writeln!(self.writer, "Syntax error")?;
        }
        Ok(())
    }

    fn check(&mut self, pair: Pair<Rule>) {
        match pair.as_rule() {
            Rule::Decl => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.check(inner_pair);
                }
            }
            Rule::FuncDef => {
                let line_no = pair.line_col().0;
                let inner_pairs = pair.into_inner();
                let mut func_def = FuncDef::new(line_no);
                let mut should_skip = false;
                
                for inner_pair in inner_pairs {
                    // 收集函数信息
                    // 1、收集函数类型
                    // 2、收集函数名称
                    // 2.1、校验函数是否重复定义
                    //      |——>是，生成错误信息，跳出整个函数的识别循环
                    //      |——>否，继续扫描
                    // 3、收集参数
                    // 3.1、校验参数是否重复定义
                    //      |——>是，生成错误信息
                    //      |——>否，继续扫描
                    // 4、函数加入当前上下文的栈，方便后续扫描的语句处理
                    // 5、处理函数块中的所有语句
                    // 6、最后

                    //
                    // 如果函数重复定义，跳过整个函数
                    if should_skip {
                        continue;
                    }

                    // 检查函数名是否重复定义
                    if inner_pair.as_rule() == Rule::Ident {
                        let name = inner_pair.as_str().to_string();
                        if self.func_contains(&name) {
                            // 函数重复定义，报错并标记跳过
                            let check_result = PairCheckResult::new(
                                inner_pair.line_col().0.to_string(),
                                Some(CheckError::new(ErrorKind::RedefineFunc, Some(name.clone()))),
                            );
                            self.check_results.push(check_result);
                            should_skip = true;
                            continue;
                        }
                        func_def.name = Some(name.clone());
                        self.context_stack.push((name, None));
                    } else {
                        self.walk_func_def(inner_pair, &mut func_def);
                    }
                }
                
                // 只有在没有错误的情况下才添加函数定义
                if !should_skip {
                    self.func_dels.push(func_def);
                }
                
                // 清理作用域状态
                if self.block_no > 0 {
                    self.block_no -= 1; // 离开函数作用域
                }
                if self.context_stack.len() > 1 {
                    self.context_stack.pop(); // 函数出栈
                }
                self.call_func = None; // 清除函数调用记录
            }
            Rule::ConstDecl | Rule::VarDecl => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    match inner_pair.as_rule() {
                        Rule::ConstDef => {

                        }
                        Rule::VarDef => {

                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn walk_func_def(&mut self, pair: Pair<Rule>, func_def: &mut FuncDef) {
        match pair.as_rule() {
            Rule::FuncType => {
                func_def.return_type = Some(pair.as_str().to_string()); //收集返回类型
            }

            Rule::ReturnStmt => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.walk_return_stmt(inner_pair, func_def);
                }
            }
            _ => {}
        }
    }

    fn has_operator(&self,pair:Pair<Rule>) -> bool {
        match pair.as_rule() {
            Rule::Plus |
            Rule::Minus |
            Rule::Mul |
            Rule::Div |
            Rule::Mod |
            Rule::Not |
            Rule::And |
            Rule::Or |
            Rule::Less |
            Rule::Greater |
            Rule::LessEqual |
            Rule::GreaterEqual |
            Rule::Equal |
            Rule::NotEqual => true,
            _ => {
                let inner_pairs = pair.into_inner();
                let mut result = false;
                for inner_pair in inner_pairs {
                    result |= self.has_operator(inner_pair);
                }
                result
            }
        }
    }


    fn walk_return_stmt(&mut self, pair: Pair<Rule>,func_def: &mut FuncDef) {
        match pair.as_rule() {
            Rule::Ident => {
                let line_no = pair.line_col().0;
                let pair_str = pair.as_str();
                let func_type = func_def.return_type.clone();
                let (v_defined,v) = self.var_contains(&pair_str);
                let f_defined = self.func_contains(&pair_str);
                if let Some(func_type) = func_type {
                    if f_defined || v {
                        self.add_check_result(line_no, ErrorKind::ReturnMismatch,Some(pair_str.to_string()));
                        return;
                    }
                    if v_defined && !v {
                        if func_type == "void".to_string() {
                            self.add_check_result(line_no, ErrorKind::ReturnMismatch,Some(pair_str.to_string()));
                        }
                    }
                }
            }
            Rule::LVal => {
                let pair_str = pair.as_str();
                if pair_str.ends_with("]") {
                    let ident = pair.into_inner().next().unwrap();
                    self.walk_return_stmt(ident, func_def);
                }else {
                    for inner_pair in pair.into_inner() {
                        self.walk_return_stmt(inner_pair, func_def);
                    }
                }
            }
            Rule::Number => {
                let line_no = pair.line_col().0;
                let pair_str = pair.as_str();
                let func_type = func_def.return_type.clone();
                if let Some(func_type) = func_type {
                    if func_type == "void".to_string() {
                        self.add_check_result(line_no, ErrorKind::ReturnMismatch,Some(pair_str.to_string()));
                    }
                }

            }
            _ => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.walk_return_stmt(inner_pair,func_def);
                }
            }
        }
    }



    fn check_val_redefine(&mut self, ident: &str, line_no: usize) {
        let (var_defined, _) = self.var_contains(&ident);
        let func_defined = self.func_contains(&ident);
        
        // 检查是否与函数重名
        if func_defined {
            self.add_check_result(line_no, ErrorKind::RedefineVal, Some(ident.to_string()));
            return;
        }
        
        // 检查是否在同一作用域内重复定义变量
        if var_defined {
            // 查找同名变量在当前作用域的定义
            let same_scope_var = self.variable_dels.iter().find(|e| {
                eq_option_string(&e.name, &Some(ident.to_string())) && e.belongs_to == self.block_no
            });
            
            if same_scope_var.is_some() {
                self.add_check_result(line_no, ErrorKind::RedefineVal, Some(ident.to_string()));
            }
        }
        
        // 检查函数参数重复定义
        if let Some((_, Some(params))) = self.get_current_context() {
            let param_count = params.iter()
                .filter(|p| eq_option_string(&p.name, &Some(ident.to_string())))
                .count();
            if param_count > 0 {
                self.add_check_result(line_no, ErrorKind::RedefineVal, Some(ident.to_string()));
            }
        }
    }

    fn func_contains(&self,ident: &str) -> bool {
        self.func_dels.iter().find(|e| eq_option_string(&e.name,&Some(ident.to_string()))).is_some()
    }

    fn var_contains(&self, ident: &str) -> (bool, bool) {
        // 先校验函数入参
        if let Some((_, Some(params))) = self.get_current_context() {
            let result = params
                .iter()
                .filter(|p| eq_option_string(&p.name, &Some(ident.to_string())))
                .map(|p| (true, p.is_array_type()))
                .collect::<Vec<_>>();
            if result.len() != 0 {
                return *result.first().unwrap();
            }
        }

        let vars = self
            .variable_dels
            .iter()
            .filter(|v| {
                if let Some(name) = &v.name
                    && v.belongs_to <= self.block_no //需要标准清楚 可见性问题 【变量所属作用域级别 小于 当前作用域】即不可见
                {
                    // 变量已经定义
                    return name == ident;
                } else {
                    false
                }
            })
            .collect::<Vec<_>>();
        if vars.len() == 0 {
            return (false, false);
        }
        // 求vars self.current_func与每个var的belongsto 差值的最小值
        let result = vars
            .iter()
            .map(|&v| (self.block_no - v.belongs_to, Some(v)))
            .min_by(|(x, _), (y, _)| x.cmp(y));

        if let Some((_, v)) = result {
            (true, v.unwrap().is_array_type())
        } else {
            (false, false)
        }
    }
    fn get_current_context(&self) -> Option<&(String, Option<Vec<FuncParam>>)> {
        self.context_stack.last()
    }

    fn add_check_result(&mut self, line_no: usize, error_kind: ErrorKind,tip: Option<String>){
        let check_result = PairCheckResult::new(
            line_no.to_string(),
            Some(CheckError::new(error_kind,tip)),
        );
        self.check_results.push(check_result);
    }
}
/// 函数定义的作用域级数是1因为函数无法嵌套但是函数内容可以使用{}块嵌套作用域
/// 进入block 层级+1
#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

    fn is_array_type(&self) -> bool {
        !self.array_dims.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct VariableDel {
    name: Option<String>,
    var_type: Option<String>,
    line_no: usize,
    is_const: bool,
    array_dims: Vec<String>,
    value: Option<String>,
    belongs_to: usize, // 该变量属于哪个函数
    is_error: bool,
}

impl VariableDel {
    fn new(line_no: usize) -> Self {
        VariableDel {
            name: None,
            var_type: None,
            line_no,
            is_const: false,
            array_dims: Vec::new(),
            value: None,
            belongs_to: 0,
            is_error: false,
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
            return add_option_string(self.name.clone(), Some(self.array_dims.join("")));
        }
        self.name.clone()
    }
}

#[derive(Debug)]
struct PairCheckResult<'a> {
    line_no: String,
    error_type: Option<CheckError>,
    output: Cow<'a, str>,
}

impl<'a> PairCheckResult<'a> {
    fn new(line_no: String, error_type: Option<CheckError>) -> Self {
        Self {
            line_no,
            error_type,
            output: Cow::Borrowed(""),
        }
    }
    fn build_str(&mut self) {
        self.output = if let Some(e) = &self.error_type {
            Cow::Owned(format!(
                "Error type {} at Line {}: {}",
                e.get_kind(),
                self.line_no,
                e.build_str()
            ))
        } else {
            Cow::Borrowed("")
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum ErrorKind {
    UndefinedVal = 1,
    UndefinedFunc = 2,
    RedefineVal = 3,
    RedefineFunc = 4,
    TypeMismatch = 5,
    TypeMismatchOp = 6,
    ReturnMismatch = 7,
    Inappropriate = 8,
    NotArrayAssign = 9,
    UnlegalFuncCall = 10,
    UnexpectedFuncAssign = 11,
    Other = 0,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", *self as u8)
    }
}

#[derive(Debug)]
pub enum CheckError {
    UndefinedVal(ErrorKind, &'static str, Option<String>),
    UndefinedFunc(ErrorKind, &'static str, Option<String>),
    RedefineVal(ErrorKind, &'static str, Option<String>),
    RedefineFunc(ErrorKind, &'static str, Option<String>),
    TypeMismatch(ErrorKind, &'static str, Option<String>),
    TypeMismatchOp(ErrorKind, &'static str, Option<String>),
    ReturnMismatch(ErrorKind, &'static str, Option<String>),
    Inappropriate(ErrorKind, &'static str, Option<String>),
    NotArrayAssign(ErrorKind, &'static str, Option<String>),
    UnlegalFuncCall(ErrorKind, &'static str, Option<String>),
    UnexpectedFuncAssign(ErrorKind, &'static str, Option<String>),
    Other(ErrorKind, &'static str, Option<String>),
}

impl CheckError {
    fn new(kind: ErrorKind, error_tip: Option<String>) -> Self {
        match kind {
            ErrorKind::UndefinedVal => {
                CheckError::UndefinedVal(kind, "Undefined variable", error_tip)
            }
            ErrorKind::UndefinedFunc => {
                CheckError::UndefinedFunc(kind, "Undefined function", error_tip)
            }
            ErrorKind::RedefineVal => {
                CheckError::RedefineVal(kind, "Redefined variable", error_tip)
            }
            ErrorKind::RedefineFunc => {
                CheckError::RedefineFunc(kind, "Redefined function", error_tip)
            }
            ErrorKind::TypeMismatch => CheckError::TypeMismatch(kind, "Type mismatched for assignment", error_tip),
            ErrorKind::TypeMismatchOp => {
                CheckError::TypeMismatchOp(kind, "Type mismatched for op", error_tip)
            }
            ErrorKind::ReturnMismatch => {
                CheckError::ReturnMismatch(kind, "Type mismatched for return", error_tip)
            }
            ErrorKind::Inappropriate => {
                CheckError::Inappropriate(kind, "Function is not applicable for arguments", error_tip)
            }
            ErrorKind::NotArrayAssign => {
                CheckError::NotArrayAssign(kind, "Not an array", error_tip)
            }
            ErrorKind::UnlegalFuncCall => {
                CheckError::UnlegalFuncCall(kind, "Not a function", error_tip)
            }
            ErrorKind::UnexpectedFuncAssign => {
                CheckError::UnexpectedFuncAssign(kind, "The left-hand side of an assignment must be a variable.", error_tip)
            }
            _ => CheckError::Other(kind, "Other error", error_tip),
        }
    }

    fn get_kind(&self) -> ErrorKind {
        match self {
            CheckError::UndefinedVal(kind, _, _)
            | CheckError::UndefinedFunc(kind, _, _)
            | CheckError::RedefineVal(kind, _, _)
            | CheckError::RedefineFunc(kind, _, _)
            | CheckError::TypeMismatch(kind, _, _)
            | CheckError::TypeMismatchOp(kind, _, _)
            | CheckError::ReturnMismatch(kind, _, _)
            | CheckError::Inappropriate(kind, _, _)
            | CheckError::NotArrayAssign(kind, _, _)
            | CheckError::UnlegalFuncCall(kind, _, _)
            | CheckError::UnexpectedFuncAssign(kind, _, _)
            | CheckError::Other(kind, _, _) => *kind,
        }
    }

    fn get_tip(&self) -> Option<String> {
        match self {
            CheckError::UndefinedVal(_, _, tip)
            | CheckError::UndefinedFunc(_, _, tip)
            | CheckError::RedefineVal(_, _, tip)
            | CheckError::RedefineFunc(_, _, tip)
            | CheckError::TypeMismatch(_, _, tip)
            | CheckError::TypeMismatchOp(_, _, tip)
            | CheckError::ReturnMismatch(_, _, tip)
            | CheckError::Inappropriate(_, _, tip)
            | CheckError::NotArrayAssign(_, _, tip)
            | CheckError::UnlegalFuncCall(_, _, tip)
            | CheckError::UnexpectedFuncAssign(_, _, tip)
            | CheckError::Other(_, _, tip) => tip.clone(),
        }
    }

    fn get_msg(&self) -> &'static str {
        match self {
            CheckError::UndefinedVal(_, msg, _)
            | CheckError::UndefinedFunc(_, msg, _)
            | CheckError::RedefineVal(_, msg, _)
            | CheckError::RedefineFunc(_, msg, _)
            | CheckError::TypeMismatch(_, msg, _)
            | CheckError::TypeMismatchOp(_, msg, _)
            | CheckError::ReturnMismatch(_, msg, _)
            | CheckError::Inappropriate(_, msg, _)
            | CheckError::NotArrayAssign(_, msg, _)
            | CheckError::UnlegalFuncCall(_, msg, _)
            | CheckError::UnexpectedFuncAssign(_, msg, _)
            | CheckError::Other(_, msg, _) => msg,
        }
    }

    fn build_str(&self) -> String {
        let tip = self.get_tip();
        if let Some(t) = tip {
            format!("{}:{}", self.get_msg(), t)
        } else {
            format!("{}", self.get_msg())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::check::Checker;
    use std::io::stdout;

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
        fn test(arr_s: &mut Vec<String>, count: u8) {
            if count == 8 {
                return;
            }
            arr_s.push("test".to_string());
            test(arr_s, count + 1);
        }
        let mut arr_s: Vec<_> = Vec::<String>::new();
        test(&mut arr_s, 0);
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
    fn test_lab3_test() {
        let filename = FILE_PATH.to_string() + "test.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut checker = Checker::new(&file, &mut binding);
        checker.syn_check().unwrap();
    }

    #[test]
    fn test_lab3_example01() {
        let filename = FILE_PATH.to_string() + "example01.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut checker = Checker::new(&file, &mut binding);
        checker.syn_check().unwrap();
    }

    #[test]
    fn test_lab3_normaltest01() {
        let filename = FILE_PATH.to_string() + "normaltest01.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut checker = Checker::new(&file, &mut binding);
        checker.syn_check().unwrap();
    }

    #[test]
    fn test_lab3_normaltest02() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "normaltest02.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut checker = Checker::new(&file, &mut buf);
        checker.syn_check().unwrap();
        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace('\n', "\r\n");
        let expected_filename = FILE_PATH.to_string() + "normaltest02.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_lab3_normaltest03() {
        let filename = FILE_PATH.to_string() + "normaltest03.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut checker = Checker::new(&file, &mut binding);
        checker.syn_check().unwrap();
    }


    #[test]
    fn test_lab3_normaltest04() {
        let filename = FILE_PATH.to_string() + "normaltest04.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut checker = Checker::new(&file, &mut binding);
        checker.syn_check().unwrap();
    }


    #[test]
    fn test_lab3_normaltest05() {
        let filename = FILE_PATH.to_string() + "normaltest05.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut checker = Checker::new(&file, &mut binding);
        checker.syn_check().unwrap();
    }

    #[test]
    fn test_lab3_normaltest06() {
        let filename = FILE_PATH.to_string() + "normaltest06.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut checker = Checker::new(&file, &mut binding);
        checker.syn_check().unwrap();
    }

    #[test]
    fn test_lab3_normaltest07() {
        let filename = FILE_PATH.to_string() + "normaltest07.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut checker = Checker::new(&file, &mut binding);
        checker.syn_check().unwrap();
    }
}