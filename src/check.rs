//! SysY语言的语义检查器
//! 
//! 该模块实现了语义分析，包括：
//! - 变量和函数声明检查
//! - 表达式和赋值的类型检查
//! - 作用域管理和可见性规则
//! - 函数调用参数验证
//! - 数组访问验证

use std::collections::HashMap;
use crate::utils::{add_option_string, eq_option_string};
use pest::Parser;
use pest::iterators::{Pair, Pairs};
use pest_derive::Parser;
use std::borrow::Cow;
use std::fmt::Display;
use std::hash::Hash;
use std::io::{self, Write};

#[derive(Parser)]
#[grammar = "pests/check.pest"]
pub struct CParser;

/// 使用语法规则解析输入文件
/// 
/// # 参数
/// * `input` - 要解析的源代码字符串
/// 
/// # 返回值
/// * `Some(Pairs)` - 如果解析成功
/// * `None` - 如果解析失败（语法错误）
fn parse_file(input: &str) -> Option<Pairs<'_, Rule>> {
    match CParser::parse(Rule::File, input) {
        Ok(pairs) => Some(pairs),
        Err(_) => None,
    }
}

/// SysY语言的语义检查器
/// 
/// 该结构体在语义分析期间维护状态，包括：
/// - 变量和函数的符号表
/// - 通过上下文堆栈进行作用域管理
/// - 错误收集和报告
#[derive(Debug)]
pub struct Checker<'a, W: Write> {
    /// 输入源代码
    input: &'a str,
    /// 错误消息的输出写入器
    writer: &'a mut W,
    /// 累积的输出字符串
    output: String,
    /// 作用域栈
    scope_stack: ScopeStack
}

impl<'a, W: Write> Checker<'a, W> {
    /// 创建新的语义检查器
    ///
    /// # 参数
    /// * `input` - 要分析的源代码
    /// * `writer` - 错误消息的输出写入器
    pub fn new(input: &'a str, writer: &'a mut W) -> Self {
        Checker {
            input,
            writer,
            output: String::new(),
            scope_stack: Default::default(),
        }
    }
    pub fn syn_check(&mut self) -> io::Result<()> {
        if let Some(mut pairs) = parse_file(self.input) {
            // 处理文件头
            let file_pair = pairs.next().unwrap();
            // 读取编译单元
            let compilation_unit = file_pair.into_inner().next().unwrap();
            // 获取所有的声明
            let declarations = compilation_unit.into_inner();

            dbg!(declarations.clone());
            // 依次解析声明
            for declaration in declarations {
                // todo 声明解析
                // self.analyze_declaration(declaration);
            }

            // todo 生成报错信息，并输出
            // self.generate_error_output()?;
        } else {
            writeln!(self.writer, "Syntax error")?;
        }
        Ok(())
    }

    pub fn analyze_declaration(&mut self, declaration: Pair<'_, Rule>) {
        match declaration.as_rule() {
            Rule::Decl => {
                let decls = declaration.into_inner();
                for decl in decls {
                    self.analyze_declaration(decl);
                }
            }
            Rule::ConstDecl => {},
            Rule::VarDecl => {
                self.analyze_var_decl(declaration);
            },
            Rule::FuncDef => {},
            _ => {}
        }
    }
    /// 获取目标的规则的迭代器
    fn skip_in<'b>(&mut self, pair: Pair<'b, Rule>) -> impl Iterator<Item = Pair<'b, Rule>>  {
        pair.into_inner().into_iter()
    }

    /// 语义检查常量声明
    pub fn analyze_const_decl(&mut self, const_decl: Pair<'_, Rule>) {

    }

    /// 语义检查常量定义
    pub fn analyze_const_def(&mut self, const_def: Pair<'_, Rule>) {

    }


    /// 语义检查变量声明
    pub fn analyze_var_decl(&mut self, var_decl: Pair<'_, Rule>) {
        let mut decl_iter = self.skip_in(var_decl);
        let mut var_defs = Vec::<Pair<'_, Rule>>::new();
        while let Some(var_def) = decl_iter.next(){
            if var_def.as_rule() == Rule::VarDef {
                var_defs.push(var_def);
            }
        }
    }

    /// 语义检查变量定义
    pub fn analyze_var_def(&mut self, var_def: Pair<'_, Rule>) {
        let mut def_iter = self.skip_in(var_def);
        let ident = def_iter.next().unwrap();
        let next_pair = def_iter.next();
        let mut scope = self.scope_stack.get_current_scope_mut();
        if next_pair.is_some() && next_pair.unwrap().as_rule()==Rule::ArrayDims {
            //todo 数组类型的变量
        }
    }


}








#[derive(Debug,Clone,Eq, Hash, PartialEq)]
pub enum ScopeKey {
    Global,
    Ident(String),
}

/// 作用域栈
#[derive(Debug)]
pub struct ScopeStack {
    /// 作用域栈
    stack: Vec<ScopeKey>,
    /// 当前作用域的符号表
    scopes: HashMap<ScopeKey, Scope>,
}

impl ScopeStack {

    /// 压入作用域
    pub fn push(&mut self, key: ScopeKey) {
        self.stack.push(key.clone());
        self.scopes.insert(key, Default::default());
    }

    /// 弹出作用域
    pub fn pop(&mut self) {
        let key = self.stack.pop().unwrap();
        self.scopes.remove(&key);
    }

    /// 获取作用域
    fn get_scope(&self, key: &ScopeKey) -> Option<&Scope> {
        self.scopes.get(key)
    }

    /// 获取可变的作用域
    fn get_scope_mut(&mut self, key: &ScopeKey) -> Option<&mut Scope> {
        self.scopes.get_mut(key)
    }

    /// 获取当前作用域的符号表
    pub fn get_current_scope(&self) -> Option<&Scope> {
        let last_key = self.stack.last().unwrap();
        self.get_scope(last_key)
    }
    
    /// 获取可变的当前作用域的符号表
    pub fn get_current_scope_mut(&mut self) -> Option<&mut Scope> {
        let last_key = self.stack.last().unwrap();
        self.get_scope_mut(last_key)
    }
}

/// 设定一个初始化的作用域栈
impl Default for ScopeStack {
    fn default() -> Self {
        let mut stack = Vec::<ScopeKey>::new();
        stack.push(ScopeKey::Global);
        Self {
            stack,
            scopes: Default::default(),
        }
    }
}

/// 作用域
#[derive(Debug)]
pub struct Scope {
    /// 当前作用域的符号表
    pub symbol_table: HashMap<String, Type>,
}

impl Default for Scope {
    fn default() -> Self {
        Scope {
            symbol_table: HashMap::new(),
        }
    }
}


/// 枚举类型
#[derive(Debug,Clone)]
pub enum Type {
    Int,
    Func(Func),
    Void,
    Array(Array)
}

/// 函数类型表示
#[derive(Debug,Clone)]
pub struct Func {
    // 函数入参类型
    pub params: Vec<Type>,
    // 函数返回类型
    pub return_type: Box<Type>,
}

/// 数组结果表示
/// 维度和类型
#[derive(Debug,Clone)]
pub struct Array {
    // 数组元素类型
    pub item_type: Box<Type>,
    // 数组维度和长度
    pub dim_size: usize,
    // 每个维度的存储空间
    pub tpl_size: HashMap<usize, usize>,
}

/// 表示在分析过程中发现的语义错误
#[derive(Debug)]
struct SemanticError<'a> {
    /// 发生错误的行号
    line_no: String,
    /// 具体的错误类型和详细信息
    error_type: Option<CheckError>,
    /// 格式化的错误消息输出
    output: Cow<'a, str>,
}

impl<'a> SemanticError<'a> {
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

/// 实验要求中定义的语义错误类型
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum ErrorKind {
    /// 错误类型1：未定义的变量
    UndefinedVal = 1,
    /// 错误类型2：未定义的函数
    UndefinedFunc = 2,
    /// 错误类型3：重复定义的变量
    RedefineVal = 3,
    /// 错误类型4：重复定义的函数
    RedefineFunc = 4,
    /// 错误类型5：赋值中的类型不匹配
    TypeMismatch = 5,
    /// 错误类型6：操作符的类型不匹配
    TypeMismatchOp = 6,
    /// 错误类型7：返回类型不匹配
    ReturnMismatch = 7,
    /// 错误类型8：函数不适用于参数
    Inappropriate = 8,
    /// 错误类型9：不是数组
    NotArrayAssign = 9,
    /// 错误类型10：不是函数
    UnlegalFuncCall = 10,
    /// 错误类型11：赋值的左侧必须是变量
    UnexpectedFuncAssign = 11,
    /// 错误类型0：其他错误
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
                CheckError::UndefinedVal(kind, "未定义的变量", error_tip)
            }
            ErrorKind::UndefinedFunc => {
                CheckError::UndefinedFunc(kind, "未定义的函数", error_tip)
            }
            ErrorKind::RedefineVal => {
                CheckError::RedefineVal(kind, "重复定义的变量", error_tip)
            }
            ErrorKind::RedefineFunc => {
                CheckError::RedefineFunc(kind, "重复定义的函数", error_tip)
            }
            ErrorKind::TypeMismatch => {
                CheckError::TypeMismatch(kind, "赋值的类型不匹配", error_tip)
            }
            ErrorKind::TypeMismatchOp => {
                CheckError::TypeMismatchOp(kind, "操作符的类型不匹配", error_tip)
            }
            ErrorKind::ReturnMismatch => {
                CheckError::ReturnMismatch(kind, "返回的类型不匹配", error_tip)
            }
            ErrorKind::Inappropriate => CheckError::Inappropriate(
                kind,
                "函数不适用于参数",
                error_tip,
            ),
            ErrorKind::NotArrayAssign => {
                CheckError::NotArrayAssign(kind, "不是数组", error_tip)
            }
            ErrorKind::UnlegalFuncCall => {
                CheckError::UnlegalFuncCall(kind, "不是函数", error_tip)
            }
            ErrorKind::UnexpectedFuncAssign => CheckError::UnexpectedFuncAssign(
                kind,
                "赋值的左侧必须是变量。",
                error_tip,
            ),
            _ => CheckError::Other(kind, "其他错误", error_tip),
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
    use crate::check::{parse_file, Checker, Rule};
    use std::io::stdout;
    use pest::iterators::Pair;

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

    // #[test]
    // fn test_lab3_test() {
    //     let filename = FILE_PATH.to_string() + "test.sy";
    //     let file = std::fs::read_to_string(filename).expect("Failed to read file");
    //     let mut binding = stdout();
    //     let mut checker = Checker::new(&file, &mut binding);
    //     // checker.syn_check().unwrap();
    // }
    // 
    // #[test]
    // fn test_lab3_example01() {
    //     // 1、把内容输出内存缓冲区
    //     let mut buf = Vec::<u8>::new();
    //     let filename = FILE_PATH.to_string() + "example01.sy";
    //     let file = std::fs::read_to_string(filename).expect("Failed to read file");
    //     let mut checker = Checker::new(&file, &mut buf);
    //     checker.syn_check().unwrap();
    //     let mut actual = String::from_utf8(buf).unwrap();
    //     // 根据操作系统替换换行符
    //     // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
    //     actual = actual.replace('\n', "\r\n");
    //     let expected_filename = FILE_PATH.to_string() + "example01.out";
    //     let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
    //     assert_eq!(actual, expected);
    // }
    // 
    // #[test]
    // fn test_lab3_normaltest01() {
    //     // 1、把内容输出内存缓冲区
    //     let mut buf = Vec::<u8>::new();
    //     let filename = FILE_PATH.to_string() + "normaltest01.sy";
    //     let file = std::fs::read_to_string(filename).expect("Failed to read file");
    //     let mut checker = Checker::new(&file, &mut buf);
    //     checker.syn_check().unwrap();
    //     let mut actual = String::from_utf8(buf).unwrap();
    //     // 根据操作系统替换换行符
    //     // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
    //     actual = actual.replace('\n', "\r\n");
    //     let expected_filename = FILE_PATH.to_string() + "normaltest01.out";
    //     let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
    //     assert_eq!(actual, expected);
    // }
    // 
    // #[test]
    // fn test_lab3_normaltest02() {
    //     // 1、把内容输出内存缓冲区
    //     let mut buf = Vec::<u8>::new();
    //     let filename = FILE_PATH.to_string() + "normaltest02.sy";
    //     let file = std::fs::read_to_string(filename).expect("Failed to read file");
    //     let mut checker = Checker::new(&file, &mut buf);
    //     checker.syn_check().unwrap();
    //     let mut actual = String::from_utf8(buf).unwrap();
    //     // 根据操作系统替换换行符
    //     // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
    //     actual = actual.replace('\n', "\r\n");
    //     let expected_filename = FILE_PATH.to_string() + "normaltest02.out";
    //     let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
    //     assert_eq!(actual, expected);
    // }
    // 
    // #[test]
    // fn test_lab3_normaltest03() {
    //     // 1、把内容输出内存缓冲区
    //     let mut buf = Vec::<u8>::new();
    //     let filename = FILE_PATH.to_string() + "normaltest03.sy";
    //     let file = std::fs::read_to_string(filename).expect("Failed to read file");
    //     let mut checker = Checker::new(&file, &mut buf);
    //     checker.syn_check().unwrap();
    //     let mut actual = String::from_utf8(buf).unwrap();
    //     // 根据操作系统替换换行符
    //     // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
    //     actual = actual.replace('\n', "\r\n");
    //     let expected_filename = FILE_PATH.to_string() + "normaltest03.out";
    //     let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
    //     assert_eq!(actual, expected);
    // }
    // 
    // #[test]
    // fn test_lab3_normaltest04() {
    //     // 1、把内容输出内存缓冲区
    //     let mut buf = Vec::<u8>::new();
    //     let filename = FILE_PATH.to_string() + "normaltest04.sy";
    //     let file = std::fs::read_to_string(filename).expect("Failed to read file");
    //     let mut checker = Checker::new(&file, &mut buf);
    //     checker.syn_check().unwrap();
    //     let mut actual = String::from_utf8(buf).unwrap();
    //     // 根据操作系统替换换行符
    //     // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
    //     actual = actual.replace('\n', "\r\n");
    //     let expected_filename = FILE_PATH.to_string() + "normaltest04.out";
    //     let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
    //     assert_eq!(actual, expected);
    // }
    // 
    // #[test]
    // fn test_lab3_normaltest05() {
    //     // 1、把内容输出内存缓冲区
    //     let mut buf = Vec::<u8>::new();
    //     let filename = FILE_PATH.to_string() + "normaltest05.sy";
    //     let file = std::fs::read_to_string(filename).expect("Failed to read file");
    //     let mut checker = Checker::new(&file, &mut buf);
    //     checker.syn_check().unwrap();
    //     let mut actual = String::from_utf8(buf).unwrap();
    //     // 根据操作系统替换换行符
    //     // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
    //     actual = actual.replace('\n', "\r\n");
    //     let expected_filename = FILE_PATH.to_string() + "normaltest05.out";
    //     let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
    //     assert_eq!(actual, expected);
    // }
    // 
    // #[test]
    // fn test_lab3_normaltest06() {
    //     // 1、把内容输出内存缓冲区
    //     let mut buf = Vec::<u8>::new();
    //     let filename = FILE_PATH.to_string() + "normaltest06.sy";
    //     let file = std::fs::read_to_string(filename).expect("Failed to read file");
    //     let mut checker = Checker::new(&file, &mut buf);
    //     checker.syn_check().unwrap();
    //     let mut actual = String::from_utf8(buf).unwrap();
    //     // 根据操作系统替换换行符
    //     // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
    //     actual = actual.replace('\n', "\r\n");
    //     let expected_filename = FILE_PATH.to_string() + "normaltest06.out";
    //     let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
    //     assert_eq!(actual, expected);
    // }
    // 
    // #[test]
    // fn test_lab3_normaltest07() {
    //     // 1、把内容输出内存缓冲区
    //     let mut buf = Vec::<u8>::new();
    //     let filename = FILE_PATH.to_string() + "normaltest07.sy";
    //     let file = std::fs::read_to_string(filename).expect("Failed to read file");
    //     let mut checker = Checker::new(&file, &mut buf);
    //     checker.syn_check().unwrap();
    //     let mut actual = String::from_utf8(buf).unwrap();
    //     // 根据操作系统替换换行符
    //     // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
    //     actual = actual.replace('\n', "\r\n");
    //     let expected_filename = FILE_PATH.to_string() + "normaltest07.out";
    //     let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
    //     assert_eq!(actual, expected);
    // }
    // 
    // #[test]
    // fn test_lab3_normaltest08() {
    //     // 1、把内容输出内存缓冲区
    //     let mut buf = Vec::<u8>::new();
    //     let filename = FILE_PATH.to_string() + "normaltest08.sy";
    //     let file = std::fs::read_to_string(filename).expect("Failed to read file");
    //     let mut checker = Checker::new(&file, &mut buf);
    //     checker.syn_check().unwrap();
    //     let mut actual = String::from_utf8(buf).unwrap();
    //     // 根据操作系统替换换行符
    //     // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
    //     actual = actual.replace('\n', "\r\n");
    //     let expected_filename = FILE_PATH.to_string() + "normaltest08.out";
    //     let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
    //     assert_eq!(actual, expected);
    // }
    // 
    // #[test]
    // fn test_lab3_normaltest09() {
    //     // 1、把内容输出内存缓冲区
    //     let mut buf = Vec::<u8>::new();
    //     let filename = FILE_PATH.to_string() + "normaltest09.sy";
    //     let file = std::fs::read_to_string(filename).expect("Failed to read file");
    //     let mut checker = Checker::new(&file, &mut buf);
    //     checker.syn_check().unwrap();
    //     let mut actual = String::from_utf8(buf).unwrap();
    //     // 根据操作系统替换换行符
    //     // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
    //     actual = actual.replace('\n', "\r\n");
    //     let expected_filename = FILE_PATH.to_string() + "normaltest09.out";
    //     let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
    //     assert_eq!(actual, expected);
    // }
    // 
    // #[test]
    // fn test_lab3_normaltest11() {
    //     // 1、把内容输出内存缓冲区
    //     let mut buf = Vec::<u8>::new();
    //     let filename = FILE_PATH.to_string() + "normaltest11.sy";
    //     let file = std::fs::read_to_string(filename).expect("Failed to read file");
    //     let mut checker = Checker::new(&file, &mut buf);
    //     checker.syn_check().unwrap();
    //     let mut actual = String::from_utf8(buf).unwrap();
    //     // 根据操作系统替换换行符
    //     // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
    //     actual = actual.replace('\n', "\r\n");
    //     let expected_filename = FILE_PATH.to_string() + "normaltest11.out";
    //     let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
    //     assert_eq!(actual, expected);
    // }


    #[test]
    #[ignore]
    fn test_number() {
        assert_eq!("0x10".parse::<i32>().is_ok(), true)
    }


    #[test]
    #[ignore]
    fn test_skip() {

        fn skip_in(pair: Pair<Rule>) -> impl Iterator<Item = Pair<Rule>>  {
            pair.into_inner().into_iter()
        }
        let filename = FILE_PATH.to_string() + "test.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        if let Some(mut pairs) = parse_file(file.as_str()) {
            // 处理文件头
            let file_pair = pairs.next().unwrap();
            // 读取编译单元
            let compilation_unit = file_pair.into_inner().next().unwrap();
            // 获取所有的声明
            let declarations = compilation_unit.into_inner();

            // 依次解析声明
            for declaration in declarations {
                if declaration.as_rule() == Rule::Decl {
                    let var = declaration.into_inner().next().unwrap();
                    if var.as_rule() == Rule::VarDecl {
                       let mut var_decl = skip_in(var).skip(1);
                       let var_def = var_decl.next();
                        println!("{}", var_def.unwrap().as_str());
                    }
                }
            }
        }
    }
}