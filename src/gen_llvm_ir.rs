//! SysY语言的LLVM-IR生成器
//! 前置条件：
//! 默认输入的文件是正确的输入
//! 文件扫描包括：
//! 1、收集变量和函数的符号
//! 2、作用域栈管理
//! 3、符号表管理
//! 4、LLVM-IR生成

#![allow(E0277)]

use crate::utils::{add_option_string, eq_option_string};
use inkwell::types::{BasicMetadataTypeEnum, IntType, VoidType};
use inkwell::values::{BasicValueEnum, FunctionValue, GlobalValue, IntValue, PointerValue};
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
    ir_core: IrCore
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
            ir_core: IrCore::new()
        }
    }
    pub fn scan_collect(&self)  {
        if let Some(mut pairs) = parse_file(self.input) {
            // 处理文件头
            let file_pair = pairs.next().unwrap();
            // 读取编译单元
            let compilation_unit = file_pair.into_inner().next().unwrap();
            // 获取所有的声明
            let declarations = compilation_unit.into_inner();

            let mut ir_session = self.ir_core.start_session("module");

            // 依次扫描声明并收集符号
            for declaration in declarations {
                // dbg!(declaration);
                self.scan_declaration(declaration,&mut ir_session);
            }
            // 输出IR
            ir_session.module.print_to_file("output.ll").unwrap();
        }
    }


    fn scan_declaration<'ctx>(&self,pair: Pair<'_, Rule>,
                              ir_session: &mut IrSession<'ctx>) {
        match pair.as_rule() {
            Rule::Decl => {
                
            },
            Rule::FuncDef => {
                self.scan_func_def(pair, ir_session);
            },
            _ => {},
        }
    }

    fn scan_decl<'ctx>(&self,pair: Pair<'_, Rule>,ir_session: &mut IrSession<'ctx>){
        match pair.as_rule() {
            Rule::ConstDecl => {
                
            },
            Rule::VarDecl => {

            },
            _ => {},
        }
    }

    fn scan_func_def<'ctx>(&self,pair: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>){
        let mut func_def_iter = Self::skip_in(pair);
        let func_type = Self::skip_in(func_def_iter.next().unwrap()).next().unwrap();
        let func_name = func_def_iter.next().unwrap();
        let (fn_ret_type,fn_ret_type_void) = Self::build_fn_ret_type(&func_type, ir_session.context);

        // 尝试抓取函数入参
        // --> 存在入参，需要解析入参，并插入到函数作用域的符号表，构建LLVM-IR时需要用到
        // --> 不存在入参，直接跳过, 构建LLVM-IR时不需要用到
        let mut func_def_iter = func_def_iter.skip(1);
        let params_rule = func_def_iter.next().unwrap();
        let mut func_body = None;
        let mut params = Vec::<String>::new();
        let mut param_types = Vec::<BasicMetadataTypeEnum>::new();
        let i32_type = ir_session.context.i32_type();
        if params_rule.as_rule() == Rule::FuncFParams {
            //解析并收集参数
            params = Self::collect_func_params(params_rule);
            param_types = params.iter().map(|param| {
                i32_type.into()
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
        let function = ir_session.module.add_function(func_name.as_str(), fn_type, None);
        function.get_param_iter().enumerate().for_each(|(i,param)|{
            // 每次入参赋上源码的参数名称
            param.set_name(params[i].as_str());
        });

        // 获取当前作用域
        let scope = ir_session.scope_stack.get_current_scope_mut().unwrap();
        // 将函数插入当前作用域的符号表
        scope.insert(func_name.as_str().to_string(), Type::Func(function));
        // 将函数更新为最近的活跃作用域
        ir_session.scope_stack.push(ScopeKey::Ident(func_name.as_str().to_string()));

        // 获取当前作用域
        let scope = ir_session.scope_stack.get_current_scope_mut().unwrap();
        let block_name = format!("{}Entry", func_name.as_str());
        // 构建一个函数入口的基本块
        let entry_block = ir_session.context.append_basic_block(function, &block_name);
        // 光标移动到入口块末尾
        ir_session.builder.position_at_end(entry_block);

        function.get_param_iter().enumerate().for_each(|(i,param)|{
            // 每次入参赋上源码的参数名称
            let t = params[i].as_str();
            let temp = ir_session.builder.build_alloca(i32_type,t).unwrap();
            temp.set_name(t);
            scope.insert(param.get_name().to_str().unwrap().to_string(),Type::LocalVar(temp));
            param.set_name(t);
        });


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
            self.scan_block_item(block_item, ir_session);
        }
        ir_session.scope_stack.pop();
    }

   
    fn scan_block_item<'ctx>(&self,block_item: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>){
        match block_item.as_rule() {
            Rule::Decl => {
                todo!();
            }
            Rule::Stmt => {
                let stmt_iter = Self::skip_in(block_item);
                for stmt in stmt_iter {
                    self.scan_stmt(stmt, ir_session);
                }
            }
            _ => {},
        }
    }

    /// 扫描stmt
    fn scan_stmt<'ctx>(&self,stmt: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>){
        match stmt.as_rule() {
            Rule::AssignStmt => {
                self.scan_assign_stmt(stmt, ir_session);
            }
            Rule::ExpStmt => {
                self.scan_exp_stmt(stmt, ir_session);
            }
            Rule::Block => {
                self.scan_block(stmt, ir_session);
            }
            Rule::IfStmt => {
                self.scan_if_stmt(stmt, ir_session);
            }
            Rule::WhileStmt => {
                self.scan_while_stmt(stmt, ir_session);
            }
            Rule::BreakStmt => {
                todo!();
            }
            Rule::ContinueStmt => {
                todo!();
            }
            Rule::ReturnStmt => {
                self.scan_return_stmt(stmt, ir_session);
            }
            _ => {},
        }
    }       


    /// 处理赋值语句
    fn scan_assign_stmt<'ctx>(&self,assign_stmt: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>){
        let mut assign_stmt_iter = Self::skip_in(assign_stmt);
        let i32_type = ir_session.context.i32_type();
        let l_val = assign_stmt_iter.next().unwrap();
        // 读取符号表，获取已经分配好的内存地址名称
        let l_val = self.scan_l_val_ident(l_val, ir_session).unwrap();
        let mut assign_stmt_iter = assign_stmt_iter.skip(1);
        let exp = assign_stmt_iter.next().unwrap();
        let temp_reg_name = self.scan_exp_stmt(exp, ir_session);

        // 把寄存器中的表达式结果存储到内存

        todo!();
    }


    fn scan_l_val_ident<'ctx>(&self,l_val: Pair<'_, Rule>,ir_session: &mut IrSession<'ctx>) -> Option<Type<'ctx>> {
        let mut l_val_iter = Self::skip_in(l_val);
        let ident = l_val_iter.next().unwrap();
        let scope  = ir_session.scope_stack.get_current_scope_mut().unwrap();
        scope.get(ident.as_str().to_string())
    }

    /// 处理表达式语句
    /// 寄存器计算的过程：
    /// 1、先把待计算的变量值从内存读入到临时寄存器
    /// 2、利用寄存器计算出结果后
    /// 3、把值从寄存器中取出写回到内存的变量中
    /// 需要返回临时寄存器名称，方便后续的赋值操作
    fn scan_exp_stmt<'ctx>(&self,exp_stmt: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>)->String{
        let mut exp_stmt_iter = Self::skip_in(exp_stmt);
        let exp = exp_stmt_iter.next().unwrap();
        self.scan_exp(exp, ir_session);
        todo!();
    }

    /// 处理语句块
    fn scan_block<'ctx>(&self,block: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>){
        todo!();
    }
    
    /// 处理条件语句
    fn scan_if_stmt<'ctx>(&self,if_stmt: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>){
        todo!();
    }
    
    /// 处理循环语句
    fn scan_while_stmt<'ctx>(&self,while_stmt: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>){
        todo!();
    }
    
    /// 处理返回语句
    fn scan_return_stmt<'ctx>(&self,return_stmt: Pair<'_, Rule>,ir_session: &mut IrSession<'ctx>){
        let exp = Self::skip_in(return_stmt).skip(1).next().unwrap();
        let i32_type = ir_session.context.i32_type();
        if exp.as_rule() != Rule::Exp {
           let _ = ir_session.builder.build_return(None);
            return;
        }
        // 处理exp 的计算结果，并根据结果构建返回值
    }   

    /// 处理表达式
    fn scan_exp<'ctx>(&self,exp: Pair<'_, Rule>,ir_session: &mut IrSession<'ctx>)->Option<IntValue>{
        let mut exp_iter = Self::skip_in(exp);
        let add_exp = exp_iter.next().unwrap();
        self.scan_add_exp(add_exp, ir_session)
    }

    fn scan_add_exp<'ctx>(&self,add_exp: Pair<'_, Rule>,ir_session: &mut IrSession<'ctx>)->Option<IntValue>{
        let mut add_exp_iter = Self::skip_in(add_exp);
        // 读取乘法表达式
        let mul_exp = add_exp_iter.next().unwrap();
        let left_exp = self.scan_mul_exp(mul_exp, ir_session);
        let op = add_exp_iter.next();
        if op.is_some() {
            // 读取另一个乘法表达式
            let mul_exp_another = add_exp_iter.next().unwrap();
            let right_exp = self.scan_mul_exp(mul_exp_another, ir_session);
            if let Some(right) = right_exp && let Some(left) = left_exp {
                match op.unwrap().as_rule() {
                    Rule::Mod => {
                        Some(ir_session.builder.build_int_signed_rem(left,right,"tmp").unwrap())
                    }
                    Rule::Div => {
                        Some(ir_session.builder.build_int_signed_div(left,right,"tmp").unwrap())
                    }
                    Rule::Mul => {
                        Some(ir_session.builder.build_int_mul(left,right,"tmp").unwrap())
                    }
                    _ => None
                }
            }else {
                None
            }
        }else {
           left_exp
        }
    }

    ///
    fn scan_mul_exp<'ctx>(&self,mul_exp: Pair<'_, Rule>,ir_session: &mut IrSession<'ctx>)->Option<IntValue>{
        let mut mul_iter = Self::skip_in(mul_exp);
        let unary_exp = mul_iter.next().unwrap();
        todo!()
    }

    fn scan_unary_exp<'ctx>(&self,exp: Pair<'_, Rule>,ir_session: &mut IrSession<'ctx>)->Option<IntValue>{
        let mut unary_exp_iter = Self::skip_in(exp);
        let op = unary_exp_iter.next();
        match op.unwrap().as_rule() {
            Rule::Not => {}
            Rule::Plus => {}
            Rule::Minus => {}
            _ => None
        }
        todo!()
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

}

#[derive(Debug)]
pub struct IrCore {
    context: Context,
}

impl IrCore {
    pub fn new() -> IrCore {
        Self {
            context:Context::create()
        }
    }
    pub fn start_session(&self, module_name: &str) -> IrSession {
        let module = self.context.create_module(module_name);
        let builder = self.context.create_builder();
        IrSession {
            context : &self.context,
            module,
            builder,
            scope_stack:ScopeStack::default(),
        }
    }
}

/// 参数传递对象
pub struct IrSession<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    pub scope_stack: ScopeStack<'ctx>
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum ScopeKey {
    Global, // 全局作用域
    Ident(String), // 函数作用域
    InnerBlock, // 块级作用域
}

/// 作用域栈
#[derive(Debug)]
pub struct ScopeStack<'ctx> {
    /// 作用域栈
    stack: Vec<ScopeKey>,
    /// 当前作用域的符号表
    scopes: HashMap<ScopeKey,Scope<'ctx>>,
}

#[allow(E0621)]
impl <'ctx> ScopeStack<'ctx> {
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

    pub fn update(&mut self, key: ScopeKey,scope: Scope<'ctx>) {
        self.scopes.insert(key, scope);
    }

    /// 获取作用域
    fn get_scope(&self, key: &ScopeKey) -> Option<&Scope> {
        self.scopes.get(key)
    }

    /// 获取可变的作用域
    fn get_scope_mut(&mut self, key: &ScopeKey) -> Option<&mut Scope<'ctx>> {
        self.scopes.get_mut(key)
    }

    /// 获取当前作用域的符号表
    pub fn get_current_scope(&self) -> Option<&Scope> {
        let last_key = self.stack.last().unwrap();
        self.get_scope(last_key)
    }

    /// 获取可变的当前作用域的符号表
    pub fn get_current_scope_mut(&mut self) -> Option<&mut Scope<'ctx>> {
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
impl<'ctx> Default for ScopeStack<'ctx> {
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
pub struct Scope<'ctx> {
    /// 当前作用域的符号表
    pub symbol_table: HashMap<String, Type<'ctx>>,
}


impl<'ctx> Default for Scope<'ctx> {
    fn default() -> Self {
        Scope {
            symbol_table: HashMap::new(),
        }
    }
}

impl <'ctx> Scope <'ctx> {

    fn insert(&mut self, key: String, value: Type<'ctx>) {
        self.symbol_table.insert(key, value);
    }

    fn contains(&self, key: String) -> bool {
        self.symbol_table.contains_key(&key)
    }

    fn get(&self, key: String) -> Option<Type<'ctx>> {
        self.symbol_table.get(&key).cloned()
    }
}

/// 枚举类型
#[derive(Debug, Clone)]
pub enum Type<'ctx> {
    GlobalVar(GlobalValue<'ctx>),
    Func(FunctionValue<'ctx>), //函数信息只需要管返回类型，其他信息会放在符号表中管理。参数作为作用域的临时变量
    LocalVar(PointerValue<'ctx>), // 本地变量分配的空间,包括函数参数
}

impl <'ctx> Type <'ctx> {
    /// 读取整形值
    pub fn is_func(&self) -> bool {
        match self {
            Type::Func(_) => true,
            _ => false,
        }
    }

    pub fn get_local(&self) -> Option<&PointerValue<'ctx>> {
        if let Type::LocalVar(v) = self {
            return Some(v);
        }
        None
    }

    pub fn get_global(&self) -> Option<&GlobalValue<'ctx>> {
        if let Type::GlobalVar(v) = self {
            return Some(v);
        }
        None
    }

    pub fn get_function(&self) -> Option<&FunctionValue<'ctx>> {
        if let Type::Func(v) = self {
            return Some(v);
        }
        None
    }

}

#[cfg(test)]
mod tests {
    use crate::gen_llvm_ir::{Scanner, Rule, parse_file};
    use pest::iterators::Pair;
    use std::io::stdout;
    use inkwell::context::Context;

    const FILE_PATH: &str = "tests/lab5/";

    
    #[test]
    fn test_scan() {
        let file_path = format!("{}{}", FILE_PATH, "example05.sy");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let mut scanner = Scanner::new(&input);
        scanner.scan_collect();
    }

    #[test]
    fn test_llvm_fn(){
        let context = Context::create();
        let module = context.create_module("main");
        let builder = context.create_builder();
        let i32_type = context.i32_type();
        let fn_type = i32_type.fn_type(&[i32_type.into(), i32_type.into()], false);
        let function = module.add_function("main", fn_type, None);
        let basic_block = context.append_basic_block(function, "entry");
        builder.position_at_end(basic_block);
        // 函数参数处理
        let a = function.get_first_param().unwrap().into_int_value();
        let b = function.get_nth_param(1).unwrap().into_int_value();
        let sum =builder.build_int_add(a, b, "sum").unwrap();

        // 定义变量
        let c = builder.build_alloca(i32_type,"c").unwrap();
        let init_val = i32_type.const_int(1,false);
        let _ = builder.build_store(c,init_val);
        // 变量赋值

        let _ = builder.build_return(Some(&sum));
        module.print_to_stderr();
    }
}
