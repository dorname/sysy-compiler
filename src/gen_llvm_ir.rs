//! SysY语言的LLVM-IR生成器
//! 前置条件：
//! 默认输入的文件是正确的输入
//! 文件扫描包括：
//! 1、收集变量和函数的符号
//! 2、作用域栈管理
//! 3、符号表管理
//! 4、LLVM-IR生成

use crate::utils::{add_option_string, eq_option_string};
use inkwell::types::{BasicMetadataTypeEnum, IntType, VoidType};
use inkwell::values::FunctionValue;
use pest::Parser;
use pest::iterators::{Pair, Pairs};
use pest_derive::Parser;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::io::{self, Write};
use std::thread::scope;
use inkwell::builder::Builder;
use inkwell::context::{self, Context};
use inkwell::module::Module;
use pest::pratt_parser::Op;

#[derive(Parser)]
#[grammar = "pests/scan.pest"]
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
pub struct Scanner<'a> {
    /// 输入源代码
    input: &'a str,
    /// 作用域栈
    scope_stack: ScopeStack,
}

impl<'a> Scanner<'a> {
    /// 创建新的语义检查器
    ///
    /// # 参数
    /// * `input` - 要分析的源代码
    /// * `writer` - 错误消息的输出写入器
    pub fn new(input: &'a str) -> Self {
        Scanner {
            input,
            scope_stack: Default::default(),
        }
    }
    pub fn scan_collect(&mut self)  {
        if let Some(mut pairs) = parse_file(self.input) {
            // 处理文件头
            let file_pair = pairs.next().unwrap();
            // 读取编译单元
            let compilation_unit = file_pair.into_inner().next().unwrap();
            // 获取所有的声明
            let declarations = compilation_unit.into_inner();

            // 初始化构建llvm-ir的工具
            let context = Context::create();
            let module = context.create_module("module");
            let builder = context.create_builder();

            // 依次扫描声明并收集符号
            for declaration in declarations {
                // dbg!(declaration);
                self.scan_declaration(declaration, &context, &module, &builder);
            }
            // 输出IR
            module.print_to_file("output.ll").unwrap();
        }
    }


    fn scan_declaration<'ctx>(&mut self,pair: Pair<'_, Rule>, context: &'ctx Context, module: &Module<'ctx>, builder: &Builder){
        match pair.as_rule() {
            Rule::Decl => {
                
            },
            Rule::FuncDef => {
                self.scan_func_def(pair, context, module, builder);
            },
            _ => {},
        }
    }

    fn scan_decl(&mut self,pair: Pair<'_, Rule>){
        match pair.as_rule() {
            Rule::ConstDecl => {
                
            },
            Rule::VarDecl => {

            },
            _ => {},
        }
    }

    fn scan_func_def<'ctx>(&mut self,pair: Pair<'_, Rule>, context: &'ctx Context, module: &Module<'ctx>, builder: &Builder){
        let mut func_def_iter = Self::skip_in(pair);
        let func_type = Self::skip_in(func_def_iter.next().unwrap()).next().unwrap();
        let func_name = func_def_iter.next().unwrap();

     
        let (fn_ret_type,fn_ret_type_void) = Self::build_fn_ret_type(&func_type, context);

        let func_type = if fn_ret_type.is_some() {
            ReturnType::Int
        } else {
            ReturnType::Void
        };

        // 获取当前作用域
        let scope = self.scope_stack.get_current_scope_mut().unwrap();
        // 将函数插入当前作用域的符号表
        scope.insert(func_name.as_str().to_string(), Type::Func(func_type));
        // 将函数更新为最近的活跃作用域
        self.scope_stack.push(ScopeKey::Ident(func_name.as_str().to_string()));

        // 尝试抓取函数入参
        // --> 存在入参，需要解析入参，并插入到函数作用域的符号表，构建LLVM-IR时需要用到
        // --> 不存在入参，直接跳过, 构建LLVM-IR时不需要用到
        let mut func_def_iter = func_def_iter.skip(1);
        let params_rule = func_def_iter.next().unwrap();
        let mut func_body = None;
        let mut params = Vec::<String>::new();
        let mut param_types = Vec::<BasicMetadataTypeEnum>::new();
        if params_rule.as_rule() == Rule::FuncFParams {
            //解析并收集参数
            params = Self::collect_func_params(params_rule);
            let mut scope = self.scope_stack.get_current_scope_mut().unwrap();
            param_types = params.iter().map(|param| {
                scope.insert(param.clone(), Type::Int(None));
                context.i32_type().into()
            }).collect::<Vec<_>>();
            func_body = func_def_iter.skip(1).next();
        } else {
            func_body = func_def_iter.next();
        }
        let fn_type = if fn_ret_type.is_some() {
            fn_ret_type.unwrap().fn_type(&param_types, false)
        }else {
            fn_ret_type_void.unwrap().fn_type(&param_types, false)
        };
        let function = module.add_function(func_name.as_str(), fn_type, None);
        function.get_param_iter().enumerate().for_each(|(i,param)|{
            // 每次入参赋上源码的参数名称
            param.set_name(params[i].as_str());
        });
        let block_name = format!("{}Entry", func_name.as_str());
        // 构建一个函数入口的基本块
        let entry_block = context.append_basic_block(function, &block_name);
        // 光标移动到入口块末尾
        builder.position_at_end(entry_block);
        // 开始解析函数体
        let mut block_items = Vec::<Pair<'_, Rule>>::new();
        let mut block_item_iter = Self::skip_in(func_body.unwrap());
        while let Some(ref item) = block_item_iter.next() {
            if item.as_rule() == Rule::BlockItem {
                block_items.push(item.clone());
            }
        }
          // 开始处理函数体中的每一项
        for block_item in block_items {
            self.scan_block_item(block_item, context, module, builder,&function);
        }
        self.scope_stack.pop();
    }

   
    fn scan_block_item(&mut self,block_item: Pair<'_, Rule>, context: &Context, module: &Module, builder: &Builder,function: &FunctionValue){
        match block_item.as_rule() {
            Rule::Decl => {
                todo!();
            }
            Rule::Stmt => {
                let stmt_iter = Self::skip_in(block_item);
                for stmt in stmt_iter {
                    self.scan_stmt(stmt, context, module, builder,function);
                }
            }
            _ => {},
        }
    }

    /// 扫描stmt
    fn scan_stmt(&mut self,stmt: Pair<'_, Rule>, context: &Context, module: &Module, builder: &Builder,function: &FunctionValue){
        match stmt.as_rule() {
            Rule::AssignStmt => {
                self.scan_assign_stmt(stmt, context, module, builder);
            }
            Rule::ExpStmt => {
                self.scan_exp_stmt(stmt, context, module, builder);
            }
            Rule::Block => {
                self.scan_block(stmt, context, module, builder,function);
            }
            Rule::IfStmt => {
                self.scan_if_stmt(stmt, context, module, builder,function);
            }
            Rule::WhileStmt => {
                self.scan_while_stmt(stmt, context, module, builder,function);
            }
            Rule::BreakStmt => {
                todo!();
            }
            Rule::ContinueStmt => {
                todo!();
            }
            Rule::ReturnStmt => {
                self.scan_return_stmt(stmt, context, module, builder);
            }
            _ => {},
        }
    }       


    /// 处理赋值语句
    fn scan_assign_stmt(&mut self,assign_stmt: Pair<'_, Rule>, context: &Context, module: &Module, builder: &Builder){
        let mut assign_stmt_iter = Self::skip_in(assign_stmt);
        let i32_type = context.i32_type();
        let lval = assign_stmt_iter.next().unwrap();
        let lval_ident = self.get_lval_ident(lval);
        let lval_mem_store = builder.build_alloca(i32_type, &lval_ident);
        let mut assign_stmt_iter = assign_stmt_iter.skip(1);
        let exp = assign_stmt_iter.next().unwrap();
        let temp_reg_name = self.scan_exp_stmt(exp, context, module, builder);

        // 把寄存器中的表达式结果存储到内存

        todo!();
    }


    fn get_lval_ident(&mut self,lval: Pair<'_, Rule>) -> String {
        let mut lval_iter = Self::skip_in(lval);
        let ident = lval_iter.next().unwrap();
        ident.as_str().to_string()
    }

    /// 处理表达式语句
    /// 寄存器计算的过程：
    /// 1、先把待计算的变量值从内存读入到临时寄存器
    /// 2、利用寄存器计算出结果后
    /// 3、把值从寄存器中取出写回到内存的变量中
    /// 需要返回临时寄存器名称，方便后续的赋值操作
    fn scan_exp_stmt(&mut self,exp_stmt: Pair<'_, Rule>, context: &Context, module: &Module, builder: &Builder)->String{
        todo!();
    }

    /// 处理语句块
    fn scan_block(&mut self,block: Pair<'_, Rule>, context: &Context, module: &Module, builder: &Builder,function: &FunctionValue){
        todo!();
    }
    
    /// 处理条件语句
    fn scan_if_stmt(&mut self,if_stmt: Pair<'_, Rule>, context: &Context, module: &Module, builder: &Builder,function: &FunctionValue){
        todo!();
    }
    
    /// 处理循环语句
    fn scan_while_stmt(&mut self,while_stmt: Pair<'_, Rule>, context: &Context, module: &Module, builder: &Builder,function: &FunctionValue){
        todo!();
    }
    
    /// 处理返回语句
    fn scan_return_stmt(&mut self,return_stmt: Pair<'_, Rule>, context: &Context, module: &Module, builder: &Builder){
        todo!();
    }   

    /// 处理表达式
    fn scan_exp(&mut self,exp: Pair<'_, Rule>, context: &Context, module: &Module, builder: &Builder){
        todo!();
    }

    /// 收集函数入参
    fn collect_func_params(params: Pair<'_, Rule>) -> Vec<String> {
        let mut params_iter = Self::skip_in(params);
        let mut params = Vec::<String>::new();
        while let Some(param) = params_iter.next() {
            if param.as_rule() == Rule::FuncFParam {
                let mut param_iter = Self::skip_in(param).skip(1);
                let ident = param_iter.next().unwrap();
                let ident_name = ident.as_str().to_string();
                params.push(ident_name);
            }
        }
        params
    }

    /// 根据返回类型构建函数的IR基础返回类型
    fn build_fn_ret_type<'ctx>(func_type: &Pair<'_, Rule>, context: &'ctx Context) -> (Option<IntType<'ctx>>, Option<VoidType<'ctx>>) {
        if func_type.as_rule() == Rule::Int {
            (Some(context.i32_type()), None)
        } else {
            (None, Some(context.void_type()))
        }
    }

    /// 获取目标的规则的迭代器
    fn skip_in(pair: Pair<Rule>) -> impl Iterator<Item = Pair<Rule>> {
        pair.into_inner().into_iter()
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
    scopes: HashMap<ScopeKey,Scope>,
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

#[derive(Debug, Clone)]
pub enum ReturnType {
    Void,
    Int
}

impl ReturnType {
    fn is_int(&self) -> bool {
        match self {
            ReturnType::Int => true,
            _ => false,
        }
    }
    
    fn is_void(&self) -> bool {
        match self {
            ReturnType::Void => true,
            _ => false,
        }
    }
}

/// 枚举类型
#[derive(Debug, Clone)]
pub enum Type {
    Int(Option<i32>),  //整型
    Func(ReturnType), //函数信息只需要管返回类型，其他信息会放在符号表中管理。参数作为作用域的临时变量
}

impl Type {
    /// 读取整形值
    pub fn get_value(& self) -> Option<i32> {
        match self {
            Type::Int(value) => value.clone(),
            _ => panic!("Not an integer"),
        }
    }

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

}

#[cfg(test)]
mod tests {
    use crate::gen_llvm_ir::{Scanner, Rule, parse_file};
    use pest::iterators::Pair;
    use std::io::stdout;

    const FILE_PATH: &str = "tests/lab5/";

    
    #[test]
    fn test_scan() {
        let file_path = format!("{}{}", FILE_PATH, "example01.sy");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let mut scanner = Scanner::new(&input);
        scanner.scan_collect();
    }
}
