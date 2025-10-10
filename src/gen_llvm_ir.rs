//! SysY语言的LLVM-IR生成器
//! 前置条件：
//! 默认输入的文件是正确的输入
//! 文件扫描包括：
//! 1、收集变量和函数的符号
//! 2、作用域栈管理
//! 3、符号表管理
//! 4、LLVM-IR生成

use crate::utils::{add_option_string, eq_option_string};
use pest::Parser;
use pest::iterators::{Pair, Pairs};
use pest_derive::Parser;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::io::{self, Write};
use std::thread::scope;
use inkwell::context::Context;

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
pub struct Scanner<'a, W: Write> {
    /// 输入源代码
    input: &'a str,
    /// 错误消息的输出写入器
    writer: &'a mut W,
    /// 作用域栈
    scope_stack: ScopeStack,
}

impl<'a, W: Write> Scanner<'a, W> {
    /// 创建新的语义检查器
    ///
    /// # 参数
    /// * `input` - 要分析的源代码
    /// * `writer` - 错误消息的输出写入器
    pub fn new(input: &'a str, writer: &'a mut W) -> Self {
        Scanner {
            input,
            writer,
            scope_stack: Default::default(),
        }
    }
    pub fn scan_collect(&mut self) -> io::Result<()> {
        if let Some(mut pairs) = parse_file(self.input) {
            // 处理文件头
            let file_pair = pairs.next().unwrap();
            // 读取编译单元
            let compilation_unit = file_pair.into_inner().next().unwrap();
            // 获取所有的声明
            let declarations = compilation_unit.into_inner();
            // 依次扫描声明并收集符号
            for declaration in declarations {
                //TODO 扫描单个声明
            }
        } else {
            writeln!(self.writer, "Syntax error")?;
        }
        Ok(())
    }



    /// 获取目标的规则的迭代器
    fn skip_in(pair: Pair<Rule>) -> impl Iterator<Item = Pair<Rule>> {
        pair.into_inner().into_iter()
    }


    pub fn analyze_const_exp(const_exp: Pair<'_, Rule>) -> usize {
        const_exp.as_str().trim().parse::<usize>().unwrap()
    }


    /// 获取行号
    fn get_line_no(pair: Pair<'_, Rule>) -> usize {
        pair.line_col().0
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum ScopeKey {
    Global, // 全局作用域
    Ident(String), // 函数作用域
    InnerBlock, // 块级作用域
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

    pub fn update(&mut self, key: ScopeKey,scope: Scope) {
        self.scopes.insert(key, scope);
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
        let last_key = self.stack.last().cloned()?; // 克隆键，避免借用冲突
        self.get_scope_mut(&last_key)
    }

    pub fn get_current_scope_key(&self) -> Option<String> {
        let last_key = self.stack.last().cloned()?;
        match last_key {
            ScopeKey::Ident(name) => Some(name),
            _ => None,
        }
    }

    /// 校验contains.需要搜索所有活跃作用域（即 栈中的作用域）
    pub fn contains(&self, name: &str) -> bool {
        for scope_key in self.stack.iter().rev() {
            let scope = self.get_scope(scope_key).unwrap();
            if scope.contains(name.to_string()) {
                return true;
            }
        }
        false
    }

    /// 根据名称从最近的活跃作用域中符号表中抽取一个符号
    pub fn get(&self, name: &str) -> Option<Type> {
        for scope_key in self.stack.iter().rev() {
            let scope = self.get_scope(scope_key).unwrap();
            if let Some(symbol) = scope.get(name.to_string()) {
                return Some(symbol);
            }
        }
        None
    }
}

/// 设定一个初始化的作用域栈
impl Default for ScopeStack {
    fn default() -> Self {
        let stack = Vec::<ScopeKey>::new();
        let mut scope_stack = Self {
            stack,
            scopes: Default::default(),
        };
        scope_stack.push(ScopeKey::Global);
        scope_stack
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

impl Scope {
    fn insert(&mut self, key: String, value: Type) {
        self.symbol_table.insert(key, value);
    }

    fn contains(&self, key: String) -> bool {
        self.symbol_table.contains_key(&key)
    }

    fn get(&self, key: String) -> Option<Type> {
        self.symbol_table.get(&key).cloned()
    }
}

/// 枚举类型
#[derive(Debug, Clone)]
pub enum Type {
    Int(i32),  //整型
    Func(Func), //函数信息
    Void,
}

impl Type {
    pub fn is_func(&self) -> bool {
        match self {
            Type::Func(_) => true,
            _ => false,
        }
    }

    pub fn is_int(&self) -> bool {
        match self {
            Type::Int(_) => true,
            _ => false,
        }
    }

    pub fn is_void(&self) -> bool {
        match self {
            Type::Void => true,
            _ => false,
        }
    }

}

/// 函数类型表示
#[derive(Debug, Clone)]
pub struct Func {
    // 函数入参类型
    pub params: Vec<Type>,
    // 函数返回类型
    pub return_type: Box<Type>,
}

impl Func {
    fn new(params: Vec<Type>, return_type: Type) -> Self {
        Self {
            params,
            return_type: Box::new(return_type),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::gen_llvm_ir::{Scanner, Rule, parse_file};
    use pest::iterators::Pair;
    use std::io::stdout;

    const FILE_PATH: &str = "tests/lab5/";

}
