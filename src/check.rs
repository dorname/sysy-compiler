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
use pest::pratt_parser::Op;

#[derive(Parser)]
#[grammar = "pests/parser.pest"]
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
                for inner_pair in inner_pairs {
                    self.walk_func_def(inner_pair, &mut func_def);
                }
                self.func_dels.push(func_def);
                self.block_no -= 1; // 离开函数作用域
                self.context_stack.pop(); // 函数出栈
                self.call_func = None; // 清楚函数调用记录
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
                let line_no = pair.line_col().0;
                if self.func_contains(&name) {
                    //函数重复定义
                    let check_result = PairCheckResult::new(
                        line_no.to_string(),
                        Some(CheckError::new(ErrorKind::RedefineFunc,Some(name.clone()))),
                    );
                    self.check_results.push(check_result);
                }
                func_def.name = Some(name.clone()); // 收集函数名称
                self.context_stack.push((name, None)); // 函数入栈
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
            Rule::Stmt => {
                if pair.as_rule() == Rule::Block {
                    self.block_no += 1; // 进入一个新的函数作用域
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
                        if !v_defined {
                            let check_result = PairCheckResult::new(
                                line_no.to_string(),
                                Some(CheckError::new(ErrorKind::UndefinedVal, Some(name.clone()))),
                            );
                            self.check_results.push(check_result);
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
                    let check_result = PairCheckResult::new(
                        line_no.to_string(),
                        Some(CheckError::new(
                            ErrorKind::UnexpectedFuncAssign,
                            Some(ident.to_string()),
                        )),
                    );
                    self.check_results.push(check_result);
                }
                // 不考虑数组的赋值语句校验
                if !pair_str.contains("[") &&  !defined  {
                    // 变量未定义错误
                    let check_result = PairCheckResult::new(
                        line_no.to_string(),
                        Some(CheckError::new(
                            ErrorKind::UndefinedVal,
                            Some(ident.to_string()),
                        )),
                    );
                    self.check_results.push(check_result);
                }
                if self.has_call(expr_rule.clone(),false) {
                    for expr_pair in expr_rule.clone().into_inner() {
                        self.walk_func_def(expr_pair, func_def);
                    }
                    return ;
                }
                let expr_str = expr_rule.as_str();
                if !expr_str.ends_with("]") && self.expr_is_number(expr_rule.clone()) && defined && !v{
                    return;
                }
                // e_v1 true => array
                // e_v2 false => int
                let (e_v1,e_defined) = self.check_expr(expr_rule.clone(),1);
                let (e_v2,_) = self.check_expr(expr_rule.clone(),0);  //是否完全由int变量组成
                if !e_defined {
                    // 变量未定义错误
                    let check_result = PairCheckResult::new(
                        line_no.to_string(),
                        Some(CheckError::new(
                            ErrorKind::UndefinedVal,
                            Some(expr_rule.as_str().to_string()),
                        )),
                    );
                    self.check_results.push(check_result);
                }else {
                    if v == e_v1 && v {
                        // 如果左右都是数组
                        // 求左边数组声明的维度
                        let mut left = 0;
                        if ori_ident.ends_with("]") {
                            left = ori_ident.chars().filter(|&c| c == '[').count();
                        }
                        let left_all_size = self.variable_dels.iter().find(|e|eq_option_string(&e.name,&Some(ident.clone()))).unwrap().array_dims.len();
                        // 求右边数组的维度
                        let  mut right = 0;
                        let mut exp_ident = expr_rule.as_str().to_string();
                        if expr_rule.as_str().ends_with("]") {
                            right = expr_rule.as_str().chars().filter(|&c| c == '[').count();
                            exp_ident = expr_rule.as_str().split("[").collect::<Vec<&str>>()[0].trim().to_string();
                        }
                        let right_all_size = self.variable_dels.iter().find(|e|eq_option_string(&e.name,&Some(exp_ident.clone()))).unwrap().array_dims.len();
                        if (left_all_size-left) !=  (right_all_size-right) {
                            let check_result = PairCheckResult::new(
                                line_no.to_string(),
                                Some(CheckError::new(
                                    ErrorKind::TypeMismatch,
                                    Some(expr_rule.as_str().to_string()),
                                )),
                            );
                            self.check_results.push(check_result);
                            return;
                        }
                    }
                    if v && v == !e_v2 {
                        let mut left = 0;
                        if ori_ident.ends_with("]") {
                            left = ori_ident.chars().filter(|&c| c == '[').count();
                        }
                        let left_all_size = self.variable_dels.iter().find(|e|eq_option_string(&e.name,&Some(ident.clone()))).unwrap().array_dims.len();
                        if left_all_size - left != 0 {
                            let check_result = PairCheckResult::new(
                                line_no.to_string(),
                                Some(CheckError::new(
                                    ErrorKind::TypeMismatch,
                                    Some(expr_rule.as_str().to_string()),
                                )),
                            );
                            self.check_results.push(check_result);
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
                            right_all_size = self.variable_dels.iter().find(|e|eq_option_string(&e.name,&Some(exp_ident.clone()))).unwrap().array_dims.len();
                        }
                        if  right != right_all_size {
                            let check_result = PairCheckResult::new(
                                line_no.to_string(),
                                Some(CheckError::new(
                                    ErrorKind::TypeMismatch,
                                    Some(expr_rule.as_str().to_string()),
                                )),
                            );
                            self.check_results.push(check_result);
                            return;
                        }
                    }
                    // 左侧是整形 右侧也是整形但是增加了[ ] 变成了数组
                    if !v && !e_v2 {
                        let exp_ident = expr_rule.as_str().to_string();
                        if exp_ident.as_str().ends_with("]") {
                            let check_result = PairCheckResult::new(
                                line_no.to_string(),
                                Some(CheckError::new(
                                    ErrorKind::NotArrayAssign,
                                    Some(expr_rule.as_str().to_string()),
                                )),
                            );
                            self.check_results.push(check_result);
                            return;
                        }
                    }
                    if !e_v1 || e_v2 {
                        let inner_pairs = expr_rule.clone().into_inner();
                        for inner_pair in inner_pairs {
                            if inner_pair.as_rule() == Rule::LVal {

                            }
                        }
                        // 存在不同类型的标识符进行基础运算
                        let check_result = PairCheckResult::new(
                            line_no.to_string(),
                            Some(CheckError::new(
                                ErrorKind::TypeMismatchOp,
                                Some(expr_rule.as_str().to_string()),
                            )),
                        );
                        self.check_results.push(check_result);
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

    /// 处理AddExp..等的维度问题
    // fn handle_exp(&mut self,pair:Pair<Rule>) ->

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
                        let check_result = PairCheckResult::new(
                            line_no.to_string(),
                            Some(CheckError::new(
                                ErrorKind::ReturnMismatch,
                                Some(pair_str.to_string()),
                            )),
                        );
                        self.check_results.push(check_result);
                        return;
                    }
                    if v_defined && !v {
                        if func_type == "void".to_string() {
                            let check_result = PairCheckResult::new(
                                line_no.to_string(),
                                Some(CheckError::new(
                                    ErrorKind::ReturnMismatch,
                                    Some(pair_str.to_string()),
                                )),
                            );
                            self.check_results.push(check_result);
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
                        let check_result = PairCheckResult::new(
                            line_no.to_string(),
                            Some(CheckError::new(
                                ErrorKind::ReturnMismatch,
                                Some(pair_str.to_string()),
                            )),
                        );
                        self.check_results.push(check_result);
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
    fn check_expr(&mut self,p:Pair<Rule>,check_op:u8)->(bool,bool) {
        match p.as_rule() {
            Rule::Ident => {
                let p = p.as_str().to_string();
                let (defined, v) = self.var_contains(&p);
                let f_defined = self.func_contains(&p);
                (v,defined||f_defined)
            }
            _ => {
                let inner_pairs = p.into_inner();
                let mut results = vec![];
                for inner_pair in inner_pairs {
                    results.push(self.check_expr(inner_pair,check_op));
                }
                let mut is_arr = if check_op == 1 {
                    (true, true)
                }else {
                    (false, false)
                };
                if check_op == 1 {
                    is_arr = results.iter().copied().fold(is_arr, |(v, d), (v1, d1)| {
                        (v && v1, d && d1)
                    });
                }else {
                    is_arr = results.iter().copied().fold(is_arr, |(v, d), (v1, d1)| {
                        (v || v1, d && d1)
                    });
                }
                is_arr
            }
        }
    }
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
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    if inner_pair.as_rule() == Rule::Ident {
                        let name = inner_pair.as_str().to_string();
                        let (v_defined,_) = self.var_contains(&name);
                        if !v_defined {
                            let check_result = PairCheckResult::new(
                                line_no.to_string(),
                                Some(CheckError::new(ErrorKind::UndefinedVal, Some(name.clone()))),
                            );
                            self.check_results.push(check_result);
                        }
                    }
                }
            }
            Rule::Ident => {
                // let i = a();
                // 这里只有从CallExp的进来
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
                            let check_result = PairCheckResult::new(
                                line_no.to_string(),
                                Some(CheckError::new(ErrorKind::UndefinedFunc,Some(ident.to_string()))),
                            );
                            self.check_results.push(check_result);
                            return;
                        }
                    }
                }
                if !self.func_contains(&ident) && no_var {
                    // 非函数调用错误
                    let check_result = PairCheckResult::new(
                        line_no.to_string(),
                        Some(CheckError::new(ErrorKind::UnlegalFuncCall,Some(ident.to_string()))),
                    );
                    self.check_results.push(check_result);
                    return;
                }
            }
            Rule::FuncRParams => {
                // 1、校验参数个数是否一致
                // 2、校验参数类型是否匹配
                let value = pair.as_str();
                let sources = value.split(',').collect::<Vec<_>>();
                let source_types = sources.iter().map(|s|{
                    return if s.contains("[") | s.contains("]") {
                        "1"
                    } else {
                        "0"
                    }
                }).collect::<Vec<_>>().join("");
                let line_no = pair.line_col().0;
                // 获取函数的参数类型
                let func_ = self.func_dels.iter().find(|e| e.name == self.call_func);
                if let Some(func) = func_ {
                    let params = func.params.clone();
                    let result = params
                        .iter()
                        .map(|p| {
                            return if p.is_array_type() {
                                "1"
                            } else {
                                "0"
                            }
                        })
                        .collect::<Vec<_>>().join("");
                    if source_types.len() != result.len() || source_types != result  //参数个数不一致 || 参数类型不一致 都输出参数不适用
                    {
                        let check_result = PairCheckResult::new(
                            line_no.to_string(),
                            Some(CheckError::new(ErrorKind::Inappropriate,Some(value.to_string()))),
                        );
                        self.check_results.push(check_result);
                    }
                }
            }
            _ => {}
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
                let (defined,_) = self.var_contains(&ident);
                if defined {
                    let defined_belongs = self.variable_dels.iter().find(|e|eq_option_string(&e.name,&Some(ident.to_string()))).unwrap().belongs_to;
                    if self.block_no == defined_belongs {  // 同一个作用域内多次定义才能被认定为重复定义
                        let check_result = PairCheckResult::new(
                            line_no.to_string(),
                            Some(CheckError::new(ErrorKind::RedefineVal,Some(ident.to_string()))),
                        );
                        self.check_results.push(check_result);
                    }
                }
                def.name = Some(ident.to_string());
            }
            Rule::ArrayDims => {
                let inner_pairs = pair.into_inner();
                for p in inner_pairs {
                    self.walk_array_dims(p, def);
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

    fn walk_array_dims(&mut self, pair: Pair<Rule>, def: &mut VariableDel) {
        match pair.as_rule() {
            Rule::ConstExp => {
                def.array_dims.push(pair.as_str().to_string());
            }
            _ => {}
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
                CheckError::UnlegalFuncCall(kind, "Illegal function call", error_tip)
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