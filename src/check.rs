//! SysY语言的语义检查器
//!
//! 该模块实现了语义分析，包括：
//! - 变量和函数声明检查
//! - 表达式和赋值的类型检查
//! - 作用域管理和可见性规则
//! - 函数调用参数验证
//! - 数组访问验证

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
    scope_stack: ScopeStack,
    /// 错误收集
    errors: Vec<SemanticError<'a>>,
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
            errors: Vec::new(),
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
            // dbg!(&declarations);
            // 依次解析声明
            for declaration in declarations {
                // 声明解析
                self.analyze_declaration(declaration);
            }
            // dbg!(self.scope_stack.get_current_scope());
            // 生成报错信息，并输出
            self.generate_error_output()?;
        } else {
            writeln!(self.writer, "Syntax error")?;
        }
        Ok(())
    }

    pub fn generate_error_output(&mut self) -> io::Result<()> {
        for error in self.errors.iter_mut() {
            error.build_str();
            self.output += &error.output;
            self.output += "\n";
        }
        write!(self.writer, "{}", self.output)?;
        Ok(())
    }

    pub fn analyze_declaration(&mut self, declaration: Pair<'a, Rule>) {
        match declaration.as_rule() {
            Rule::Decl => {
                let decls = declaration.into_inner();
                for decl in decls {
                    self.analyze_declaration(decl);
                }
            }
            Rule::ConstDecl | Rule::VarDecl => {
                self.analyze_var_decl(declaration);
            }
            Rule::FuncDef => {
                self.analyze_func_def(declaration);
            }
            _ => {}
        }
    }
    /// 获取目标的规则的迭代器
    fn skip_in(pair: Pair<Rule>) -> impl Iterator<Item = Pair<Rule>> {
        pair.into_inner().into_iter()
    }

    /// 语义检查变量声明
    pub fn analyze_var_decl(&mut self, var_decl: Pair<'a, Rule>) {
        let mut decl_iter = Self::skip_in(var_decl);
        let mut var_defs = Vec::<Pair<'_, Rule>>::new();
        while let Some(var_def) = decl_iter.next() {
            if var_def.as_rule() == Rule::VarDef {
                var_defs.push(var_def);
            }
        }
        for var_def in var_defs {
            self.analyze_def(var_def);
        }
    }
    /// 函数定义解析
    fn analyze_func_def(&mut self, func_def: Pair<'a, Rule>) {
        let mut func_def_iter = Self::skip_in(func_def);
        let func_type = Self::skip_in(func_def_iter.next().unwrap()).next().unwrap();
        let func_name = func_def_iter.next().unwrap();
        let ident_name = func_name.as_str().to_string();
        let line_no = Self::get_line_no(func_name);
        //校验函数是否重复定义
        let is_redefined = self.check_same_ident(&ident_name, line_no, 1);
        if is_redefined {
            return;
        }

        // 将函数类型加入到最近活跃作用域的符号表中
        let func_ty = if func_type.as_rule() == Rule::Int {
            Type::Int
        } else {
            Type::Void
        };
        let function = Func::new(Vec::new(), func_ty);
        let scope = self.scope_stack.get_current_scope_mut().unwrap();
        scope.insert(ident_name.clone(), Type::Func(function));

        // 将函数更新为最近的活跃作用域
        self.scope_stack.push(ScopeKey::Ident(ident_name));
        // 解析函数参数
        let mut func_def_iter = func_def_iter.skip(1);
        let params = func_def_iter.next().unwrap();
        let mut func_body = None;
        if params.as_rule() == Rule::FuncFParams {
            // 函数参数解析
            self.analyze_func_params(params);
            func_body = func_def_iter.skip(1).next();
        } else {
            func_body = func_def_iter.next();
        }
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
            self.analyze_block_item(block_item);
        }
        // 处理完整个块之后弹出作用域
        self.scope_stack.pop();
    }

    /// 语义检查块中的每一项
    fn analyze_block_item(&mut self, block_item: Pair<'a, Rule>) {
        match block_item.as_rule() {
            Rule::Decl => {
                self.analyze_declaration(block_item);
            }
            Rule::Stmt => {
                self.analyze_stmt(block_item);
            }
            Rule::BlockItem => {
                let inner_items = Self::skip_in(block_item);
                for item in inner_items {
                    self.analyze_block_item(item);
                }
            }
            _ => {}
        }
    }

    pub fn analyze_stmt(&mut self, stmt: Pair<'a, Rule>) {
        let stmt_iter = Self::skip_in(stmt);
        for p in stmt_iter {
            match p.as_rule() {
                Rule::AssignStmt => {
                    self.analyze_assign_stmt(p);
                }
                Rule::IfStmt => {
                    self.analyze_if_stmt(p);
                }
                Rule::WhileStmt => {
                    self.analyze_while_stmt(p);
                }
                Rule::ReturnStmt => {
                    self.analyze_return_stmt(p);
                }
                Rule::ExpStmt => {
                    let exp = Self::skip_in(p).next().unwrap();
                    self.analyze_exp(exp);
                }
                Rule::Block => {
                    // 更新作用域
                    self.scope_stack.push(ScopeKey::InnerBlock);
                    let mut block_items = Vec::<Pair<'_, Rule>>::new();
                    let mut inner_items = Self::skip_in(p);
                    while let Some(ref item) = inner_items.next() {
                        if item.as_rule() == Rule::BlockItem {
                            block_items.push(item.clone());
                        }
                    }
                    // 开始处理函数体中的每一项
                    for block_item in block_items {
                        self.analyze_block_item(block_item);
                    }
                    // 处理完整个块之后弹出作用域
                    self.scope_stack.pop();
                }
                _ => {}
            }
        }
    }

    /// 处理赋值语句
    fn analyze_assign_stmt(&mut self, pair: Pair<'_, Rule>) {
        let line_no = Self::get_line_no(pair.clone());
        let stmt_str = pair.as_str();
        let mut assign = Self::skip_in(pair);
        let l_val = assign.next().unwrap();
        let mut assign = assign.skip(1);
        let r_exp = assign.next().unwrap();
        let l_type = self.analyze_lval(l_val);
        let r_type = self.analyze_exp(r_exp);
        if l_type.is_none() || r_type.is_none() {
            return;
        }
        if let (Some(l), Some(r)) = (l_type, r_type) {
            if l.is_array() && r.is_array() {
                // 判断维度是否一致
                if let (Type::Array(ll), Type::Array(rr)) = (l, r) {
                    if ll.get_dim_size() != rr.get_dim_size() {
                        // 维度不一致
                        self.collect_error(ErrorKind::TypeMismatch, line_no, stmt_str);
                    }
                }
                return;
            }
            if l.is_func() {
                // 对非变量和数组元素赋值
                self.collect_error(ErrorKind::UnexpectedFuncAssign, line_no, stmt_str);
                return;
            }
            if r.is_func() {
                // 将函数赋值给数组或变量
                self.collect_error(ErrorKind::TypeMismatch, line_no, stmt_str);
                return;
            }
            if l.is_int() && !r.is_int() {
                self.collect_error(ErrorKind::TypeMismatch, line_no, stmt_str);
                return;
            }
            if l.is_array() && r.is_int() {
                if let Type::Array(t) = l {
                    if t.get_dim_size() != 0 {
                        // 数组维度不一致
                        self.collect_error(ErrorKind::TypeMismatch, line_no, stmt_str);
                        return;
                    }
                }
                return;
            }
        }
    }

    /// 解析exp
    /// 数据处理只需要保证最终结果类型，比如：
    /// 函数调用func1() -> 返回类型
    /// 数组访问arr[0] -> 返回类型
    fn analyze_exp(&mut self, pair: Pair<'_, Rule>) -> Option<Type> {
        let mut exp = Self::skip_in(pair);
        let add_exp = exp.next().unwrap();
        self.analyze_add_exp(add_exp)
    }

    /// 解析add_exp
    fn analyze_add_exp(&mut self, pair: Pair<'_, Rule>) -> Option<Type> {
        let line_no = Self::get_line_no(pair.clone());
        let add_str = pair.as_str();
        let mut add_exp = Self::skip_in(pair);
        let l_mul_exp = add_exp.next()?;
        let next = add_exp.next();
        if next.is_none() {
            self.analyze_mul_exp(l_mul_exp)
        } else {
            let r_mul_exp = add_exp.next().unwrap();
            let l_type = self.analyze_mul_exp(l_mul_exp)?;
            let r_type = self.analyze_mul_exp(r_mul_exp)?;
            // 当返回为数组或者函数时，表达式使用时存在使用错误
            if l_type.is_array() || l_type.is_func() {
                self.collect_error(ErrorKind::TypeMismatchOp, line_no, add_str);
                None
            } else if r_type.is_func() || r_type.is_array() {
                self.collect_error(ErrorKind::TypeMismatchOp, line_no, add_str);
                return None;
            } else if l_type.is_int() && r_type.is_int() {
                return Some(Type::Int);
            } else {
                return None;
            }
        }
    }

    /// 解析mul_exp
    fn analyze_mul_exp(&mut self, pair: Pair<'_, Rule>) -> Option<Type> {
        let line_no = Self::get_line_no(pair.clone());
        let mul_str = pair.as_str();
        let mut mul_exp = Self::skip_in(pair);
        let l_unary_exp = mul_exp.next().unwrap();
        let next = mul_exp.next();
        if next.is_none() {
            self.analyze_unary_exp(l_unary_exp)
        } else {
            let r_unary_exp = mul_exp.next().unwrap();
            let l_type = self.analyze_unary_exp(l_unary_exp)?;
            let r_type = self.analyze_unary_exp(r_unary_exp)?;
            // 当返回为数组或者函数时，表达式使用时存在使用错误
            if l_type.is_array() || l_type.is_func() {
                self.collect_error(ErrorKind::TypeMismatchOp, line_no, mul_str);
                None
            } else if r_type.is_func() || r_type.is_array() {
                self.collect_error(ErrorKind::TypeMismatchOp, line_no, mul_str);
                return None;
            } else if l_type.is_int() && r_type.is_int() {
                return Some(Type::Int);
            } else {
                return None;
            }
        }
    }

    fn analyze_unary_exp(&mut self, pair: Pair<'_, Rule>) -> Option<Type> {
        let mut unary_exp = Self::skip_in(pair);
        let inner_exp = unary_exp.next().unwrap();
        match inner_exp.as_rule() {
            Rule::CallExp => {
                let call_exp = self.analyze_call_exp(inner_exp);
                call_exp
            }
            Rule::PrimaryExp => {
                let primary_exp = self.analyze_primary_exp(inner_exp);
                primary_exp
            }
            Rule::UnaryOpExp => {
                let unary_op_exp = self.analyze_unary_op_exp(inner_exp);
                unary_op_exp
            }
            _ => None,
        }
    }

    fn analyze_unary_op_exp(&mut self, pair: Pair<'_, Rule>) -> Option<Type> {
        let line_no = Self::get_line_no(pair.clone());
        let unary_str = pair.as_str();
        let mut unary_op_exp = Self::skip_in(pair).skip(1);
        let unary_exp = unary_op_exp.next()?;
        let unary_type =self.analyze_unary_exp(unary_exp)?;
        if unary_type.is_int() {
            return Some(Type::Int);
        }else {
            self.collect_error(ErrorKind::TypeMismatchOp, line_no, unary_str);
        }
        None
    }

    fn analyze_call_exp(&mut self, pair: Pair<'_, Rule>) -> Option<Type> {
        let line_no = Self::get_line_no(pair.clone());
        let mut call_exp = Self::skip_in(pair);
        let func_name = call_exp.next()?.as_str();
        // 校验函数是否已经定义
        let is_undefined = self.check_undefine(func_name, line_no, 1);
        if is_undefined {
            return None;
        }
        let mut call_exp = call_exp.skip(1);
        let params = call_exp.next()?;
        if params.as_rule() == Rule::FuncRParams {
            // 函数调用时存在参数， 需要校验参数个数、参数类型
            let mut params_iter = Self::skip_in(params);
            // 收集调用参数
            let mut params_vec = vec![];
            while let Some(param) = params_iter.next() {
                if param.as_rule() == Rule::Exp {
                    let param_type = self.analyze_exp(param);
                    if let Some(pty) = param_type {
                        params_vec.push(pty);
                    }
                    continue;
                }
            }
            // 开始校验参数个数是否一致
            let function = self.scope_stack.get(func_name)?;
            if let Type::Func(func) = function {
                if params_vec.len() != func.params.len() {
                    // 参数个数不一致 需要添加类型8错误
                    self.collect_error(ErrorKind::Inappropriate, line_no, func_name);
                    return None;
                } else {
                    let func_params = func.params.clone();
                    for (i, param) in func_params.iter().enumerate() {
                        let use_param = params_vec.get(i).unwrap();
                        let same_flag = (use_param.is_int() && param.is_int())
                            || (use_param.is_array() && param.is_array());
                        if !same_flag {
                            // 参数类型不一致 需要添加类型8错误
                            self.collect_error(ErrorKind::Inappropriate, line_no, func_name);
                            return None;
                        }
                        if use_param.is_array() && param.is_array() {
                            if let Type::Array(up) = use_param
                                && let Type::Array(p) = param
                            {
                                if up.get_dim_size() != p.get_dim_size() {
                                    self.collect_error(
                                        ErrorKind::Inappropriate,
                                        line_no,
                                        func_name,
                                    );
                                    return None;
                                }
                            }
                        }
                    }
                }
                let return_ty = func.return_type.as_ref().clone();
                return Some(return_ty); // 返回函数的返回值类型
            }else {
                self.collect_error(ErrorKind::UnlegalFuncCall, line_no, func_name);
            }
        } else {
            // 函数调用时没有传参数
            let function = self.scope_stack.get(func_name)?;
            if let Type::Func(func) = function {
                if func.params.len() != 0 {
                    self.collect_error(ErrorKind::Inappropriate, line_no, func_name);
                    return None;
                }
                let return_ty = func.return_type.as_ref().clone();
                return Some(return_ty); // 返回函数的返回值类型
            }else {
                self.collect_error(ErrorKind::UnlegalFuncCall, line_no, func_name);
            }
        }
        None
    }

    fn analyze_primary_exp(&mut self, pair: Pair<'_, Rule>) -> Option<Type> {
        let mut p_exp_iter = Self::skip_in(pair);
        let next = p_exp_iter.next().unwrap();
        if next.as_rule() == Rule::LVal {
            return self.analyze_lval(next);
        }
        if next.as_rule() == Rule::Number {
            return self.analyze_number();
        }
        let exp = p_exp_iter.next().unwrap();
        self.analyze_exp(exp)
    }

    fn analyze_number(&mut self) -> Option<Type> {
        Some(Type::Int)
    }

    ///
    fn analyze_lval(&mut self, pair: Pair<'_, Rule>) -> Option<Type> {
        let line_no = Self::get_line_no(pair.clone());
        let mut lval_iter = Self::skip_in(pair);
        let ident = lval_iter.next().unwrap();
        let ident_str = ident.as_str();
        let undefined = self.check_undefine(ident_str, line_no, 0);
        if undefined {
            return None;
        }
        let next = lval_iter.next();
        let val = self.scope_stack.get(ident_str);
        if next.is_none()
            && let Some(ref v) = val
        {
            // 说明没有用数组操作符，直接返回使用元素的类型
            return Some(v.clone());
        }
        if next.is_some()
            && let Some(v) = val
        {
            if !v.is_array() {
                // 对非数组类型使用数组操作符
                self.collect_error(ErrorKind::NotArrayAssign, line_no, ident_str);
                return None;
            }
            let mut items = vec![];
            while let Some(r) = lval_iter.next() {
                if r.as_rule() == Rule::Exp {
                    let exp_ty = self.analyze_exp(r).unwrap();
                    items.push(exp_ty);
                    continue;
                }
            }
            // 数组类型 需要计算使用维度，如果使用维度等于定义维度则返回Int 否则返回一个使用维度的数组类型
            if let Type::Array(ty) = v {
                let dims = ty.get_dim_size() - items.len();
                return if dims == 0 {
                    Some(Type::Int)
                } else {
                    let old_type = ty.item_type.as_ref().clone();
                    let mut arr = ArrayStruct::new(old_type);
                    for _ in 0..dims {
                        arr.insert_dim(0);
                    }
                    // 返回一个使用维度的数组类型
                    Some(Type::Array(arr))
                };
            }
        }
        None
    }

    /// 解析条件语句
    fn analyze_if_stmt(&mut self, pair: Pair<'a, Rule>) {
        let mut if_stmt_iter = Self::skip_in(pair).skip(2);
        let cond = if_stmt_iter.next().unwrap();
        // 解析条件
        self.analyze_cond(cond);
        let mut if_stmt_iter = if_stmt_iter.skip(1);
        let stmt = if_stmt_iter.next().unwrap();
        self.analyze_stmt(stmt);
        let next = if_stmt_iter.next();
        if next.is_none() {
            return;
        }else {
            let next_stmt = if_stmt_iter.next().unwrap();
            self.analyze_stmt(next_stmt);
        }
    }

    /// 解析条件
    fn analyze_cond(&mut self, pair: Pair<'a, Rule>) {
        let mut cond_iter = Self::skip_in(pair);
        let l_or_exp = cond_iter.next().unwrap();
        self.analyze_lor_exp(l_or_exp);
    }

    fn analyze_lor_exp(&mut self, pair: Pair<'a, Rule>) {
        let line_no = Self::get_line_no(pair.clone());
        let lor_str = pair.as_str();
        let mut lor_exp_iter = Self::skip_in(pair);
        let mut l_and_exp_exps = Vec::<Type>::new();
        while let Some(l_and_exp) = lor_exp_iter.next() {
            if l_and_exp.as_rule() == Rule::LAndExp {
                let l_and_exp_ty = self.analyze_l_and_exp(l_and_exp);
                if l_and_exp_ty.is_none() {
                    return;
                }
                l_and_exp_exps.push(l_and_exp_ty.unwrap());
            }
        }
        // 判断类型是否存在数组或者函数，存在则添加错误
        for ty in l_and_exp_exps.iter() {
            if ty.is_array() || ty.is_func() {
                self.collect_error(ErrorKind::TypeMismatchOp, line_no, lor_str);
                return;
            }
        }

    }

    fn analyze_l_and_exp(&mut self, pair: Pair<'a, Rule>) -> Option<Type>{
        let line_no = Self::get_line_no(pair.clone());
        let lor_str = pair.as_str();
        let mut l_and_exp_iter = Self::skip_in(pair);
        let mut eq_exps = Vec::<Type>::new();
        while let Some(eq_exp) = l_and_exp_iter.next() {
            if eq_exp.as_rule() == Rule::EqExp {
                let eq_exp_ty = self.analyze_eq_exp(eq_exp);
                if eq_exp_ty.is_none() {
                    return None;
                }
                eq_exps.push(eq_exp_ty.unwrap());
            }
        }
        for ty in eq_exps.iter() {
            if ty.is_array() || ty.is_func() {
                self.collect_error(ErrorKind::TypeMismatchOp, line_no, lor_str);
                return None;
            }
        }
        Some(Type::Int)
    }

    fn analyze_eq_exp(&mut self, pair: Pair<'a, Rule>) -> Option<Type> {
        let line_no = Self::get_line_no(pair.clone());
        let lor_str = pair.as_str();
        let mut eq_exp_iter = Self::skip_in(pair);
        let mut rel_exps = Vec::<Type>::new();
        while let Some(rel_exp) = eq_exp_iter.next() {
            if rel_exp.as_rule() == Rule::RelExp {
                let rel_exp_ty = self.analyze_rel_exp(rel_exp);
                if rel_exp_ty.is_none() {
                    return None;
                }
                rel_exps.push(rel_exp_ty.unwrap());
            }
        }
        for ty in rel_exps.iter() {
            if ty.is_array() || ty.is_func() {
                self.collect_error(ErrorKind::TypeMismatchOp, line_no, lor_str);
                return None;
            }
        }
        Some(Type::Int)
    }

    fn analyze_rel_exp(&mut self, pair: Pair<'a, Rule>) -> Option<Type> {
        let line_no = Self::get_line_no(pair.clone());
        let lor_str = pair.as_str();
        let mut rel_exp_iter = Self::skip_in(pair);
        let mut add_exps = Vec::<Type>::new();
        while let Some(add_exp) = rel_exp_iter.next() {
            if add_exp.as_rule() == Rule::AddExp {
                let add_exp_ty = self.analyze_add_exp(add_exp);
                if add_exp_ty.is_none() {
                    return None;
                }
                add_exps.push(add_exp_ty.unwrap());
            }
        }
        for ty in add_exps.iter() {
            if ty.is_array() || ty.is_func() {
                self.collect_error(ErrorKind::TypeMismatchOp, line_no, lor_str);
                return None;
            }
        }
        Some(Type::Int)
    }

    /// 解析循环语句
    fn analyze_while_stmt(&mut self, pair: Pair<'a, Rule>) {
        let mut while_stmt_iter = Self::skip_in(pair).skip(2);
        let cond = while_stmt_iter.next().unwrap();
        self.analyze_cond(cond);
        let mut while_stmt_iter = while_stmt_iter.skip(1);
        let stmt = while_stmt_iter.next().unwrap();
        self.analyze_stmt(stmt);
    }

    /// 返回语句
    fn analyze_return_stmt(&mut self, pair: Pair<'a, Rule>) {
        let line_no = Self::get_line_no(pair.clone());
        let return_stmt_str = pair.as_str();
        let mut return_stmt_iter = Self::skip_in(pair).skip(1);
        let next = return_stmt_iter.next().unwrap();
        if next.as_rule() == Rule::Exp {
            let exp_ty =self.analyze_exp(next);
            if exp_ty.is_none() {
                return;
            }
            let ident = self.scope_stack.get_current_scope_key();
            // if ident.is_none() {
            //     self.collect_error(ErrorKind::ReturnMismatch, line_no, return_stmt_str);
            //     return;
            // }
            let func = self.scope_stack.get(&ident.unwrap()).unwrap();
            if let Type::Func(ty) = func && !ty.return_type.is_same_type(&exp_ty.unwrap())   {
                self.collect_error(ErrorKind::ReturnMismatch, line_no, return_stmt_str);
            }
        }else {
            let ident = self.scope_stack.get_current_scope_key();
            let func = self.scope_stack.get(&ident.unwrap()).unwrap();
            if let Type::Func(ty) = func && !ty.return_type.is_same_type(&Type::Void)   {
                self.collect_error(ErrorKind::ReturnMismatch, line_no, return_stmt_str);
            }
        }
    }

    /// 解析函数定义的参数列表
    fn analyze_func_params(&mut self, params: Pair<'_, Rule>) {
        let mut params_iter = Self::skip_in(params);
        let mut params = Vec::<Pair<'_, Rule>>::new();
        while let Some(param) = params_iter.next() {
            if param.as_rule() == Rule::FuncFParam {
                params.push(param);
            }
        }
        for param in params {
            self.analyze_func_param(param);
        }
    }

    /// 语义检查函数定义的单个参数
    fn analyze_func_param(&mut self, param: Pair<'_, Rule>) {
        let mut param_iter = Self::skip_in(param).skip(1);
        let ident = param_iter.next().unwrap();
        let next_pair = param_iter.next();
        let ident_name = ident.as_str().to_string();
        let line_no = Self::get_line_no(ident);
        // 校验当前作用域是否已经存在该变量了
        let is_redefined = self.check_same_ident(&ident_name, line_no, 0);
        if is_redefined {
            return;
        }
        if next_pair.is_some() {
            //参数为数组类型，分为两类：1、单维数组 2、多维数组
            let mut arr = ArrayStruct::new(Type::Int);
            arr.insert_dim(0); // 默认是单维
            while let Some(next) = param_iter.next() {
                //多维
                if next.as_rule() == Rule::IntArr {
                    let exp = Self::skip_in(next).skip(1).next().unwrap().as_str();
                    arr.insert_dim(exp.parse::<usize>().unwrap());
                    continue;
                }
            }
            // 插入当前作用域的符号表中
            let current_scope = self.scope_stack.get_current_scope_mut().unwrap();
            current_scope.insert(ident_name, Type::Array(arr.clone()));
            // 更新函数结构体中的参数列表
            let func_key = self.scope_stack.get_current_scope_key().unwrap();
            let func = self.scope_stack.get(&func_key).unwrap();
            if let Type::Func(mut function) = func {
                function.params.push(Type::Array(arr));
                let global_scope = self.scope_stack.get_scope_mut(&ScopeKey::Global).unwrap();
                global_scope.insert(func_key, Type::Func(function));
            }
        }else {
            let current_scope = self.scope_stack.get_current_scope_mut().unwrap();
            current_scope.insert(ident_name, Type::Int);
            let func_key = self.scope_stack.get_current_scope_key().unwrap();
            let func = self.scope_stack.get(&func_key).unwrap();
            if let Type::Func(mut function) = func {
                function.params.push(Type::Int);
                let global_scope = self.scope_stack.get_scope_mut(&ScopeKey::Global).unwrap();
                global_scope.insert(func_key, Type::Func(function));
            }
        }
    }

    /// 语义检查变量定义
    pub fn analyze_def(&mut self, var_def: Pair<'a, Rule>) {
        let mut def_iter = Self::skip_in(var_def);
        let ident = def_iter.next().unwrap();
        let ident_str = ident.as_str();
        let ident_name = ident_str.to_string();
        let line_no = Self::get_line_no(ident);
        // 重复定义校验
        let is_redefine = self.check_same_ident(ident_str, line_no, 0);
        if is_redefine {
            // 出现重复定义问题，终止遍历
            return;
        }
        let next_pair = def_iter.next();

        if next_pair.is_some() {
            let nn = next_pair.unwrap();
            if nn.as_rule() == Rule::ArrayDims {
                let mut dims = Vec::<usize>::new();
                let mut array_str = ArrayStruct::new(Type::Int);
                Self::analyze_array_dims(nn, &mut dims);
                for dim in dims {
                    array_str.insert_dim(dim);
                }
                // array定义的 赋值错误校验
                let mut def_iter = def_iter.skip(1);
                let init_val = def_iter.next();
                if let Some(val) = init_val {
                    let val_ty = self.analyze_init(val).unwrap();
                    if !val_ty.is_array() {
                        self.collect_error(ErrorKind::TypeMismatchOp, line_no, ident_str);
                        return;
                    } else {
                        // TODO 实验阶段 暂时不处理
                        // if let Type::Array(val_ty) = val_ty {
                        //     if val_ty.get_dim_size() != array_str.get_dim_size() {
                        //         self.collect_error(ErrorKind::Other, line_no, ident_str);
                        //         return;
                        //     }
                        // }
                    }
                }
                let scope = self.scope_stack.get_current_scope_mut().unwrap();
                scope
                    .symbol_table
                    .insert(ident_name, Type::Array(array_str));
            } else {
                // int变量定义 赋值类型错误校验
                let init_val = def_iter.next();
                if let Some(val) = init_val {
                    let val_ty = self.analyze_init(val);
                    if val_ty.is_none() {
                        return;
                    }
                    if !val_ty.unwrap().is_int() {
                        self.collect_error(ErrorKind::TypeMismatchOp, line_no, ident_str);
                    } else {
                        let scope = self.scope_stack.get_current_scope_mut().unwrap();
                        scope.symbol_table.insert(ident_name, Type::Int);
                    }
                }
            }
        } else {
            let scope = self.scope_stack.get_current_scope_mut().unwrap();
            // 当下一个token为空时说明没有初始值，故不需要校验初值类型是否正确
            scope.symbol_table.insert(ident_name, Type::Int);
        }
    }

    pub fn analyze_init(&mut self, init_val: Pair<'a, Rule>) -> Option<Type> {
        match init_val.as_rule() {
            Rule::InitVal => {
                let mut val_iter = Self::skip_in(init_val);
                let mut val_rules = Vec::<Pair<'_, Rule>>::new();
                let mut init_vals = vec![];
                while let Some(val) = val_iter.next() {
                    if val.as_rule() == Rule::InitVal {
                        Self::analyze_init_val(val, &mut val_rules);
                        continue;
                    }
                    if val.as_rule() == Rule::Exp {
                        val_rules.push(val);
                    }
                }
                for val in val_rules {
                    let ty = self.analyze_exp(val);
                    if ty.is_none() {
                        return None;
                    }
                    init_vals.push(ty);
                }
                if init_vals.len() == 1 {
                    return Some(Type::Int);
                }
                let mut arr = ArrayStruct::new(Type::Int);
                for _ in init_vals {
                    arr.insert_dim(0);
                }
                Some(Type::Array(arr))
            }
            Rule::ConstInitVal => {
                let mut const_iter = Self::skip_in(init_val);
                let mut const_rules = Vec::<Pair<'_, Rule>>::new();
                let mut const_inits = vec![];
                while let Some(val) = const_iter.next() {
                    if val.as_rule() == Rule::ConstInitVal {
                        Self::analyze_init_val(val, &mut const_rules);
                        continue;
                    }
                    if val.as_rule() == Rule::ConstExp {
                        const_rules.push(val);
                    }
                }
                for const_rule in const_rules {
                    let add_exp = Self::skip_in(const_rule).next().unwrap();
                    let ty = self.analyze_add_exp(add_exp)?;
                    const_inits.push(ty);
                }
                if const_inits.len() == 1 {
                    return Some(Type::Int);
                }
                let mut arr = ArrayStruct::new(Type::Int);
                for _ in const_inits {
                    arr.insert_dim(0);
                }
                Some(Type::Array(arr))
            }
            _ => None,
        }
    }

    fn analyze_init_val(init_val: Pair<'a, Rule>, exps: &mut Vec<Pair<'a, Rule>>) {
        match init_val.as_rule() {
            Rule::InitVal => {
                let mut val_iter = Self::skip_in(init_val);
                while let Some(val) = val_iter.next() {
                    if val.as_rule() == Rule::InitVal {
                        Self::analyze_init_val(val, exps);
                        continue;
                    }
                    if val.as_rule() == Rule::Exp {
                        exps.push(val);
                        continue;
                    }
                }
            }
            Rule::ConstInitVal => {
                let mut const_val_iter = Self::skip_in(init_val);
                while let Some(const_val) = const_val_iter.next() {
                    if const_val.as_rule() == Rule::ConstInitVal {
                        Self::analyze_init_val(const_val, exps);
                        continue;
                    }
                    if const_val.as_rule() == Rule::ConstExp {
                        exps.push(const_val);
                        continue;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn analyze_array_dims(array_dims: Pair<'_, Rule>, dims: &mut Vec<usize>) {
        let inner_pairs = array_dims.into_inner();
        for inner_pair in inner_pairs {
            match inner_pair.as_rule() {
                Rule::ConstExp => {
                    let dim_size = Self::analyze_const_exp(inner_pair);
                    dims.push(dim_size);
                }
                _ => {}
            }
        }
    }

    pub fn analyze_const_exp(const_exp: Pair<'_, Rule>) -> usize {
        const_exp.as_str().trim().parse::<usize>().unwrap()
    }

    /// 判断在同一个作用域下，是否存在同名的标识符
    /// op -> 0 当前正在定义变量
    /// op -> 1 当前正在定义函数
    pub fn check_same_ident(&mut self, ident: &str, line_no: usize, op: usize) -> bool {
        let scope_option = self.scope_stack.get_current_scope();
        if let Some(scope) = scope_option {
            if scope.symbol_table.contains_key(ident) {
                // 判断符号类型
                if op == 1 {
                    self.collect_error(ErrorKind::RedefineFunc, line_no, ident);
                    return true;
                }
                if op == 0 {
                    self.collect_error(ErrorKind::RedefineVal, line_no, ident);
                    return true;
                }
            }
        }
        false
    }

    /// 校验函数、变量、数组在使用时是否已经定义
    fn check_undefine(&mut self, ident: &str, line_no: usize, op: usize) -> bool {
        if !self.scope_stack.contains(ident) {
            // 判断符号类型
            if op == 1 {
                self.collect_error(ErrorKind::UndefinedFunc, line_no, ident);
                return true;
            }
            if op == 0 {
                self.collect_error(ErrorKind::UndefinedVal, line_no, ident);
                return true;
            }
        }
        false
    }

    /// 收集错误信息
    fn collect_error(&mut self, error_kind: ErrorKind, line_no: usize, tips: &str) {
        let checker_error = CheckError::new(error_kind, Some(tips.to_string()));
        let semantic_error = SemanticError::new(line_no.to_string(), Some(checker_error));
        self.errors.push(semantic_error);
    }

    /// 获取行号
    fn get_line_no(pair: Pair<'_, Rule>) -> usize {
        pair.line_col().0
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum ScopeKey {
    Global,
    Ident(String),
    InnerBlock,
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
    Int,
    Func(Func),
    Void,
    Array(ArrayStruct),
}

impl Type {
    pub fn is_func(&self) -> bool {
        match self {
            Type::Func(_) => true,
            _ => false,
        }
    }
    pub fn is_array(&self) -> bool {
        match self {
            Type::Array(_) => true,
            _ => false,
        }
    }
    pub fn is_int(&self) -> bool {
        match self {
            Type::Int => true,
            _ => false,
        }
    }

    pub fn is_void(&self) -> bool {
        match self {
            Type::Void => true,
            _ => false,
        }
    }

    fn is_same_type(&self, other: &Self) -> bool {
        (self.is_int() && other.is_int()) ||
            (self.is_array() && other.is_array()) ||
            (self.is_func() && other.is_func()) ||
            (self.is_void() && other.is_void())
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

/// 数组结果表示
/// 维度和类型
#[derive(Debug, Clone)]
pub struct ArrayStruct {
    // 数组元素类型
    pub item_type: Box<Type>,
    // 维度标识数组
    pub dim_nos: Vec<usize>,
    // 每个维度的存储空间
    pub tpl_size: HashMap<usize, usize>,
    // 数组维度
    pub array_dims: usize,
    // 维度流水号
    pub crr_no: usize,
}

impl ArrayStruct {
    fn new(item_type: Type) -> Self {
        Self {
            item_type: Box::new(item_type),
            dim_nos: Vec::new(),
            tpl_size: Default::default(),
            array_dims: 0,
            crr_no: 0,
        }
    }
    fn insert_dim(&mut self, dim_size: usize) {
        self.dim_nos.push(self.crr_no);
        self.tpl_size.insert(self.crr_no, dim_size);
        self.array_dims += 1;
        self.crr_no += 1;
    }

    fn get_dim_size(&self) -> usize {
        self.array_dims
    }

    fn get_item_type(&self) -> &Type {
        &self.item_type
    }
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
            ErrorKind::TypeMismatch => {
                CheckError::TypeMismatch(kind, "Type mismatched for assignment", error_tip)
            }
            ErrorKind::TypeMismatchOp => {
                CheckError::TypeMismatchOp(kind, "Type mismatched for op", error_tip)
            }
            ErrorKind::ReturnMismatch => {
                CheckError::ReturnMismatch(kind, "Type mismatched for return", error_tip)
            }
            ErrorKind::Inappropriate => CheckError::Inappropriate(
                kind,
                "Function is not applicable for arguments",
                error_tip,
            ),
            ErrorKind::NotArrayAssign => {
                CheckError::NotArrayAssign(kind, "Not an array", error_tip)
            }
            ErrorKind::UnlegalFuncCall => {
                CheckError::UnlegalFuncCall(kind, "Not a function", error_tip)
            }
            ErrorKind::UnexpectedFuncAssign => CheckError::UnexpectedFuncAssign(
                kind,
                "The left-hand side of an assignment must be a variable",
                error_tip,
            ),
            _ => CheckError::Other(kind, "other", error_tip),
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
            format!("{} -> {}", self.get_msg(), t)
        } else {
            format!("{}", self.get_msg())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::check::{Checker, Rule, parse_file};
    use pest::iterators::Pair;
    use std::io::stdout;

    const FILE_PATH: &str = "tests/semantic/";
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
    fn test_semantic_test() {
        let filename = FILE_PATH.to_string() + "test.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut checker = Checker::new(&file, &mut binding);
        checker.syn_check().unwrap();
    }

    #[test]
    fn test_semantic_example() {
        // 1、把内容输出内存缓冲区
        let mut buf = stdout();
        let filename = FILE_PATH.to_string() + "example01.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut checker = Checker::new(&file, &mut buf);
        checker.syn_check().unwrap();
    }

    #[test]
    fn test_semantic_undefined_variable() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        // let mut buf = stdout();
        let filename = FILE_PATH.to_string() + "normaltest01.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut checker = Checker::new(&file, &mut buf);
        checker.syn_check().unwrap();
        let actual = String::from_utf8(buf).unwrap();
        let expected_filename = FILE_PATH.to_string() + "normaltest01.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_semantic_undefined_function() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        // let mut buf = stdout();
        let filename = FILE_PATH.to_string() + "normaltest02.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut checker = Checker::new(&file, &mut buf);
        checker.syn_check().unwrap();
        let actual = String::from_utf8(buf).unwrap();
        let expected_filename = FILE_PATH.to_string() + "normaltest02.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_semantic_redefined_variable() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        // let mut buf = stdout();
        let filename = FILE_PATH.to_string() + "normaltest03.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut checker = Checker::new(&file, &mut buf);
        checker.syn_check().unwrap();
        let actual = String::from_utf8(buf).unwrap();
        let expected_filename = FILE_PATH.to_string() + "normaltest03.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_semantic_redefined_function() {
        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "normaltest04.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut checker = Checker::new(&file, &mut buf);
        checker.syn_check().unwrap();
        let actual = String::from_utf8(buf).unwrap();
        let expected_filename = FILE_PATH.to_string() + "normaltest04.out";
        let expected = std::fs::read_to_string(expected_filename).expect("Failed to read file");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_semantic_type_mismatched_assignment() {
        // 1、把内容输出内存缓冲区
        let mut buf = stdout();
        let filename = FILE_PATH.to_string() + "normaltest05.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut checker = Checker::new(&file, &mut buf);
        checker.syn_check().unwrap();
    }

    #[test]
    fn test_semantic_type_mismatched_operation() {
        // 1、把内容输出内存缓冲区
        let mut buf = stdout();
        let filename = FILE_PATH.to_string() + "normaltest06.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut checker = Checker::new(&file, &mut buf);
        checker.syn_check().unwrap();
    }

    #[test]
    fn test_semantic_type_mismatched_return() {
        // 1、把内容输出内存缓冲区
        let mut buf = stdout();
        let filename = FILE_PATH.to_string() + "normaltest07.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut checker = Checker::new(&file, &mut buf);
        checker.syn_check().unwrap();
    }

    #[test]
    fn test_semantic_function_not_applicable() {
        // 1、把内容输出内存缓冲区
        let mut buf = stdout();
        let filename = FILE_PATH.to_string() + "normaltest08.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut checker = Checker::new(&file, &mut buf);
        checker.syn_check().unwrap();

    }

    #[test]
    fn test_semantic_not_an_array() {
        // 1、把内容输出内存缓冲区
        let mut buf =  stdout();
        let filename = FILE_PATH.to_string() + "normaltest09.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut checker = Checker::new(&file, &mut buf);
        checker.syn_check().unwrap();
    }
    //
    #[test]
    fn test_semantic_not_a_variable() {
        // 1、把内容输出内存缓冲区
        let mut buf =  stdout();
        let filename = FILE_PATH.to_string() + "normaltest11.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut checker = Checker::new(&file, &mut buf);
        checker.syn_check().unwrap();
    }

    #[test]
    #[ignore]
    fn test_number() {
        assert_eq!("0x10".parse::<i32>().is_ok(), true)
    }

    #[test]
    #[ignore]
    fn test_skip() {
        let filename = FILE_PATH.to_string() + "test.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut checker = Checker::new(&file, &mut binding);
        checker.syn_check().unwrap();
    }
}
