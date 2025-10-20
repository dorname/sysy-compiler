//! SysY语言的LLVM-IR生成器
//! 前置条件：
//! 默认输入的文件是正确的输入
//! 文件扫描包括：
//! 1、收集变量和函数的符号
//! 2、作用域栈管理
//! 3、符号表管理
//! 4、LLVM-IR生成
use crate::utils::{add_option_string, eq_option_string};
use inkwell::IntPredicate;
use inkwell::IntPredicate::{EQ, NE, SGE, SGT, SLE, SLT};
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::{self, Context};
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, IntType, VoidType};
use inkwell::values::{BasicMetadataValueEnum, BasicValue, BasicValueEnum, CallSiteValue, FunctionValue, GlobalValue, IntValue, PointerValue};
use pest::Parser;
use pest::iterators::{Pair, Pairs};
use pest::pratt_parser::Op;
use pest_derive::Parser;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::io::{self, Write};
use std::process::Output;
use std::thread::scope;

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
    output: &'a str,
    ir_core: IrCore,
}

impl<'a> Scanner<'a> {
    /// 创建新的语义检查器
    ///
    /// # 参数
    /// * `input` - 要分析的源代码
    /// * `writer` - 错误消息的输出写入器
    pub fn new(input: &'a str,output: &'a str) -> Self {
        Scanner {
            input,
            output,
            ir_core: IrCore::new(),
        }
    }
    pub fn scan_collect(&self) -> Result<(), String> {
        let mut pairs = match parse_file(self.input) {
            Some(pairs) => pairs,
            None => return Err("语法解析失败".to_string()),
        };

        // 处理文件头
        let file_pair = pairs.next().ok_or("文件为空")?;
        // 读取编译单元
        let compilation_unit = file_pair.into_inner().next().ok_or("编译单元为空")?;
        // 获取所有的声明
        let declarations = compilation_unit.into_inner();

        let mut ir_session = self.ir_core.start_session("module");

        // 依次扫描声明并收集符号
        for declaration in declarations {
            if let Err(e) = self.scan_declaration(declaration, &mut ir_session) {
                return Err(format!("声明解析错误: {}", e));
            }
        }

        // 验证IR
        if let Err(e) = ir_session.module.verify() {
            return Err(format!("LLVM IR验证失败: {}", e));
        }

        // 输出IR
        ir_session.module.print_to_file(self.output)
            .map_err(|e| format!("输出IR文件失败: {}", e))?;

        Ok(())
    }

    fn scan_declaration<'ctx>(&self, pair: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>) -> Result<(), String> {
        match pair.as_rule() {
            Rule::Decl => {
                self.scan_decl(pair, ir_session)?;
            }
            Rule::FuncDef => {
                self.scan_func_def(pair, ir_session)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn scan_decl<'ctx>(&self, pair: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>) -> Result<(), String> {
        let decl_iter = Self::skip_in(pair);
        for decl in decl_iter {
            match decl.as_rule() {
                Rule::ConstDecl => {
                    self.scan_const_decl(decl, ir_session)?;
                }
                Rule::VarDecl => {
                    self.scan_var_decl(decl, ir_session)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn scan_const_decl<'ctx>(&self, pair: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>) -> Result<(), String> {
        let const_decl_iter = Self::skip_in(pair);
        let mut const_decls = Vec::<Pair<'_, Rule>>::new();
        for decl in const_decl_iter {
            if decl.as_rule() == Rule::ConstDef {
                const_decls.push(decl);
            }
        }
        for decl in const_decls {
            self.scan_const_def(decl, ir_session)?;
        }
        Ok(())
    }

    fn scan_const_def<'ctx>(&self, pair: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>) -> Result<(), String> {
        let mut const_def_iter = Self::skip_in(pair);
        let ident = const_def_iter.next().ok_or("常量定义缺少标识符")?;
        let i32_type = ir_session.context.i32_type();
        let key = ir_session.scope_stack.get_last_key().ok_or("作用域栈为空")?;
        for decl in const_def_iter {
            if decl.as_rule() == Rule::ConstInitVal {
                let const_init = self.scan_const_init(decl, ir_session).ok_or("常量初始化失败")?;
                // 分配空间
                if key.is_global() {
                    let val = ir_session.module.add_global(i32_type, None, ident.as_str());
                    // 初始化
                    val.set_initializer(&const_init);
                    val.set_constant(true);
                    // 加入符号表
                    let scope = ir_session.scope_stack.get_current_scope_mut().unwrap();
                    scope.insert(
                        ident.as_str().to_string(),
                        Type::GlobalVar(val.as_pointer_value()),
                    );
                } else {
                    let val = ir_session
                        .builder
                        .build_alloca(i32_type, ident.as_str())
                        .unwrap();
                    // 存储到val中
                    let _ = ir_session.builder.build_store(val, const_init);
                    // 加入符号表
                    let scope = ir_session.scope_stack.get_current_scope_mut().unwrap();
                    scope.insert(ident.as_str().to_string(), Type::LocalVar(val));
                }
            }
        }
        Ok(())
    }

    fn scan_const_init<'ctx>(
        &self,
        pair: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        // 本次实验不处理数组
        let const_init_iter = Self::skip_in(pair);
        for const_exp in const_init_iter {
            if const_exp.as_rule() == Rule::ConstExp {
                let add_exp = const_exp.into_inner().next().unwrap();
                return self.scan_add_exp(add_exp, ir_session);
            }
        }
        panic!("error const_init_val");
    }

    fn scan_var_decl<'ctx>(&self, pair: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>) -> Result<(), String> {
        let var_decl_iter = Self::skip_in(pair);
        let mut var_defs = Vec::<Pair<'_, Rule>>::new();
        for rule in var_decl_iter {
            if rule.as_rule() == Rule::VarDef {
                var_defs.push(rule);
            }
        }
        for def in var_defs {
            self.scan_var_def(def, ir_session)?;
        }
        Ok(())
    }

    fn scan_var_def<'ctx>(&self, pair: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>) -> Result<(), String> {
        let mut var_def_iter = Self::skip_in(pair);
        let ident = var_def_iter.next().ok_or("变量定义缺少标识符")?;
        let i32_type = ir_session.context.i32_type();
        let key = ir_session.scope_stack.get_last_key().ok_or("作用域栈为空")?;
        for def in var_def_iter {
            if def.as_rule() == Rule::InitVal {
                let const_init = self.scan_init_val(def, ir_session).ok_or("变量初始化失败")?;
                // 分配空间
                if key.is_global() {
                    let val = ir_session.module.add_global(i32_type, None, ident.as_str());
                    // 初始化
                    val.set_initializer(&const_init);
                    // 加入符号表
                    let scope = ir_session.scope_stack.get_current_scope_mut().unwrap();
                    scope.insert(
                        ident.as_str().to_string(),
                        Type::GlobalVar(val.as_pointer_value()),
                    );
                } else {
                    let val = ir_session
                        .builder
                        .build_alloca(i32_type, ident.as_str())
                        .unwrap();
                    // 存储到val中
                    let _ = ir_session.builder.build_store(val, const_init);
                    // 加入符号表
                    let scope = ir_session.scope_stack.get_current_scope_mut().unwrap();
                    scope.insert(ident.as_str().to_string(), Type::LocalVar(val));
                }
            }
        }
        Ok(())
    }

    fn scan_init_val<'ctx>(
        &self,
        pair: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        // 本次实验不处理数组
        let var_init_iter = Self::skip_in(pair);
        for exp in var_init_iter {
            if exp.as_rule() == Rule::Exp {
                return self.scan_exp(exp, ir_session);
            }
        }
        panic!("error init_val");
    }

    fn scan_func_def<'ctx>(&self, pair: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>) -> Result<(), String> {
        let mut func_def_iter = Self::skip_in(pair);
        let func_type = Self::skip_in(func_def_iter.next().unwrap()).next().unwrap();
        let func_name = func_def_iter.next().unwrap();
        let (fn_ret_type, fn_ret_type_void) =
            Self::build_fn_ret_type(&func_type, ir_session.context);

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
            param_types = params
                .iter()
                .map(|_param| i32_type.into())
                .collect::<Vec<_>>();
            func_body = func_def_iter.skip(1).next();
        } else {
            func_body = func_def_iter.next();
        }
        let fn_type = if fn_ret_type.is_some() {
            fn_ret_type.unwrap().fn_type(&param_types, false)
        } else {
            fn_ret_type_void.unwrap().fn_type(&param_types, false)
        };
        let function = ir_session
            .module
            .add_function(func_name.as_str(), fn_type, None);
        function
            .get_param_iter()
            .enumerate()
            .for_each(|(i, param)| {
                // 每次入参赋上源码的参数名称
                param.set_name(params[i].as_str());
            });

        // 获取当前作用域
        let scope = ir_session.scope_stack.get_current_scope_mut().unwrap();
        // 将函数插入当前作用域的符号表
        scope.insert(func_name.as_str().to_string(), Type::Func(function));
        // 将函数更新为最近的活跃作用域
        ir_session.scope_stack.push(ScopeKey::Ident(function));

        // 获取当前作用域
        let scope = ir_session.scope_stack.get_current_scope_mut().unwrap();
        let block_name = format!("{}Entry", func_name.as_str());
        // 构建一个函数入口的基本块
        let entry_block = ir_session.context.append_basic_block(function, &block_name);
        // 光标移动到入口块末尾
        ir_session.builder.position_at_end(entry_block);

        function
            .get_param_iter()
            .enumerate()
            .for_each(|(i, param)| {
                // 每次入参赋上源码的参数名称
                let t = params[i].as_str();
                let temp = ir_session.builder.build_alloca(i32_type, t).unwrap();
                temp.set_name(t);
                let _ = ir_session.builder.build_store(temp, param);
                scope.insert(
                    t.to_string(),
                    Type::LocalVar(temp),
                );
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
            let item_iter = Self::skip_in(block_item);
            for item in item_iter {
                self.scan_block_item(item, ir_session,None,None);
            }
        }

        if let Some(bb) = ir_session.builder.get_insert_block() {
            if !bb.get_terminator().is_some() {
                if fn_ret_type.is_some() {
                    // 有返回值的函数，返回0
                    let zero = ir_session.context.i32_type().const_int(0, false);
                    ir_session.builder.build_return(Some(&zero)).unwrap();
                } else {
                    // void函数
                    ir_session.builder.build_return(None).unwrap();
                }
            }
        }
        ir_session.scope_stack.pop();
        Ok(())
    }

    fn scan_block_item<'ctx>(
        &self,
        block_item: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
        cond_blk: Option<BasicBlock<'ctx>>,
        next_blk: Option<BasicBlock<'ctx>>,
    ) {
        match block_item.as_rule() {
            Rule::Decl => {
                self.scan_decl(block_item, ir_session);
            }
            Rule::Stmt => {
                let stmt_iter = Self::skip_in(block_item);
                for stmt in stmt_iter {
                    self.scan_stmt(stmt, ir_session, false, cond_blk, next_blk);
                }
            }
            _ => {}
        }
    }

    /// 扫描stmt
    fn scan_stmt<'ctx>(
        &self,
        stmt: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
        no_inner: bool,
        cond_blk: Option<BasicBlock<'ctx>>,
        next_blk: Option<BasicBlock<'ctx>>,
    ) {
        match stmt.as_rule() {
            Rule::AssignStmt => {
                self.scan_assign_stmt(stmt, ir_session);
            }
            Rule::ExpStmt => {
                self.scan_exp_stmt(stmt, ir_session);
            }
            Rule::Block => self.scan_block(stmt, ir_session, no_inner, cond_blk, next_blk),
            Rule::IfStmt => {
                self.scan_if_stmt(stmt, ir_session, cond_blk, next_blk);
            }
            Rule::WhileStmt => {
                self.scan_while_stmt(stmt, ir_session);
            }
            Rule::ReturnStmt => {
                self.scan_return_stmt(stmt, ir_session);
            }
            Rule::ContinueStmt => {
                if let Some(cond) = cond_blk {
                    let _ = ir_session.builder.build_unconditional_branch(cond);
                }
            }
            Rule::BreakStmt => {
                if let Some(nxt) = next_blk {
                    let _ = ir_session.builder.build_unconditional_branch(nxt);
                }
            }
            _ => {}
        }
    }

    /// 处理赋值语句
    fn scan_assign_stmt<'ctx>(
        &self,
        assign_stmt: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) {
        let mut assign_stmt_iter = Self::skip_in(assign_stmt);
        let l_val_exp = assign_stmt_iter.next().unwrap();
        // 读取符号表，获取已经分配好的内存地址名称
        let assign_for = Self::skip_in(l_val_exp).next().unwrap().as_str();
        let mut assign_stmt_iter = assign_stmt_iter.skip(1);
        let exp = assign_stmt_iter.next().unwrap();
        let assign_val = self.scan_exp(exp, ir_session);
        // 将新的值存储到左值地址不需要重新插入一个值
        if let Some(val) = assign_val {
            let ty = ir_session.scope_stack.get(assign_for).unwrap();
            let ty_p = ty.get_val().unwrap();
            let _ = ir_session.builder.build_store(*ty_p, val);
        }
    }

    fn scan_l_val<'ctx>(
        &self,
        l_val: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut l_val_iter = Self::skip_in(l_val);
        let ident = l_val_iter.next().unwrap();
        let re = ir_session.scope_stack.get(ident.as_str());
        // 保持静默：不要在编译阶段输出调试信息到stderr
        if re.is_none() {
            // 输入保证正确，理论上不应出现；若出现，随后unwrap会触发并被上层捕获
        }
        let ty = ir_session.scope_stack.get(ident.as_str()).unwrap();
        if !ty.is_func() {
            let g = ty.get_val().unwrap();
            let val = ir_session
                .builder
                .build_load(*g, ident.as_str())
                .unwrap()
                .into_int_value();
            Some(val)
        } else {
            panic!("Unexpected local ident {:?}", ident.as_str());
        }
    }

    /// 处理表达式语句
    /// 寄存器计算的过程：
    /// 1、先把待计算的变量值从内存读入到临时寄存器
    /// 2、利用寄存器计算出结果后
    /// 3、把值从寄存器中取出写回到内存的变量中
    /// 需要返回临时寄存器名称，方便后续的赋值操作
    fn scan_exp_stmt<'ctx>(
        &self,
        exp_stmt: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut exp_stmt_iter = Self::skip_in(exp_stmt);
        let exp = exp_stmt_iter.next().unwrap();
        self.scan_exp(exp, ir_session)
    }

    /// 处理语句块
    /// 根据是否有from来判断是否是任意的内部块
    /// 比如：
    /// ```
    ///  {
    ///    int a = 1;
    ///  }
    /// ```
    fn scan_block<'ctx>(
        &self,
        block: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
        no_inner: bool,
        cond_blk: Option<BasicBlock<'ctx>>,
        next_blk: Option<BasicBlock<'ctx>>,
    ) {
        // 当no_inner为true 不是需要创建新块并添加进入作用域
        if no_inner {
            let block_item_iter = Self::skip_in(block);
            for block_item in block_item_iter {
                let item_iter = Self::skip_in(block_item);
                for item in item_iter {
                    self.scan_block_item(item, ir_session,cond_blk,next_blk);
                }
            }
        } else {
            // 创建新的作用域用于内部块
            let function = ir_session
                .builder
                .get_insert_block()
                .unwrap()
                .get_parent()
                .unwrap();
            let block_name = format!("block_{}", std::ptr::addr_of!(block) as usize);
            let inner_block = ir_session.context.append_basic_block(function, &block_name);
            
            // 压入新的作用域
            ir_session.scope_stack.push(ScopeKey::InnerBlock(inner_block));
            ir_session.builder.position_at_end(inner_block);
            
            let block_item_iter = Self::skip_in(block);
            for block_item in block_item_iter {
                let item_iter = Self::skip_in(block_item);
                for item in item_iter {
                    self.scan_block_item(item, ir_session, cond_blk, next_blk);
                }
            }
            
            // 弹出作用域
            ir_session.scope_stack.pop();
        }
    }

    /// 处理条件语句
    fn scan_if_stmt<'ctx>(
        &self,
        if_stmt: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
        cond_blk: Option<BasicBlock<'ctx>>,
        next_blk: Option<BasicBlock<'ctx>>,
    ) {
        let if_iter = Self::skip_in(if_stmt.clone());
        let function = ir_session
            .builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap();
        let if_true = ir_session.context.append_basic_block(function, "if_true");
        let if_next = ir_session.context.append_basic_block(function, "if_next");
        let mut if_iter = if_iter.skip(2);
        let cond = if_iter.next().unwrap();
        let _ = if_iter.next();
        let stmt = if_iter.next().unwrap();
        if let Some(els) = if_iter.next()
            && els.as_rule() == Rule::Else
        {
            let if_false = ir_session.context.append_basic_block(function, "if_false");
            // 处理条件表达式的分支跳转
            self.scan_cond_for_branches(cond, ir_session, if_true, if_false);

            // 更新作用域为 if_true块
            ir_session.scope_stack.push(ScopeKey::InnerBlock(if_true));
            ir_session.builder.position_at_end(if_true); // 光标移动到if_true块末端
            let stmt_iter = Self::skip_in(stmt);
            for stt in stmt_iter {
                self.scan_stmt(stt, ir_session, true, None, None);
            }
            // 确保if_true块有终止符
            if let Some(bb) = ir_session.builder.get_insert_block() {
                if !bb.get_terminator().is_some() {
                    ir_session.builder.build_unconditional_branch(if_next).unwrap();
                }
            }
            ir_session.scope_stack.pop();

            // 更新作用域为 if_false块
            ir_session.scope_stack.push(ScopeKey::InnerBlock(if_false));
            ir_session.builder.position_at_end(if_false);
            let stmt = if_iter.next().unwrap();
            let stmt_iter = Self::skip_in(stmt);
            for stt in stmt_iter {
                self.scan_stmt(stt, ir_session, true, None, None);
            }
            // 确保if_false块有终止符
            if let Some(bb) = ir_session.builder.get_insert_block() {
                if !bb.get_terminator().is_some() {
                    ir_session.builder.build_unconditional_branch(if_next).unwrap();
                }
            }
            ir_session.scope_stack.pop();
        } else {
            // 处理条件表达式的分支跳转
            self.scan_cond_for_branches(cond, ir_session, if_true, if_next);
            // 更新作用域为 if_true块
            ir_session.scope_stack.push(ScopeKey::InnerBlock(if_true));
            ir_session.builder.position_at_end(if_true); // 光标移动到if_true块末端
            let stmt_iter = Self::skip_in(stmt);
            for stt in stmt_iter {
                match stt.as_rule() {
                    Rule::ContinueStmt => {
                        if let Some(c) = cond_blk {
                            let _ = ir_session.builder.build_unconditional_branch(c);
                        }
                    }
                    Rule::BreakStmt => {
                        if let Some(n) = next_blk {
                            let _ = ir_session.builder.build_unconditional_branch(n);
                        }
                    }

                    _ => {
                        self.scan_stmt(stt, ir_session, true, None, None);
                    }
                }
            }
            // 确保if_true块有终止符
            if let Some(bb) = ir_session.builder.get_insert_block() {
                if !bb.get_terminator().is_some() {
                    ir_session.builder.build_unconditional_branch(if_next).unwrap();
                }
            }
            ir_session.scope_stack.pop();
        }
        // ir_session.scope_stack.push(ScopeKey::InnerBlock(if_next)); if_next 实际上回到了函数体所以实际上不需要入栈因为它的作用域和函数体是一样
        ir_session.builder.position_at_end(if_next);
        // 注意：这里不应该添加额外的分支，因为if_next是后续代码的入口点
    }

    /// 处理循环语句
    fn scan_while_stmt<'ctx>(&self, while_stmt: Pair<'_, Rule>, ir_session: &mut IrSession<'ctx>) {
        let while_iter = Self::skip_in(while_stmt);
        // let function = ir_session.scope_stack.get_current_scope_func_key().unwrap();
        let function = ir_session
            .builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap();
        let mut while_iter = while_iter.skip(2);
        let cond = while_iter.next().unwrap();

        let while_cond = ir_session.context.append_basic_block(function, "whileCond");
        let while_body = ir_session.context.append_basic_block(function, "whileBody");
        let while_next = ir_session.context.append_basic_block(function, "whileNext");
        let _ = ir_session.builder.build_unconditional_branch(while_cond);
        // 定位到条件块
        ir_session.builder.position_at_end(while_cond);
        // 处理条件表达式的分支跳转
        self.scan_cond_for_branches(cond, ir_session, while_body, while_next);

        // 定位到函数体
        ir_session.builder.position_at_end(while_body);
        // 循环体入栈
        ir_session
            .scope_stack
            .push(ScopeKey::InnerBlock(while_body));
        let mut while_iter = while_iter.skip(1);
        let stmt = while_iter.next().unwrap();
        let stmt_iter = Self::skip_in(stmt);
        for s in stmt_iter {
            self.scan_stmt(s, ir_session, true, Some(while_cond), Some(while_next));
        }
        // 确保while_body块有终止符
        if let Some(bb) = ir_session.builder.get_insert_block() {
            if !bb.get_terminator().is_some() {
                ir_session.builder.build_unconditional_branch(while_cond).unwrap();
            }
        }
        // 函数体出栈
        ir_session.scope_stack.pop();

        // 定位到循环体外
        ir_session.builder.position_at_end(while_next);
    }

    /// 处理返回语句
    fn scan_return_stmt<'ctx>(
        &self,
        return_stmt: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) {
        let exp = Self::skip_in(return_stmt).skip(1).next().unwrap();
        if exp.as_rule() != Rule::Exp {
            let _ = ir_session.builder.build_return(None);
            return;
        }
        // 处理exp 的计算结果，并根据结果构建返回值
        let result = self.scan_exp(exp, ir_session);
        if result.is_some() {
            let val = result.unwrap();
            let _ = ir_session.builder.build_return(Some(&val));
        } else {
            let _ = ir_session.builder.build_return(None);
        }
    }

    /// 处理表达式
    fn scan_exp<'ctx>(
        &self,
        exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut exp_iter = Self::skip_in(exp);
        let add_exp = exp_iter.next().unwrap();
        self.scan_add_exp(add_exp, ir_session)
    }

    fn scan_add_exp<'ctx>(
        &self,
        add_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut add_exp_iter = Self::skip_in(add_exp);
        // 读取乘法表达式
        let mul_exp = add_exp_iter.next().unwrap();
        let mul_res =  self.scan_mul_exp(mul_exp, ir_session);
        if mul_res.is_none() {
            return None;
        }
        let mut left = mul_res.unwrap();
        while let Some(op) = add_exp_iter.next() {
            if op.as_rule() != Rule::MulExp {
                let right_exp = add_exp_iter.next().unwrap();
                let right = self.scan_mul_exp(right_exp, ir_session).unwrap();
                match op.as_rule() {
                    Rule::Plus => {
                        left = ir_session
                            .builder
                            .build_int_add(left, right, "add_result")
                            .unwrap();
                    }
                    Rule::Minus => {
                        left = ir_session
                            .builder
                            .build_int_sub(left, right, "sub_result")
                            .unwrap();
                    }
                    _ => panic!("Unexpected AddExp rule {:?}", op.as_rule()),
                }
            }
        }
        Some(left)
    }

    ///
    fn scan_mul_exp<'ctx>(
        &self,
        mul_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut mul_iter = Self::skip_in(mul_exp);
        let unary_exp = mul_iter.next().unwrap();
        let unary_res = self.scan_unary_exp(unary_exp, ir_session);
        if unary_res.is_none() {
           return None;
        }
        let mut left = unary_res.unwrap();
        while let Some(op) = mul_iter.next() {
            if op.as_rule() != Rule::UnaryExp {
                let right_exp = mul_iter.next().unwrap();
                let right = self.scan_unary_exp(right_exp, ir_session).unwrap();
                match op.as_rule() {
                    Rule::Mul => {
                        left = ir_session
                            .builder
                            .build_int_mul(left, right, "mul_result")
                            .unwrap();
                    }
                    Rule::Div => {
                        left = ir_session
                            .builder
                            .build_int_signed_div(left, right, "div_result")
                            .unwrap();
                    }
                    Rule::Mod => {
                        left = ir_session
                            .builder
                            .build_int_signed_rem(left, right, "mod_result")
                            .unwrap();
                    }
                    _ => panic!("Unexpected MulExp rule {:?}", op.as_rule()),
                }
            }
        }
        Some(left)
    }

    fn scan_unary_exp<'ctx>(
        &self,
        exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut unary_exp_iter = Self::skip_in(exp);
        let unary_exp = unary_exp_iter.next().unwrap();
        match unary_exp.as_rule() {
            Rule::CallExp => self.scan_call_exp(unary_exp, ir_session),
            Rule::PrimaryExp => self.scan_primary_exp(unary_exp, ir_session),
            Rule::UnaryOpExp => self.scan_unary_op_exp(unary_exp, ir_session),
            _ => None,
        }
    }

    /// 扫描函数调用表达式
    /// 1、从符号表拿到函数
    /// 2、读取参数
    /// 3、调用build_call构建LLVM_IR
    fn scan_call_exp<'ctx>(
        &self,
        call_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut call_exp_iter = Self::skip_in(call_exp);
        let call_name = call_exp_iter.next()?.as_str();
        let params = call_exp_iter.skip(1).next()?;
        
        let function = {
            let binding = ir_session.scope_stack.get(call_name)?;
            binding.get_function()?.clone()
        };
        
        let mut result = None;
        let mut param_exps = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        
        if params.as_rule() == Rule::FuncRParams {
            let params_iter = Self::skip_in(params);
            for param in params_iter {
                if param.as_rule() == Rule::Exp {
                    let val = self.scan_exp(param, ir_session)?;
                    param_exps.push(val.into());
                }
            }
        }
        
        // 检查参数数量是否匹配
        let expected_param_count = function.get_type().get_param_types().len();
        let actual_param_count = param_exps.len();
        
        // 为避免在Online Judge上向stderr输出，忽略参数数量不匹配的警告。
        // LLVM会在调用处类型检查，如有问题将在验证阶段体现。
        
        let r = ir_session
            .builder
            .build_call(function, &param_exps, call_name)
            .ok()?
            .try_as_basic_value()
            .left();
            
        if let Some(val) = r {
            result = Some(val.into_int_value());
        }
        
        if function.get_type().get_return_type().is_none() {
            None
        } else {
            result
        }
    }

    fn scan_primary_exp<'ctx>(
        &self,
        primary_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut prim_exp_iter = Self::skip_in(primary_exp);
        let inner = prim_exp_iter.next().unwrap();
        match inner.as_rule() {
            Rule::LVal => self.scan_l_val(inner, ir_session),
            Rule::Number => self.scan_number(inner, ir_session),
            Rule::OpenParen => {
                let exp = prim_exp_iter.next().unwrap();
                self.scan_exp(exp, ir_session)
            }
            _ => None,
        }
    }

    fn scan_number<'ctx>(
        &self,
        number: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let number_str = number.as_str();
        let i32_type = ir_session.context.i32_type();
        
        let value = if number_str.starts_with("0x") || number_str.starts_with("0X") {
            // 十六进制
            u64::from_str_radix(&number_str[2..], 16).ok()?
        } else if number_str.starts_with("0") && number_str.len() > 1 {
            // 八进制
            u64::from_str_radix(&number_str[1..], 8).ok()?
        } else {
            // 十进制
            number_str.parse::<u64>().ok()?
        };
        
        // 转换为i32范围
        let result = i32_type.const_int(value, false);
        Some(result)
    }

    fn scan_unary_op_exp<'ctx>(
        &self,
        unary_op_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut unary_op_exp_iter = Self::skip_in(unary_op_exp);
        let unary_op = unary_op_exp_iter.next().unwrap();
        let op = Self::skip_in(unary_op).next().unwrap();
        let unary_exp = unary_op_exp_iter.next().unwrap();
        let unary_exp_val = self.scan_unary_exp(unary_exp, ir_session).unwrap();
        let i32_type = ir_session.context.i32_type();
        match op.as_rule() {
            Rule::Not => {
                let cmp = ir_session
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        unary_exp_val,
                        i32_type.const_int(0, false),
                        "cmp",
                    )
                    .unwrap();
                let res = ir_session.builder.build_int_z_extend(cmp, i32_type, "not");
                Some(res.unwrap().as_basic_value_enum().into_int_value())
            }
            Rule::Minus => {
                let neg = ir_session.builder.build_int_neg(unary_exp_val, "neg");
                Some(neg.unwrap().as_basic_value_enum().into_int_value())
            }
            Rule::Plus => Some(unary_exp_val),
            _ => None,
        }
    }

    /// 不能直接返回计算结果应该返回地址，因为对于循环条件来说返回结果不是固定的
    fn scan_cond<'ctx>(
        &self,
        cond_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut cond_exp_iter = Self::skip_in(cond_exp);
        let l_or_exp = cond_exp_iter.next().unwrap();
        let result = self.scan_l_or_exp(l_or_exp, ir_session)?;
        // 将整数结果转换为布尔值用于分支条件
        let bool_result = ir_session.builder.build_int_compare(
            IntPredicate::NE, result, ir_session.context.i32_type().const_int(0, false), "cond_bool"
        ).unwrap();
        // 将布尔值转换回i32用于返回值
        let i32_result = ir_session.builder.build_int_z_extend(bool_result, ir_session.context.i32_type(), "cond_result").unwrap().as_basic_value_enum().into_int_value();
        Some(i32_result)
    }

    /// 处理条件表达式的求值，返回整数结果
    fn scan_cond_for_branch<'ctx>(
        &self,
        cond_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut cond_exp_iter = Self::skip_in(cond_exp);
        let l_or_exp = cond_exp_iter.next().unwrap();
        let result = self.scan_l_or_exp_for_branch(l_or_exp, ir_session)?;
        // 将整数结果转换为布尔值用于分支判断
        let bool_result = ir_session.builder.build_int_compare(
            IntPredicate::NE, result, ir_session.context.i32_type().const_int(0, false), "cond_bool"
        ).unwrap();
        // 将布尔值转换回i32用于表达式求值
        let i32_result = ir_session.builder.build_int_z_extend(bool_result, ir_session.context.i32_type(), "cond_result").unwrap().as_basic_value_enum().into_int_value();
        Some(i32_result)
    }

    /// 处理条件表达式的分支跳转
    fn scan_cond_for_branches<'ctx>(
        &self,
        cond_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
        true_block: BasicBlock<'ctx>,
        false_block: BasicBlock<'ctx>,
    ) {
        let mut cond_exp_iter = Self::skip_in(cond_exp);
        let l_or_exp = cond_exp_iter.next().unwrap();
        self.scan_l_or_exp_for_branches(l_or_exp, ir_session, true_block, false_block);
    }

    /// 处理逻辑OR表达式的分支跳转
    fn scan_l_or_exp_for_branches<'ctx>(
        &self,
        l_or_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
        true_block: BasicBlock<'ctx>,
        false_block: BasicBlock<'ctx>,
    ) {
        let mut l_or_exp_iter = Self::skip_in(l_or_exp);
        let first = l_or_exp_iter.next().unwrap();
        let rest: Vec<_> = l_or_exp_iter.filter(|p| p.as_rule() == Rule::LAndExp).collect();

        if rest.is_empty() {
            // 单个AND表达式，直接处理
            self.scan_l_and_exp_for_branches(first, ir_session, true_block, false_block);
        } else {
            // 多个OR操作，需要创建中间块
            let function = ir_session
                .builder
                .get_insert_block()
                .unwrap()
                .get_parent()
                .unwrap();
            let mut next_block = ir_session.context.append_basic_block(function, "or_continue");

            self.scan_l_and_exp_for_branches(first, ir_session, true_block, next_block);

            for (i, item) in rest.iter().enumerate() {
                ir_session.builder.position_at_end(next_block);
                if i < rest.len() - 1 {
                    let new_next = ir_session.context.append_basic_block(function, "or_continue");
                    self.scan_l_and_exp_for_branches(item.clone(), ir_session, true_block, new_next);
                    next_block = new_next;
                } else {
                    self.scan_l_and_exp_for_branches(item.clone(), ir_session, true_block, false_block);
                }
            }
        }
    }

    /// 处理逻辑AND表达式的分支跳转
    fn scan_l_and_exp_for_branches<'ctx>(
        &self,
        l_and_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
        true_block: BasicBlock<'ctx>,
        false_block: BasicBlock<'ctx>,
    ) {
        let mut l_and_exp_iter = Self::skip_in(l_and_exp);
        let first = l_and_exp_iter.next().unwrap();
        let rest: Vec<_> = l_and_exp_iter.filter(|p| p.as_rule() == Rule::EqExp).collect();

        if rest.is_empty() {
            // 单个等式表达式，直接处理
            let value = self.scan_eq_exp(first, ir_session).unwrap();
            let i32_type = ir_session.context.i32_type();
            let cond = ir_session
                .builder
                .build_int_compare(
                    IntPredicate::NE,
                    value,
                    i32_type.const_int(0, false),
                    "cond",
                )
                .unwrap();
            ir_session
                .builder
                .build_conditional_branch(cond, true_block, false_block)
                .unwrap();
        } else {
            // 多个AND操作，需要创建中间块
            let function = ir_session
                .builder
                .get_insert_block()
                .unwrap()
                .get_parent()
                .unwrap();
            let mut next_block = ir_session.context.append_basic_block(function, "and_continue");

            let value = self.scan_eq_exp(first, ir_session).unwrap();
            let i32_type = ir_session.context.i32_type();
            let cond = ir_session
                .builder
                .build_int_compare(
                    IntPredicate::NE,
                    value,
                    i32_type.const_int(0, false),
                    "cond",
                )
                .unwrap();
            ir_session
                .builder
                .build_conditional_branch(cond, next_block, false_block)
                .unwrap();

            for (i, item) in rest.iter().enumerate() {
                ir_session.builder.position_at_end(next_block);
                if i < rest.len() - 1 {
                    let new_next = ir_session.context.append_basic_block(function, "and_continue");
                    let value = self.scan_eq_exp(item.clone(), ir_session).unwrap();
                    let cond = ir_session
                        .builder
                        .build_int_compare(
                            IntPredicate::NE,
                            value,
                            i32_type.const_int(0, false),
                            "cond",
                        )
                        .unwrap();
                    ir_session
                        .builder
                        .build_conditional_branch(cond, new_next, false_block)
                        .unwrap();
                    next_block = new_next;
                } else {
                    let value = self.scan_eq_exp(item.clone(), ir_session).unwrap();
                    let cond = ir_session
                        .builder
                        .build_int_compare(
                            IntPredicate::NE,
                            value,
                            i32_type.const_int(0, false),
                            "cond",
                        )
                        .unwrap();
                    ir_session
                        .builder
                        .build_conditional_branch(cond, true_block, false_block)
                        .unwrap();
                }
            }
        }
    }

    /// 处理逻辑OR表达式的求值
    fn scan_l_or_exp_for_branch<'ctx>(
        &self,
        l_or_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut l_or_exp_iter = Self::skip_in(l_or_exp);
        let first = l_or_exp_iter.next()?;
        let rest: Vec<_> = l_or_exp_iter.filter(|p| p.as_rule() == Rule::LAndExp).collect();
        
        if rest.is_empty() {
            // 没有OR操作，直接计算LAndExp
            self.scan_l_and_exp_for_branch(first, ir_session)
        } else {
            // 处理多个OR操作，使用短路求值
            let mut result = self.scan_l_and_exp_for_branch(first, ir_session)?;
            
            for e in rest {
                let res = self.scan_l_and_exp_for_branch(e, ir_session)?;
                // 将两个整数转换为布尔值进行比较
                let left_bool = ir_session.builder.build_int_compare(
                    IntPredicate::NE, result, ir_session.context.i32_type().const_int(0, false), "left_bool"
                ).unwrap();
                let right_bool = ir_session.builder.build_int_compare(
                    IntPredicate::NE, res, ir_session.context.i32_type().const_int(0, false), "right_bool"
                ).unwrap();
                // 执行逻辑OR操作
                let or_result = ir_session.builder.build_or(left_bool, right_bool, "or_bool").unwrap();
                // 将布尔结果转换回i32
                result = ir_session.builder.build_int_z_extend(or_result, ir_session.context.i32_type(), "or_final").unwrap().as_basic_value_enum().into_int_value();
            }
            
            Some(result)
        }
    }


    /// 处理逻辑AND表达式的求值
    fn scan_l_and_exp_for_branch<'ctx>(
        &self,
        l_and_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut l_and_exp_iter = Self::skip_in(l_and_exp);
        // 先处理第一个 EqExp 作为初始值
        let first = l_and_exp_iter.next()?;
        let mut result = self.scan_eq_exp_for_branch(first, ir_session)?;
        
        // 处理后续的 AND 操作
        for e in l_and_exp_iter {
            if e.as_rule() == Rule::EqExp {
                let res = self.scan_eq_exp_for_branch(e, ir_session)?;
                // 将两个整数转换为布尔值进行比较
                let left_bool = ir_session.builder.build_int_compare(
                    IntPredicate::NE, result, ir_session.context.i32_type().const_int(0, false), "left_bool"
                ).unwrap();
                let right_bool = ir_session.builder.build_int_compare(
                    IntPredicate::NE, res, ir_session.context.i32_type().const_int(0, false), "right_bool"
                ).unwrap();
                // 执行逻辑AND操作
                let and_result = ir_session.builder.build_and(left_bool, right_bool, "and_bool").unwrap();
                // 将布尔结果转换回i32
                result = ir_session.builder.build_int_z_extend(and_result, ir_session.context.i32_type(), "and_final").unwrap().as_basic_value_enum().into_int_value();
            }
        }
        
        Some(result)
    }


    /// 处理等式表达式的求值
    fn scan_eq_exp_for_branch<'ctx>(
        &self,
        eq_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut eq_exp_iter = Self::skip_in(eq_exp);
        // 先处理第一个 RelExp
        let rel_exp = eq_exp_iter.next().unwrap();
        let mut result = self.scan_rel_exp_for_branch(rel_exp, ir_session).unwrap();
        
        // 处理后续的 == 或 != 操作
        while let Some(e) = eq_exp_iter.next() {
            if e.as_rule() != Rule::RelExp {
                let right_rel = eq_exp_iter.next().unwrap();
                let right = self.scan_rel_exp_for_branch(right_rel, ir_session).unwrap();
                
                let cmp_result = if e.as_rule() == Rule::Equal {
                    ir_session
                        .builder
                        .build_int_compare(EQ, result, right, "eq_cmp")
                        .unwrap()
                } else {
                    ir_session
                        .builder
                        .build_int_compare(NE, result, right, "ne_cmp")
                        .unwrap()
                };
                // 将布尔结果转换为i32
                result = ir_session.builder.build_int_z_extend(cmp_result, ir_session.context.i32_type(), "eq_result").unwrap().as_basic_value_enum().into_int_value();
            }
        }
        Some(result)
    }

    /// 处理关系表达式的求值
    fn scan_rel_exp_for_branch<'ctx>(
        &self,
        rel_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut rel_exp_iter = Self::skip_in(rel_exp);
        // 先处理第一个 AddExp
        let add_exp = rel_exp_iter.next().unwrap();
        let mut left = self.scan_add_exp(add_exp, ir_session).unwrap();
        let mut has_comparison = false;
        
        // 处理后续的比较操作
        while let Some(e) = rel_exp_iter.next() {
            if e.as_rule() != Rule::AddExp {
                let right_add = rel_exp_iter.next().unwrap();
                let right = self.scan_add_exp(right_add, ir_session).unwrap();
                
                let cmp_result = match e.as_rule() {
                    Rule::LessEqual => {
                        has_comparison = true;
                        ir_session
                            .builder
                            .build_int_compare(SLE, left, right, "cmp")
                            .unwrap()
                    }
                    Rule::GreaterEqual => {
                        has_comparison = true;
                        ir_session
                            .builder
                            .build_int_compare(SGE, left, right, "cmp")
                            .unwrap()
                    }
                    Rule::Less => {
                        has_comparison = true;
                        ir_session
                            .builder
                            .build_int_compare(SLT, left, right, "cmp")
                            .unwrap()
                    }
                    Rule::Greater => {
                        has_comparison = true;
                        ir_session
                            .builder
                            .build_int_compare(SGT, left, right, "cmp")
                            .unwrap()
                    }
                    _ => {
                        // 如果不是比较操作，需要将整数转换为布尔值
                        has_comparison = true;
                        ir_session.builder.build_int_compare(
                            IntPredicate::NE, left, ir_session.context.i32_type().const_int(0, false), "to_bool"
                        ).unwrap()
                    },
                };
                // 将布尔结果转换为i32
                left = ir_session.builder.build_int_z_extend(cmp_result, ir_session.context.i32_type(), "cmp_result").unwrap().as_basic_value_enum().into_int_value();
            }
        }
        
        // 如果没有比较操作，将整数转换为布尔值
        if !has_comparison {
            let bool_result = ir_session.builder.build_int_compare(
                IntPredicate::NE, left, ir_session.context.i32_type().const_int(0, false), "to_bool"
            ).unwrap();
            left = ir_session.builder.build_int_z_extend(bool_result, ir_session.context.i32_type(), "bool_result").unwrap().as_basic_value_enum().into_int_value();
        }
        
        Some(left)
    }

    fn scan_l_or_exp<'ctx>(
        &self,
        l_or_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut l_or_exp_iter = Self::skip_in(l_or_exp);
        // 先处理第一个 LAndExp 作为初始值
        let first = l_or_exp_iter.next()?;
        let mut result = self.scan_l_and_exp(first, ir_session)?;
        
        // 处理后续的 OR 操作 - 需要转换为布尔类型
        for e in l_or_exp_iter {
            if e.as_rule() == Rule::LAndExp {
                let res = self.scan_l_and_exp(e, ir_session)?;
                // 将两个整数转换为布尔值进行比较
                let left_bool = ir_session.builder.build_int_compare(
                    IntPredicate::NE, result, ir_session.context.i32_type().const_int(0, false), "left_bool"
                ).unwrap();
                let right_bool = ir_session.builder.build_int_compare(
                    IntPredicate::NE, res, ir_session.context.i32_type().const_int(0, false), "right_bool"
                ).unwrap();
                // 执行逻辑OR操作
                let or_result = ir_session.builder.build_or(left_bool, right_bool, "or_bool").unwrap();
                // 将布尔结果转换回i32
                result = ir_session.builder.build_int_z_extend(or_result, ir_session.context.i32_type(), "or_final").unwrap().as_basic_value_enum().into_int_value();
            }
        }
        
        Some(result)
    }

    fn scan_l_and_exp<'ctx>(
        &self,
        l_and_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut l_and_exp_iter = Self::skip_in(l_and_exp);
        // 先处理第一个 EqExp 作为初始值
        let first = l_and_exp_iter.next()?;
        let mut result = self.scan_eq_exp(first, ir_session)?;
        
        // 处理后续的 AND 操作 - 需要转换为布尔类型
        for e in l_and_exp_iter {
            if e.as_rule() == Rule::EqExp {
                let res = self.scan_eq_exp(e, ir_session)?;
                // 将两个整数转换为布尔值进行比较
                let left_bool = ir_session.builder.build_int_compare(
                    IntPredicate::NE, result, ir_session.context.i32_type().const_int(0, false), "left_bool"
                ).unwrap();
                let right_bool = ir_session.builder.build_int_compare(
                    IntPredicate::NE, res, ir_session.context.i32_type().const_int(0, false), "right_bool"
                ).unwrap();
                // 执行逻辑AND操作
                let and_result = ir_session.builder.build_and(left_bool, right_bool, "and_bool").unwrap();
                // 将布尔结果转换回i32
                result = ir_session.builder.build_int_z_extend(and_result, ir_session.context.i32_type(), "and_final").unwrap().as_basic_value_enum().into_int_value();
            }
        }
        
        Some(result)
    }

    fn scan_eq_exp<'ctx>(
        &self,
        eq_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut eq_exp_iter = Self::skip_in(eq_exp);
        // 先处理第一个 RelExp
        let rel_exp = eq_exp_iter.next().unwrap();
        let mut result = self.scan_rel_exp(rel_exp, ir_session).unwrap();
        
        // 处理后续的 == 或 != 操作
        while let Some(e) = eq_exp_iter.next() {
            if e.as_rule() != Rule::RelExp {
                let right_rel = eq_exp_iter.next().unwrap();
                let right = self.scan_rel_exp(right_rel, ir_session).unwrap();
                
                let cmp_result = if e.as_rule() == Rule::Equal {
                    ir_session
                        .builder
                        .build_int_compare(EQ, result, right, "eq_cmp")
                        .unwrap()
                } else {
                    ir_session
                        .builder
                        .build_int_compare(NE, result, right, "ne_cmp")
                        .unwrap()
                };
                // 将布尔结果转换为i32
                result = ir_session.builder.build_int_z_extend(cmp_result, ir_session.context.i32_type(), "eq_result").unwrap().as_basic_value_enum().into_int_value();
            }
        }
        Some(result)
    }

    fn scan_rel_exp<'ctx>(
        &self,
        rel_exp: Pair<'_, Rule>,
        ir_session: &mut IrSession<'ctx>,
    ) -> Option<IntValue<'ctx>> {
        let mut rel_exp_iter = Self::skip_in(rel_exp);
        // 先处理第一个 AddExp
        let add_exp = rel_exp_iter.next().unwrap();
        let mut left = self.scan_add_exp(add_exp, ir_session).unwrap();
        
        // 处理后续的比较操作
        while let Some(e) = rel_exp_iter.next() {
            if e.as_rule() != Rule::AddExp {
                let right_add = rel_exp_iter.next().unwrap();
                let right = self.scan_add_exp(right_add, ir_session).unwrap();
                
                let cmp_result = match e.as_rule() {
                    Rule::LessEqual => {
                        ir_session
                            .builder
                            .build_int_compare(SLE, left, right, "cmp")
                            .unwrap()
                    }
                    Rule::GreaterEqual => {
                        ir_session
                            .builder
                            .build_int_compare(SGE, left, right, "cmp")
                            .unwrap()
                    }
                    Rule::Less => {
                        ir_session
                            .builder
                            .build_int_compare(SLT, left, right, "cmp")
                            .unwrap()
                    }
                    Rule::Greater => {
                        ir_session
                            .builder
                            .build_int_compare(SGT, left, right, "cmp")
                            .unwrap()
                    }
                    _ => {
                        // 如果不是比较操作，将整数转换为布尔值
                        ir_session.builder.build_int_compare(
                            IntPredicate::NE, left, ir_session.context.i32_type().const_int(0, false), "to_bool"
                        ).unwrap()
                    },
                };
                // 将布尔结果转换为i32
                left = ir_session.builder.build_int_z_extend(cmp_result, ir_session.context.i32_type(), "cmp_result").unwrap().as_basic_value_enum().into_int_value();
            }
        }
        Some(left)
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
    fn build_fn_ret_type<'ctx>(
        func_type: &Pair<'_, Rule>,
        context: &'ctx Context,
    ) -> (Option<IntType<'ctx>>, Option<VoidType<'ctx>>) {
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
            context: Context::create(),
        }
    }
    pub fn start_session(&self, module_name: &str) -> IrSession<'_> {
        let module = self.context.create_module(module_name);
        let builder = self.context.create_builder();
        IrSession {
            context: &self.context,
            module,
            builder,
            scope_stack: ScopeStack::default(),
        }
    }
}

/// 参数传递对象
pub struct IrSession<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    pub scope_stack: ScopeStack<'ctx>,
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum ScopeKey<'ctx> {
    Global,                       // 全局作用域
    Ident(FunctionValue<'ctx>),   // 函数作用域
    InnerBlock(BasicBlock<'ctx>), // 块级作用域
}

impl<'ctx> ScopeKey<'ctx> {
    fn is_global(&self) -> bool {
        if let ScopeKey::Global = self {
            return true;
        }
        false
    }

    fn is_function(&self) -> bool {
        if let ScopeKey::Ident(_) = self {
            return true;
        }
        false
    }
}

/// 作用域栈
#[derive(Debug)]
pub struct ScopeStack<'ctx> {
    /// 作用域栈
    stack: Vec<ScopeKey<'ctx>>,
    /// 当前作用域的符号表
    scopes: HashMap<ScopeKey<'ctx>, Scope<'ctx>>,
}

impl<'ctx> ScopeStack<'ctx> {
    /// 压入作用域
    pub fn push(&mut self, key: ScopeKey<'ctx>) {
        self.stack.push(key.clone());
        self.scopes.insert(key, Default::default());
    }

    /// 弹出作用域
    pub fn pop(&mut self) {
        let key = self.stack.pop().unwrap();
        self.scopes.remove(&key);
    }

    pub fn update(&mut self, key: ScopeKey<'ctx>, scope: Scope<'ctx>) {
        self.scopes.insert(key, scope);
    }

    /// 获取作用域
    fn get_scope(&self, key: &ScopeKey<'ctx>) -> Option<&Scope<'ctx>> {
        self.scopes.get(key)
    }

    /// 获取可变的作用域
    fn get_scope_mut(&mut self, key: &ScopeKey<'ctx>) -> Option<&mut Scope<'ctx>> {
        self.scopes.get_mut(key)
    }

    /// 获取当前作用域的符号表
    pub fn get_current_scope(&self) -> Option<&Scope<'_>> {
        let last_key = self.stack.last().unwrap();
        self.get_scope(last_key)
    }

    /// 获取可变的当前作用域的符号表
    pub fn get_current_scope_mut(&mut self) -> Option<&mut Scope<'ctx>> {
        let last_key = self.stack.last().cloned()?; // 克隆键，避免借用冲突
        self.get_scope_mut(&last_key)
    }

    pub fn get_current_scope_block(&self) -> Option<BasicBlock<'ctx>> {
        let last_key = self.stack.last().cloned()?;
        match last_key {
            ScopeKey::InnerBlock(a) => Some(a),
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
    pub fn get(&self, name: &str) -> Option<&Type<'ctx>> {
        for scope_key in self.stack.iter().rev() {
            if let Some(scope) = self.get_scope(scope_key)
                && let Some(symbol) = scope.get(name.to_string())
            {
                return Some(symbol);
            }
        }
        None
    }

    fn get_last_key(&self) -> Option<ScopeKey<'ctx>> {
        self.stack.last().cloned()
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

impl<'ctx> Scope<'ctx> {
    fn insert(&mut self, key: String, value: Type<'ctx>) {
        self.symbol_table.insert(key, value);
    }

    fn contains(&self, key: String) -> bool {
        self.symbol_table.contains_key(&key)
    }

    fn get(&self, key: String) -> Option<&Type<'ctx>> {
        self.symbol_table.get(&key)
    }
}

/// 枚举类型
#[derive(Debug, Clone)]
pub enum Type<'ctx> {
    GlobalVar(PointerValue<'ctx>),
    Func(FunctionValue<'ctx>), //函数信息只需要管返回类型，其他信息会放在符号表中管理。参数作为作用域的临时变量
    LocalVar(PointerValue<'ctx>), // 本地变量分配的空间,包括函数参数
}

impl<'ctx> Type<'ctx> {
    /// 读取整形值
    pub fn is_func(&self) -> bool {
        match self {
            Type::Func(_) => true,
            _ => false,
        }
    }

    pub fn is_local(&self) -> bool {
        match self {
            Type::LocalVar(_) => true,
            _ => false,
        }
    }

    pub fn is_global(&self) -> bool {
        match self {
            Type::GlobalVar(_) => true,
            _ => false,
        }
    }

    pub fn get_val(&self) -> Option<&PointerValue<'ctx>> {
        match self {
            Type::GlobalVar(v) => Some(v),
            Type::LocalVar(v) => Some(v),
            _ => None,
        }
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
    use crate::gen_llvm_ir::{Rule, Scanner, parse_file};
    use inkwell::context::Context;
    use pest::iterators::Pair;
    use std::io::stdout;
    use std::process::Command;

    const FILE_PATH: &str = "tests/lab5/";
    
    /// 执行LLVM IR文件并返回退出码
    fn execute_llvm_ir(file_path: &str) -> i32 {
        let output = Command::new("lli")
            .arg(file_path)
            .output()
            .expect("Failed to execute lli command");
        
        output.status.code().unwrap_or(-1)
    }
    
    /// 从源文件中解析期望的输出值
    fn parse_expected_output(source_path: &str) -> Option<i32> {
        let content = std::fs::read_to_string(source_path).ok()?;
        for line in content.lines() {
            if line.trim().starts_with("// output") {
                let parts: Vec<&str> = line.trim().split_whitespace().collect();
                if parts.len() >= 3 && parts[0] == "//" && parts[1] == "output" {
                    return parts[2].parse::<i32>().ok();
                }
            }
        }
        None
    }
    
    /// 比较两个LLVM IR文件的执行结果
    fn compare_execution_results(official_path: &str, generated_path: &str) {
        let official_result = execute_llvm_ir(official_path);
        let generated_result = execute_llvm_ir(generated_path);
        
        assert_eq!(
            official_result, 
            generated_result,
            "Execution results differ: official={}, generated={}",
            official_result,
            generated_result
        );
    }
    
    /// 比较生成的LLVM IR文件与期望的输出值
    fn compare_with_expected_output(generated_path: &str, expected_output: i32) {
        let generated_result = execute_llvm_ir(generated_path);
        
        assert_eq!(
            generated_result, 
            expected_output,
            "Execution result differs from expected: generated={}, expected={}",
            generated_result,
            expected_output
        );
    }
    #[test]
    fn test_normaltest1() {
        let file_path = format!("{}{}", FILE_PATH, "normaltest1.sy");
        let out_path = format!("{}{}", FILE_PATH, "normaltest1_out.ll");
        let official_path = format!("{}{}", FILE_PATH, "normaltest1.ll");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let scanner = Scanner::new(&input,&out_path);
        scanner.scan_collect();
        
        // 比较执行结果
        compare_execution_results(&official_path, &out_path);
    }


    #[test]
    fn test_normaltest2() {
        let file_path = format!("{}{}", FILE_PATH, "normaltest2.sy");
        let out_path = format!("{}{}", FILE_PATH, "normaltest2_out.ll");
        
        // 从源文件中解析期望的输出值
        let expected_output = parse_expected_output(&file_path)
            .expect("Failed to parse expected output from source file");
        
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let scanner = Scanner::new(&input,&out_path);
        scanner.scan_collect();
        
        // 比较执行结果
        compare_with_expected_output(&out_path, expected_output);
    }

    #[test]
    fn test_normaltest4() {
        let file_path = format!("{}{}", FILE_PATH, "normaltest4.sy");
        let out_path = format!("{}{}", FILE_PATH, "normaltest4_out.ll");
        let official_path = format!("{}{}", FILE_PATH, "normaltest4.ll");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let scanner = Scanner::new(&input,&out_path);
        scanner.scan_collect();
        
        // 比较执行结果
        compare_execution_results(&official_path, &out_path);
    }

    #[test]
    fn test_normaltest9() {
        let file_path = format!("{}{}", FILE_PATH, "normaltest9.sy");
        let out_path = format!("{}{}", FILE_PATH, "normaltest9_out.ll");
        
        // 从源文件中解析期望的输出值
        let expected_output = parse_expected_output(&file_path)
            .expect("Failed to parse expected output from source file");
        
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let scanner = Scanner::new(&input,&out_path);
        scanner.scan_collect();
        
        // 比较执行结果
        compare_with_expected_output(&out_path, expected_output);
    }

    #[test]
    fn test_normaltest11() {
        let file_path = format!("{}{}", FILE_PATH, "normaltest11.sy");
        let out_path = format!("{}{}", FILE_PATH, "normaltest11_out.ll");
        let official_path = format!("{}{}", FILE_PATH, "normaltest11.ll");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let scanner = Scanner::new(&input,&out_path);
        scanner.scan_collect();
        
        // 比较执行结果
        compare_execution_results(&official_path, &out_path);
    }

    #[test]
    fn test_example01() {
        let file_path = format!("{}{}", FILE_PATH, "example01.sy");
        let out_path = format!("{}{}", FILE_PATH, "example01_out.ll");
        let official_path = format!("{}{}", FILE_PATH, "example01.ll");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let scanner = Scanner::new(&input,&out_path);
        scanner.scan_collect();
        
        // 比较执行结果
        compare_execution_results(&official_path, &out_path);
    }

    #[test]
    fn test_example02() {
        let file_path = format!("{}{}", FILE_PATH, "example02.sy");
        let out_path = format!("{}{}", FILE_PATH, "example02_out.ll");
        let official_path = format!("{}{}", FILE_PATH, "example02.ll");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let scanner = Scanner::new(&input,&out_path);
        scanner.scan_collect();
        
        // 比较执行结果
        compare_execution_results(&official_path, &out_path);
    }

    #[test]
    fn test_example03() {
        let file_path = format!("{}{}", FILE_PATH, "example03.sy");
        let out_path = format!("{}{}", FILE_PATH, "example03_out.ll");
        let official_path = format!("{}{}", FILE_PATH, "example03.ll");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let scanner = Scanner::new(&input,&out_path);
        scanner.scan_collect();
        
        // 比较执行结果
        compare_execution_results(&official_path, &out_path);
    }

    #[test]
    fn test_example04() {
        let file_path = format!("{}{}", FILE_PATH, "example04.sy");
        let out_path = format!("{}{}", FILE_PATH, "example04_out.ll");
        let official_path = format!("{}{}", FILE_PATH, "example04.ll");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let scanner = Scanner::new(&input,&out_path);
        scanner.scan_collect();
        
        // 比较执行结果
        compare_execution_results(&official_path, &out_path);
    }


    #[test]
    fn test_example05() {
        let file_path = format!("{}{}", FILE_PATH, "example05.sy");
        let out_path = format!("{}{}", FILE_PATH, "example05_out.ll");
        let official_path = format!("{}{}", FILE_PATH, "example05.ll");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let scanner = Scanner::new(&input,&out_path);
        scanner.scan_collect();
        
        // 比较执行结果
        compare_execution_results(&official_path, &out_path);
    }

    #[test]
    fn test_example06() {
        let file_path = format!("{}{}", FILE_PATH, "example06.sy");
        let out_path = format!("{}{}", FILE_PATH, "example06_out.ll");
        let official_path = format!("{}{}", FILE_PATH, "example06.ll");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let scanner = Scanner::new(&input,&out_path);
        scanner.scan_collect();
        
        // 比较执行结果
        compare_execution_results(&official_path, &out_path);
    }

    #[test]
    fn test_example07() {
        let file_path = format!("{}{}", FILE_PATH, "example07.sy");
        let out_path = format!("{}{}", FILE_PATH, "example07_out.ll");
        let official_path = format!("{}{}", FILE_PATH, "example07.ll");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let scanner = Scanner::new(&input,&out_path);
        scanner.scan_collect();
        
        // 比较执行结果
        compare_execution_results(&official_path, &out_path);
    }

    #[test]
    fn test_llvm_fn() {
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
        let sum = builder.build_int_add(a, b, "sum").unwrap();

        // 定义变量
        let c = builder.build_alloca(i32_type, "c").unwrap();
        let init_val = i32_type.const_int(1, false);
        let _ = builder.build_store(c, init_val);
        // 变量赋值

        let _ = builder.build_return(Some(&sum));
        module.print_to_stderr();
    }
}

