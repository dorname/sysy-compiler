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
                writeln!(self.writer, "{}", self.output)?;
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
                            let mut val = VariableDel::new(inner_pair.line_col().0);
                            val.is_const = true;
                            val.var_type = Some("int".to_string());
                            self.walk_val_def(inner_pair, &mut val);
                            if !val.is_error {
                                self.variable_dels.push(val);
                            }
                        }
                        Rule::VarDef => {
                            let mut val = VariableDel::new(inner_pair.line_col().0);
                            val.is_const = false;
                            val.var_type = Some("int".to_string());
                            val.belongs_to = self.block_no.clone();
                            self.walk_val_def(inner_pair, &mut val);
                            if !val.is_error {
                                self.variable_dels.push(val);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn walk_func_def(&mut self, pair: Pair<Rule>, func_def: &mut FuncDef) {
        /// 收集函数入参
        fn walk_func_param(pair: Pair<Rule>, param: &mut FuncParam) {
            match pair.as_rule() {
                Rule::Ident => {
                    param.name = Some(pair.as_str().to_string());
                }
                Rule::NonIntArr => {
                    // -1 代表 无形状数组 []
                    param.array_dims.push("-1".to_string());
                }
                Rule::IntArr => {
                    let inner_pairs = pair.into_inner();
                    for inner_pair in inner_pairs {
                        walk_func_param(inner_pair, param);
                    }
                }
                Rule::Exp => {
                    param.array_dims.push(pair.as_str().to_string());
                }
                _ => {}
            }
        }
        match pair.as_rule() {
            Rule::FuncType => {
                func_def.return_type = Some(pair.as_str().to_string()); //收集返回类型
            }
            Rule::Ident => {
                let name = pair.as_str().to_string();
                func_def.name = Some(name.clone()); // 收集函数名称
                // 注意：函数重复定义检查已经在上层的check方法中处理了
            }
            Rule::FuncFParam => {
                let line_no = pair.line_col().0;
                let inner_pairs = pair.into_inner();
                let mut param = FuncParam::new();
                for inner_pair in inner_pairs {
                    walk_func_param(inner_pair, &mut param);
                }
                // 校验参数是否重复定义
                if func_def
                    .params
                    .iter()
                    .filter(|p| eq_option_string(&p.name, &param.name))
                    .count()
                    > 0
                {
                    // 参数重复定义错误
                    let check_result = PairCheckResult::new(
                        line_no.to_string(),
                        Some(CheckError::new(ErrorKind::RedefineVal, param.name.clone())),
                    );
                    self.check_results.push(check_result);
                } else {
                    func_def.params.push(param);
                    let _ = self.context_stack.pop();
                    if let Some(name) = &func_def.name {
                        let params = func_def.params.clone();
                        self.context_stack.push((name.clone(), Some(params)))
                    }
                }
            }
            Rule::FuncFParams |
            Rule::Block |
            Rule::BlockItem |
            Rule::Exp |
            Rule::AddExp |
            Rule::MulExp |
            Rule::UnaryExp |
            Rule::PrimaryExp |
            Rule::Cond |
            Rule::LOrExp |
            Rule::LAndExp |
            Rule::EqExp |
            Rule::RelExp |
            Rule::Stmt => {
                if pair.as_rule() == Rule::Block {
                    self.block_no += 1; // 进入一个新的函数作用域
                }
                if (pair.as_rule() == Rule::Exp||pair.as_rule()==Rule::Cond) && !self.is_assign_stmt(pair.clone()) {
                    if self.has_operator(pair.clone()) {
                        let line_no = pair.line_col().0;
                        // 检查运算符类型匹配 - 按照README要求，如果第一个操作数不匹配就直接报错
                        if !self.check_operator_type_match(pair.clone()) {
                            self.add_check_result(line_no, ErrorKind::TypeMismatchOp, Some(pair.clone().as_str().to_string()));
                        }
                    }
                }
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.walk_func_def(inner_pair, func_def);
                }
            }
            Rule::LVal => {
                let line_no = pair.line_col().0;
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    if inner_pair.as_rule() == Rule::Ident {
                        let name = inner_pair.as_str().to_string();
                        let (v_defined,_) = self.var_contains(&name);
                        let f_defined = self.func_contains(&name);
                        
                        // 只有在既不是变量也不是函数的情况下才报告未定义
                        // 如果是函数名但用于运算，会在运算符类型检查中报告错误类型6
                        if !v_defined && !f_defined {
                            self.add_check_result(line_no,ErrorKind::UndefinedVal,Some(name.clone()));
                        }
                    }
                }
            }
            Rule::Decl => {
                self.check(pair);
            }
            Rule::CallExp => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.walk_assign(inner_pair);
                }
            }
            Rule::AssignStmt => {
                let line_no = pair.line_col().0;
                let pair_str = pair.as_str();
                let mut inner_pairs = pair.into_inner();
                let ori_ident = inner_pairs.next().unwrap().as_str().trim().to_string();
                let _ = inner_pairs.next().unwrap().as_str().to_string();
                let expr_rule = inner_pairs.next().unwrap();
                let mut ident = ori_ident.clone();
                if ident.ends_with("]") {
                    ident = ident.split("[").collect::<Vec<&str>>()[0].to_string();
                }
                let (defined, v) = self.var_contains(&ident);
                let f_defined = self.func_contains(&ident);
                let defined = f_defined || defined;
                if f_defined {
                    self.add_check_result(line_no,ErrorKind::UnexpectedFuncAssign,Some(ident.clone()));
                }
                // 不考虑数组的赋值语句校验
                if !pair_str.contains("[") &&  !defined  {
                    // 变量未定义错误
                    self.add_check_result(line_no,ErrorKind::UndefinedVal,Some(ident.clone()));
                }
                if self.has_call(expr_rule.clone(),false) {
                    for expr_pair in expr_rule.clone().into_inner() {
                        self.walk_func_def(expr_pair, func_def);
                    }
                    return ;
                }
                
                // 优先检查运算符类型匹配
                if self.has_operator(expr_rule.clone()) {
                    if !self.check_operator_type_match(expr_rule.clone()) {
                        self.add_check_result(line_no, ErrorKind::TypeMismatchOp, Some(expr_rule.as_str().to_string()));
                        return;
                    }
                }
                let expr_str = expr_rule.as_str();
                if !expr_str.ends_with("]") && self.expr_is_number(expr_rule.clone()) && defined && !v{
                    return;
                }
                // e_v1 true => array
                // e_v2 false => int
                let (e_v1,e_defined,f_defined) = self.check_expr(expr_rule.clone(),1);
                let (e_v2,_,_) = self.check_expr(expr_rule.clone(),0);  //是否完全由int变量组成
                if !e_defined {
                    // 变量未定义错误
                    self.add_check_result(line_no,ErrorKind::UndefinedVal,Some(expr_rule.as_str().to_string()));
                }else {
                    if v == e_v1 && v {
                        // 如果左右都是数组
                        // 求左边数组声明的维度
                        let mut left = 0;
                        let mut left_all_size = 0;
                        if ori_ident.ends_with("]") {
                            left = ori_ident.chars().filter(|&c| c == '[').count();
                        }
                        let l_var = self.variable_dels.iter().find(|e|eq_option_string(&e.name,&Some(ident.clone())));
                        if let Some(x) = l_var {
                            left_all_size = x.array_dims.len();
                        }
                        // 求右边数组的维度
                        let mut right = 0;
                        let mut right_all_size = 0;
                        let mut exp_ident = expr_rule.as_str().to_string();
                        if expr_rule.as_str().ends_with("]") {
                            right = expr_rule.as_str().chars().filter(|&c| c == '[').count();
                            exp_ident = expr_rule.as_str().split("[").collect::<Vec<&str>>()[0].trim().to_string();
                        }
                        let r_var = self.variable_dels.iter().find(|e|eq_option_string(&e.name,&Some(exp_ident.clone())));
                        if let Some(x) = r_var {
                            right_all_size = x.array_dims.len();
                        }
                        if (left_all_size-left) !=  (right_all_size-right) {
                            self.add_check_result(line_no,ErrorKind::TypeMismatch,Some(expr_rule.as_str().to_string()));
                        }
                        return;
                    }
                    if v && v == !e_v2 {
                        // 左边是数组 右边是纯int
                        let mut left = 0;
                        let mut left_all_size = 0;
                        if ori_ident.ends_with("]") {
                            left = ori_ident.chars().filter(|&c| c == '[').count();
                        }
                        let l_var = self.variable_dels.iter().find(|e|eq_option_string(&e.name,&Some(ident.clone())));
                        if let Some(x) = l_var {
                            left_all_size = x.array_dims.len();
                        }
                        if left_all_size - left != 0 {
                            self.add_check_result(line_no,ErrorKind::TypeMismatch,Some(expr_rule.as_str().to_string()));
                            return;
                        }
                    }
                    if !v && e_v1 {
                        // 求右边数组的维度
                        let mut right = 0;
                        let mut right_all_size = 0;
                        let mut exp_ident = expr_rule.as_str().to_string();
                        if expr_rule.as_str().ends_with("]") {
                            right = expr_rule.as_str().chars().filter(|&c| c == '[').count();
                            exp_ident = expr_rule.as_str().split("[").collect::<Vec<&str>>()[0].trim().to_string();
                        }
                        let r_var =  self.variable_dels.iter().find(|e|eq_option_string(&e.name,&Some(exp_ident.clone())));
                        if let Some(x) = r_var {
                            right_all_size = x.array_dims.len();
                        }
                        if  right != right_all_size {
                            self.add_check_result(line_no,ErrorKind::TypeMismatch,Some(expr_rule.as_str().to_string()));
                            return;
                        }
                    }
                    // 左侧是整形 右侧也是整形或者函数但是增加了[ ] 变成了数组
                    if !v && (!e_v2 || f_defined ) {
                        let exp_ident = expr_rule.as_str().to_string();
                        if exp_ident.as_str().ends_with("]") {
                            self.add_check_result(line_no,ErrorKind::NotArrayAssign,Some(expr_rule.as_str().to_string()));
                            return;
                        }
                    }
                    if !e_v1 || e_v2 {
                        if v {
                            let mut left = 0;
                            let mut left_all_size = 0;
                            if ori_ident.ends_with("]") {
                                left = ori_ident.chars().filter(|&c| c == '[').count();
                            }
                            let l_var = self.variable_dels.iter().find(|e|eq_option_string(&e.name,&Some(ident.clone())));
                            if let Some(x) = l_var {
                                left_all_size = x.array_dims.len();
                            }
                            if left_all_size == left {
                                return;
                            }else{
                                // 存在不同类型的标识符进行基础运算
                                self.add_check_result(line_no,ErrorKind::TypeMismatchOp,Some(expr_rule.as_str().to_string()));
                            }
                        }else {
                            let inner_pairs = expr_rule.clone().into_inner();
                            let mut flag  = true;
                            for inner_pair in inner_pairs {
                                flag &= self.check_expr_w(inner_pair);
                            }
                            if !flag {
                                // 存在不同类型的标识符进行基础运算
                                self.add_check_result(line_no,
                                                      ErrorKind::TypeMismatchOp,
                                                      Some(expr_rule.as_str().to_string()));
                            }
                        }
                        return;
                    }
                }

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

    /// 检查运算符类型匹配 - 按照README要求，如果第一个操作数不匹配就直接报错
    fn check_operator_type_match(&mut self, pair: Pair<Rule>) -> bool {
        match pair.as_rule() {
            Rule::AddExp | Rule::MulExp | Rule::RelExp | Rule::EqExp | 
            Rule::LAndExp | Rule::LOrExp => {
                let inner_pairs: Vec<_> = pair.into_inner().collect();
                if inner_pairs.len() > 1 {
                    // 有运算符的表达式，检查第一个操作数
                    let first_operand = &inner_pairs[0];
                    return self.check_operand_is_int_type(first_operand.clone());
                }
                // 单个操作数，递归检查
                if let Some(first) = inner_pairs.first() {
                    return self.check_operator_type_match(first.clone());
                }
                true
            }
            Rule::UnaryExp => {
                let inner_pairs: Vec<_> = pair.into_inner().collect();
                if let Some(first) = inner_pairs.first() {
                    return self.check_operator_type_match(first.clone());
                }
                true
            }
            Rule::PrimaryExp | Rule::LVal | Rule::Number => {
                self.check_operand_is_int_type(pair)
            }
            Rule::CallExp => {
                // 函数调用应该返回int类型，这里暂时返回true
                // 具体的函数返回类型检查在其他地方处理
                true
            }
            _ => {
                // 其他情况递归检查
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    if !self.check_operator_type_match(inner_pair) {
                        return false;
                    }
                }
                true
            }
        }
    }

    /// 检查操作数是否为int类型（非数组、非函数）
    fn check_operand_is_int_type(&mut self, pair: Pair<Rule>) -> bool {
        match pair.as_rule() {
            Rule::Number => true, // 数字字面量是int类型
            Rule::LVal => {
                let pair_str = pair.as_str();
                if pair_str.ends_with("]") {
                    // 数组元素访问，需要检查维度
                    let ident = pair_str.split("[").collect::<Vec<&str>>()[0].to_string();
                    let (defined, is_array) = self.var_contains(&ident);
                    if !defined {
                        return false; // 变量未定义
                    }
                    if !is_array {
                        return false; // 对非数组使用下标
                    }
                    // 这里应该检查数组访问后的类型，暂时简化处理
                    true
                } else {
                    // 简单变量访问
                    let (var_defined, is_array) = self.var_contains(&pair_str);
                    let func_defined = self.func_contains(&pair_str);
                    
                    // 如果是函数名，不能用于运算
                    if func_defined {
                        return false;
                    }
                    
                    var_defined && !is_array // 必须是已定义的非数组变量
                }
            }
            Rule::CallExp => {
                // 函数调用，这里需要检查函数是否存在且返回int
                let inner_pairs: Vec<_> = pair.into_inner().collect();
                if let Some(ident_pair) = inner_pairs.first() {
                    if ident_pair.as_rule() == Rule::Ident {
                        let func_name = ident_pair.as_str();
                        return self.func_contains(func_name);
                    }
                }
                false
            }
            _ => {
                // 递归检查内部
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    if !self.check_operand_is_int_type(inner_pair) {
                        return false;
                    }
                }
                true
            }
        }
    }

    /// 检查函数调用参数是否为数组类型
    fn check_is_arr(&mut self, pair: Pair<Rule>) -> bool {
        match pair.as_rule() {
            Rule::LVal => {
                let pair_str = pair.as_str();
                if pair_str.ends_with("]") {
                    // 数组元素访问，需要计算剩余维度
                    let ident = pair_str.split("[").collect::<Vec<&str>>()[0].to_string();
                    let (defined, is_array) = self.var_contains(&ident);
                    if !defined || !is_array {
                        return false;
                    }
                    
                    // 计算访问的维度数
                    let access_dims = pair_str.chars().filter(|&c| c == '[').count();
                    
                    // 获取变量的总维度数
                    let var = self.variable_dels.iter().find(|e| eq_option_string(&e.name, &Some(ident.clone())));
                    if let Some(v) = var {
                        let total_dims = v.array_dims.len();
                        return access_dims < total_dims; // 如果访问维度小于总维度，则结果仍是数组
                    }
                    false
                } else {
                    // 简单变量访问
                    let (defined, is_array) = self.var_contains(&pair_str);
                    defined && is_array
                }
            }
            _ => {
                // 其他表达式类型，递归检查
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    if self.check_is_arr(inner_pair) {
                        return true;
                    }
                }
                false
            }
        }
    }

    /// 判断exp下面是否是赋值语句
    fn is_assign_stmt(&self,pair:Pair<Rule>) -> bool {
        match pair.as_rule() {
            Rule::AssignStmt => true,
            _ => {
                let inner_pairs = pair.into_inner();
                let mut flag = false;
                for inner_pair in inner_pairs {
                    flag |= self.is_assign_stmt(inner_pair)
                }
                flag
            }
        }
    }

    /// 递归检查Exp维度问题是否一致
    fn check_expr_w(&mut self,pair:Pair<Rule>) -> bool {
        match pair.as_rule() {
            Rule::LVal => {
                let inner_str = pair.as_str().trim().to_string();
                if inner_str.ends_with("]"){
                    let curr = inner_str.chars().filter(|&c| c == '[').count();
                    let id = inner_str.split("[").collect::<Vec<&str>>()[0].to_string();
                    let mut all = 0;
                    let var = self.variable_dels.iter().find(|e|eq_option_string(&e.name,&Some(id.clone())));
                    if let Some(x) = var {
                        all = x.array_dims.len();
                    }
                     curr == all
                }else {
                    let id = inner_str;
                    let (defined,v) = self.var_contains(&id);
                    defined && !v
                }
            }
            _ => {
                let inner_pairs = pair.into_inner();
                let mut flag = true;
                for inner_pair in inner_pairs {
                   flag &= self.check_expr_w(inner_pair);
                }
                flag
            }
        }
    }

    fn expr_is_number(&mut self,pair: Pair<Rule>) -> bool {
        match pair.as_rule() {
            Rule::Number => true,
            Rule::Ident => false,
            _ => {
                let inner_pairs = pair.into_inner();
                let mut is_number = false;
                for inner_pair in inner_pairs {
                    is_number = is_number || self.expr_is_number(inner_pair);
                }
                is_number
            }

        }
    }

    fn expr_is_arr(&mut self,pair: Pair<Rule>) -> bool {
        match pair.as_rule() {
            Rule::Ident => {
                let name = pair.as_str().to_string();
                self.var_contains(&name).1
            }
            _ => {
                let inner_pairs = pair.into_inner();
                let mut is_func = false;
                for inner_pair in inner_pairs {
                    is_func = is_func || self.expr_is_arr(inner_pair);
                }
                is_func
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


    /// 判断一个ruLe下是否有callExp
    fn has_call(&self,pair:Pair<Rule>,target:bool) -> bool {
        if pair.as_rule() == Rule::CallExp {
             true
        }else {
            let inner_pairs = pair.into_inner();
            let mut target = target;
            for inner_pair in inner_pairs {
                target = target || self.has_call(inner_pair,target);
            }
            target
        }
    }

    /// check_op 1 时
    /// 假设返回值为 x
    /// x.1 为 true 表示表达式都是定义过的标识符
    /// x.1 为 false 表示表达式存在至少一个未定义的标识符
    /// 如果 x.0 为true  => 表达式全部都是数组类型
    /// 如果 x.0 为false => 表达式中至少存在一个int类型变量
    /// check_op 0 时
    /// 如果 x.0 为false => 表达式全部都是int类型
    /// 如果 x.0 为true  => 表达式至少存在一个数组类型变量
    /// x.1 无作用
    fn check_expr(&mut self,p:Pair<Rule>,check_op:u8)->(bool,bool,bool) {
        match p.as_rule() {
            Rule::Ident => {
                let p = p.as_str().to_string();
                let (defined, v) = self.var_contains(&p);
                let f_defined = self.func_contains(&p);
                (v,defined,f_defined)
            }
            _ => {
                let inner_pairs = p.into_inner();
                let mut results = vec![];
                for inner_pair in inner_pairs {
                    results.push(self.check_expr(inner_pair,check_op));
                }
                let mut is_arr = if check_op == 1 {
                    (true, true, true)
                }else {
                    (false, false, false)
                };
                if check_op == 1 {
                    is_arr = results.iter().copied().fold(is_arr, |(v, d,f), (v1, d1,f1)| {
                        (v && v1, d && d1, f && f1)
                    });
                }else {
                    is_arr = results.iter().copied().fold(is_arr, |(v, d, f), (v1, d1,f1)| {
                        (v || v1, d && d1,f && f1)
                    });
                }
                is_arr
            }
        }
    }

    fn walk_val_def(&mut self, pair: Pair<Rule>, def: &mut VariableDel) {
        match pair.as_rule() {
            Rule::ConstDef => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.walk_val_def(inner_pair, def);
                }
            }
            Rule::VarDef => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.walk_val_def(inner_pair, def);
                }
            }
            Rule::Ident => {
                let ident = pair.as_str();
                let line_no = pair.line_col().0;
                self.check_val_redefine(ident, line_no);
                def.name = Some(ident.to_string());
            }
            Rule::ArrayDims => {
                let inner_pairs = pair.into_inner();
                for pair in inner_pairs {
                    match pair.as_rule() {
                        Rule::ConstExp => {
                            def.array_dims.push(pair.as_str().to_string());
                        }
                        _ => {}
                    }
                }
            }
            Rule::ConstInitVal | Rule::InitVal => {
                def.value = Some(pair.as_str().to_string());
                let inner_pairs = pair.into_inner();
                for p in inner_pairs {
                    self.walk_assign(p);
                }
            }
            _ => {}
        }
    }

    /// 从变量声明定义的语句中进来
    fn walk_assign(&mut self,pair: Pair<Rule>) {
        match pair.as_rule() {
            Rule::Exp |
            Rule::ConstExp |
            Rule::AddExp |
            Rule:: MulExp |
            Rule::UnaryExp |
            Rule::PrimaryExp |
            Rule::CallExp => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.walk_assign(inner_pair);
                }
            }
            Rule::LVal => {
                let line_no = pair.line_col().0;
                let pair_str = pair.as_str();
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    if inner_pair.as_rule() == Rule::Ident {
                        let name = inner_pair.as_str().to_string();
                        let (v_defined,v) = self.var_contains(&name);
                        let f_defined= self.func_contains(&name);
                        if !v_defined && !f_defined {
                            self.add_check_result(line_no, ErrorKind::UndefinedVal,Some(name.clone()));
                        }
                        if (f_defined || !v) && pair_str.ends_with("]") {
                            self.add_check_result(line_no, ErrorKind::NotArrayAssign,Some(name));
                        }
                    }
                }
            }
            Rule::Ident => {
                // let i = a();
                // 这里只可能从声明语句的CallExp的进来
                // （1）函数未定义错误：
                //  没有同一作用域级别或者上一层级的同名变量定义或者函数定义。
                // （2）非函数调用错误：
                //  存在同名变量且同名变量在所有同名函数和变量中作用域级别与当前作用域最接近，且所属层级要小于等于当前层级。
                let ident = pair.as_str();
                // 记录一下当前调用的函数名称
                self.call_func = Some(ident.to_string());
                let line_no = pair.line_col().0;
                let (no_var,_ ) = self.var_contains(&ident);
                if !self.func_contains(&ident) && !no_var {
                    if let Some((name, _)) = self.get_current_context() {
                        // 如果当前函数不是递归
                        if *name != ident.to_string() {
                            // 函数未定义
                            self.add_check_result(line_no, ErrorKind::UndefinedFunc,Some(ident.to_string()));
                            return;
                        }
                    }
                }
                if !self.func_contains(&ident) && no_var {
                    // 非函数调用错误
                    self.add_check_result(line_no, ErrorKind::UnlegalFuncCall,Some(ident.to_string()));
                    return;
                }
            }
            Rule::FuncRParams => {
                let line_no = pair.line_col().0;
                let pair_str = pair.as_str().to_string();
                
                if let Some(func_name) = &self.call_func {
                    // 收集实际参数
                    let actual_params: Vec<_> = pair.into_inner().collect();
                    
                    // 获取函数定义 (克隆函数名避免借用冲突)
                    let func_name_clone = func_name.clone();
                    let expected_params = self.func_dels.iter()
                        .find(|e| eq_option_string(&e.name, &Some(func_name_clone.clone())))
                        .map(|f| f.params.clone());
                        
                    if let Some(expected_params) = expected_params {
                        // 检查参数数量
                        if actual_params.len() != expected_params.len() {
                            self.add_check_result(line_no, ErrorKind::Inappropriate, Some(pair_str));
                            return;
                        }
                        
                        // 检查参数类型匹配
                        for (i, actual_param) in actual_params.iter().enumerate() {
                            if let Some(expected_param) = expected_params.get(i) {
                                let actual_is_array = self.check_is_arr(actual_param.clone());
                                let expected_is_array = expected_param.is_array_type();
                                
                                if actual_is_array != expected_is_array {
                                    self.add_check_result(line_no, ErrorKind::Inappropriate, Some(pair_str));
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
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
    use crate::format::Formatter;
    use std::io::stdout;
    use crate::lexer::tokenizer;

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
        let filename = FILE_PATH.to_string() + "normaltest02.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut checker = Checker::new(&file, &mut binding);
        checker.syn_check().unwrap();


        // 1、把内容输出内存缓冲区
        let mut buf = Vec::<u8>::new();
        let filename = FILE_PATH.to_string() + "simple.in";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let _ = tokenizer(&file, &mut buf);

        let mut actual = String::from_utf8(buf).unwrap();
        // 根据操作系统替换换行符
        // windows下 writeln! 生成的是 \n 但从文件中读出来的换行符是 \r\n
        actual = actual.replace("\r\n", "\n").replace('\r', "\n");
        // 再按平台输出
        if cfg!(windows) {
            actual = actual.replace('\n', "\r\n");
        }
        let expected_filename = FILE_PATH.to_string() + "simple.out";
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