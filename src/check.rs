use crate::check;
use crate::utils::add_option_string;
use crate::utils::eq_option_string;
use clap::builder::Str;
use pest::Parser;
use pest::iterators::Pair;
use pest::iterators::Pairs;
use pest::pratt_parser::Op;
use pest_derive::Parser;
use std::borrow::Cow;
use std::fmt::Display;
use std::io;
use std::io::Write;
use std::ops::Add;
use std::process::id;

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
    context_stack: Vec<Option<FuncDef>>, // 作用域级别
    // 当前作用域，越小可见性越高
    block_no: usize,
    check_results: Vec<PairCheckResult<'a>>,
    // 当前函数调用
    call_func: Option<String>,
}

impl<'a, W: Write> Checker<'a, W> {
    pub fn new(input: &'a str, writer: &'a mut W) -> Self {
        let mut context_stack = Vec::new();
        context_stack.push(None);
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
            } else {
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
                    if should_skip {
                        continue;
                    }
                    // 收集函数信息
                    let pair_str = inner_pair.as_str().to_string();
                    // 1、收集函数类型
                    if inner_pair.as_rule() == Rule::FuncType {
                        func_def.return_type = Some(pair_str);
                    }
                    // 2、收集函数名称
                    if inner_pair.as_rule() == Rule::FuncName {
                        let name = inner_pair.as_str().to_string();
                        //3 重复定义判断
                        let is_redefine = self.check_redefine(&name, line_no, 1);
                        should_skip |= is_redefine;
                        if is_redefine {
                            continue;
                        }
                        func_def.name = Some(name.clone());
                        // 4、函数加入当前上下文的栈，方便后续扫描的语句处理
                        self.context_stack.push(Some(func_def.clone()));
                    } else {
                        // 5、处理函数块中的所有语句
                        self.walk_func_def(inner_pair, &mut func_def);
                    }
                }
                // 只有在没有重复定义的情况下才添加函数定义
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
            }
            Rule::ConstDecl | Rule::VarDecl => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    match inner_pair.as_rule() {
                        Rule::ConstDef => self.handle_const_var_def(inner_pair, true),
                        Rule::VarDef => self.handle_const_var_def(inner_pair, false),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn walk_func_def(&mut self, pair: Pair<Rule>, func_def: &mut FuncDef) {
        match pair.as_rule() {
            Rule::Block | Rule::BlockItem | Rule::Stmt | Rule::FuncFParams => {
                if pair.as_rule() == Rule::Block {
                    self.block_no += 1; // 进入一个新的函数作用域
                }
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.walk_func_def(inner_pair, func_def)
                }
            }
            Rule::FuncFParam => {
                let line_no = pair.line_col().0;
                let str_pair = pair.as_str().to_string();
                let inner_pairs = pair.into_inner();
                let mut param = FuncParam::new();
                for inner_pair in inner_pairs {
                    match inner_pair.as_rule() {
                        Rule::BType => {
                            param.var_type = Some(inner_pair.as_str().to_string());
                        }
                        Rule::FuncFParamName => {
                            param.name = Some(inner_pair.as_str().to_string());
                        }
                        Rule::NonIntArr => {
                            param.array_dims.push("-1".to_string());
                        }
                        Rule::IntArr => {
                            param.array_dims.push(inner_pair.as_str().to_string());
                        }
                        _ => {}
                    }
                }
                // 校验参数名是否重复定义。注意此时函数的声明还未加入到全局符号表中
                // 通过context_stack获取当前函数作用域的参数集合
                if let Some(func) = self.context_stack.pop().unwrap() {
                    if func.contains(&param.name.clone().unwrap()) {
                        self.add_check_result(line_no, ErrorKind::RedefineVal, Some(str_pair));
                    } else {
                        // 往函数中添加参数
                        func_def.params.push(param);
                        // 将添加完参数的函数重新添加到栈中
                        self.context_stack.push(Some(func_def.clone()));
                    }
                }
            }
            Rule::AssignStmt => {
                self.check_expr(pair);
            }
            Rule::Decl => {
                self.check(pair);
            }
            Rule::ReturnStmt => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.walk_return_stmt(inner_pair, func_def);
                }
            }
            Rule::Exp | Rule::Cond => {
                self.check_expr(pair);
            }
            _ => {}
        }
    }

    /// # 解析表达式，
    ///
    /// 表达式中可能存在：数组、函数调用、变量访问。
    ///
    /// 需要有以下校验：
    /// - 判断访问的数组元素是否越界
    /// - 函数返回值是否其他类型一致
    /// - 变量/数组当作函数访问
    /// - 函数/变量当作数组访问
    /// - 函数当作变量访问
    /// - 函数调用参数个数是否一致
    /// 如果存在数组类型，需要返回表达式的整体维度
    fn check_expr(&mut self, pair: Pair<Rule>) {
        match pair.as_rule() {
            Rule::AssignStmt => {
                let str_pair = pair.as_str().to_string();
                let line_no = pair.line_col().0;
                let inner_pairs = pair.into_inner();
                let mut l_var: Option<(i8, usize)> = None;
                let mut r_expr: Option<usize> = None;
                for inner_pair in inner_pairs {
                    match inner_pair.as_rule() {
                        Rule::LVal => {
                            let str_pair = inner_pair.as_str().to_string();
                            l_var = check_l_var(self, inner_pair);
                            if l_var.is_some() {
                                let (var_type, _) = l_var.unwrap();
                                if var_type == 2 {
                                    // 函数调用
                                    self.add_check_result(
                                        line_no,
                                        ErrorKind::UnexpectedFuncAssign,
                                        Some(str_pair),
                                    );
                                    return;
                                }
                            }
                        }
                        Rule::Exp => {
                            let str_pair = inner_pair.as_str().to_string();
                            // 先扁平化
                            let mut ps = Vec::new();
                            flat_exp(self, inner_pair, &mut ps);
                            r_expr = get_type_dims(self, line_no, &mut ps, Some(str_pair),true);
                        }
                        _ => {}
                    }
                }
                if l_var.is_some() && r_expr.is_some() {
                    let (_, l_var_dims) = l_var.unwrap();
                    let r_expr_dims = r_expr.unwrap();
                    if l_var_dims != r_expr_dims {
                        self.add_check_result(line_no, ErrorKind::TypeMismatch, Some(str_pair));
                    }
                }
            }
            Rule::CallExp => {
                let _ = check_call_exp(self, pair);
            }
            Rule::LVal => {
                let _ = check_l_var(self, pair);
            }
            _ => {
                if self.has_operator(pair.clone()) {
                    let line_no = pair.line_col().0;
                    let str_pair = pair.as_str().to_string();
                    let mut ps = Vec::new();
                    flat_exp(self, pair, &mut ps);
                    get_type_dims(self, line_no, &mut ps, Some(str_pair),false);
                    return;
                }
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.check_expr(inner_pair);
                }
            }
        }

        fn check_unary_op_exp<W: Write>(
            checker: &mut Checker<W>,
            pair: Pair<Rule>,
        ){
            let line_no = pair.line_col().0;
            let pair_str = pair.as_str().to_string();
            if pair.as_rule() != Rule::UnaryOpExp {
                return;
            }
            let mut ident = "";
            for inner_pair in pair.into_inner() {
                if inner_pair.as_rule() == Rule::UnaryExp {
                    ident = inner_pair.as_str();
                    break;
                }
            }
            if !ident.is_empty() {
               let re =  checker.get_ident_type(ident,line_no,0);
                if re != 0 {
                    checker.add_check_result(line_no, ErrorKind::TypeMismatchOp, Some(pair_str));
                }
            }
        }

        fn get_type_dims<'a, W: Write>(
            checker: &mut Checker<W>,
            line_no: usize,
            pairs: &mut Vec<Pair<'a, Rule>>,
            tips: Option<String>,
            is_assign: bool,
        ) -> Option<usize> {
            let mut results = Vec::<Option<(i8,usize)>>::new();
            let pairs_len = pairs.len();
            for pair in pairs {
                match pair.as_rule() {
                    Rule::LVal => {
                        let pair_str = pair.as_str().to_string();
                       let res =  check_l_var(checker, pair.clone());
                        if res.is_some() && res.unwrap().0 == 2 && !is_assign{
                            // 说明函数没有正确被使用
                            checker.add_check_result(line_no,ErrorKind::TypeMismatchOp,tips.clone());
                             break;
                        }
                        if res.is_some() && res.unwrap().0 == 1 && !pair_str.contains('[') && !is_assign {
                            checker.add_check_result(line_no,ErrorKind::TypeMismatchOp,tips.clone());
                            break;
                        }
                        results.push(res);
                    }
                    Rule::CallExp => {
                        results.push(check_call_exp(checker, pair.clone()));
                    }
                    _ => {
                        results.push(Some((0,0)));
                    }
                }
            }

            if results.len() < pairs_len {
                return None;
            }

            // 1) 若有 None，直接返回 None（表示上游已报错或无法判定）
            if results.iter().any(|r| r.is_none()) {
                return None;
            }

            // 2) 全部都是 Some，取出里面的值
            let vals: Vec<(i8, usize)> = results.iter().map(|r| r.unwrap()).collect();

            // 空集合（理论上不该发生），按需要决定返回什么；这里返回默认 (0,1)
            if vals.is_empty() {
                return None;
            }

            // 判断是否全部相等：用 windows(2) 检查相邻相等
            let all_equal = vals.windows(2).all(|w| w[0].1 == w[1].1);

            if all_equal {
                Some(vals[0].1)
            } else {
                checker.add_check_result(line_no, ErrorKind::TypeMismatchOp, tips);
                None
            }
        }

        fn flat_exp<'a, W: Write>(
            checker: &mut Checker<W>,
            pair: Pair<'a, Rule>,
            pairs: &mut Vec<Pair<'a, Rule>>,
        ) {
            match pair.as_rule() {
                Rule::LVal | Rule::CallExp | Rule::Number => {
                    pairs.push(pair);
                }
                Rule::UnaryOpExp => {
                    check_unary_op_exp(checker, pair);
                }
                _ => {
                    let inner_pairs = pair.into_inner();
                    for inner_pair in inner_pairs {
                        flat_exp(checker, inner_pair, pairs);
                    }
                }
            }
        }

        fn check_l_var<W: Write>(
            checker: &mut Checker<W>,
            pair: Pair<Rule>,
        ) -> Option<(i8, usize)> {
            if pair.as_rule() != Rule::LVal {
                return None;
            }
            // 访问变量
            let pair_str = pair.as_str().to_string();
            let line_no = pair.line_col().0;
            let inner_pairs = pair.into_inner();
            let mut var_type = -1;
            let mut dims = usize::MAX;
            let mut var_name = "".to_string();
            for inner_pair in inner_pairs {
                match inner_pair.as_rule() {
                    Rule::LVarName => {
                        // 注意这里无法校验 变量和数组当前函数使用以及函数参数使用等错误
                        // 校验变量/函数当作数组使用错误
                        // 校验数组的访问越界
                        // 拿到变量/数组名称
                        // 判断是否定义
                        var_name = inner_pair.as_str().to_string();
                        // 获取实际类型
                        var_type = checker.get_ident_type(&var_name, line_no, 0);
                        if var_type == 1 {
                            dims = checker.get_array_dims(&var_name);
                        }
                        if (var_type == 0 || 
                            var_type == 2) &&
                            pair_str.contains('[') {
                            // 变量和函数被当作数组使用
                            checker.add_check_result(
                                line_no,
                                ErrorKind::NotArrayAssign,
                                Some(inner_pair.as_str().to_string()),
                            );
                        }
                    }
                    Rule::Array => {
                        match var_type {
                            1 => {
                                // 获取数组的原始维度
                                dims -=1 ;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            if var_type < 0 {
                return None;
            }
            if dims == usize::MAX && var_type != 1 {
                dims = 0;
            }
            Some((var_type, dims))
        }

        fn check_call_exp<W: Write>(
            checker: &mut Checker<W>,
            pair: Pair<Rule>,
        ) -> Option<(i8, usize)> {
            if pair.as_rule() != Rule::CallExp {
                return None;
            }
            let line_no = pair.line_col().0;
            let inner_pairs = pair.into_inner();
            let mut func_name = "".to_string();
            let mut func_type = -1;
            for inner_pair in inner_pairs {
                match inner_pair.as_rule() {
                    Rule::CallName => {
                        func_name = inner_pair.as_str().to_string();
                        func_type = checker.get_ident_type(&func_name, line_no, 1);
                        if func_type == 0 || func_type == 1 {
                            // 函数被当作变量使用
                            checker.add_check_result(
                                line_no,
                                ErrorKind::UnlegalFuncCall,
                                Some(func_name.clone()),
                            );
                            return None;
                        }
                        if func_type < 0 {
                            return None;
                        }
                    }
                    Rule::FuncRParams => {
                        // 校验使用函数时参数个数和类型是否对应
                        // 获取使用时的参数
                        let mut use_params = vec![];
                        let pair_str = inner_pair.as_str();
                        for param in pair_str.split(",") {
                            let param_type = if checker.is_number_str(param.trim()) {
                                0
                            } else {
                                // TODO 要处理 入参是 表达式的情况 如 r - 1
                                let mut param_name  = param.trim().to_string();
                                let mut idents = Vec::new();
                                fn get_ident(p:Pair<Rule>,idents:&mut Vec<String>) {
                                   match p.as_rule() {
                                       Rule::Ident => idents.push(p.as_str().to_string()),
                                       _ => {
                                           let inners = p.into_inner();
                                           for inner in inners {
                                               get_ident(inner,idents);
                                           }
                                       }
                                   }
                                }
                              get_ident(inner_pair.clone(),&mut idents);
                                if !idents.is_empty() {
                                    param_name = idents[0].clone();
                                }
                                checker.get_ident_type(&param_name, line_no, 0)
                            };
                            use_params.push(param_type);
                        }
                        if let Some(func_def) = checker.get_func_by_name(&func_name) {
                            let params = func_def.params.clone();
                            if params.len() != use_params.len() {
                                // 参数个数不一致
                                checker.add_check_result(
                                    line_no,
                                    ErrorKind::Inappropriate,
                                    Some(func_name.clone()),
                                );
                                return None;
                            } else {
                                for (i, param) in params.iter().enumerate() {
                                    if !param.eq_type(use_params[i].to_string()) {
                                        // 参数类型不一致
                                        checker.add_check_result(
                                            line_no,
                                            ErrorKind::Inappropriate,
                                            Some(func_name.clone()),
                                        );
                                        return None;
                                    } else {
                                        // 参数维度不一致，因为这里仅支持一维数组所以只考虑一维数组的情况
                                        if param.is_array_type() && param.array_dims.len() != 1 {
                                            checker.add_check_result(
                                                line_no,
                                                ErrorKind::Inappropriate,
                                                Some(func_name.clone()),
                                            );
                                            return None;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Some((2, 0))
        }
    }

    /// 判断字符串是否为数字
    /// 支持:
    /// - 十进制 (例如 "123")
    /// - 十六进制 (以 "0x" 或 "0X" 开头，例如 "0x1A")
    /// - 八进制 (以 "0" 开头且不是 "0x"，例如 "0777")
    fn is_number_str(&self, s: &str) -> bool {
        if s.starts_with("0x") || s.starts_with("0X") {
            i32::from_str_radix(&s[2..], 16).is_ok()
        } else if s.starts_with('0') && s.len() > 1 {
            i32::from_str_radix(&s[1..], 8).is_ok()
        } else {
            s.parse::<i32>().is_ok()
        }
    }

    fn get_array_dims(&self, name: &str) -> usize {
        let result =
        self.variable_dels
            .iter()
            .find(|x| x.name == Some(name.to_string()))
            .unwrap()
            .array_dims
            .len();
        result
    }

    fn handle_const_var_def(&mut self, pair: Pair<Rule>, is_const: bool) {
        let mut val = VariableDel::new(pair.line_col().0);
        val.is_const = is_const;
        val.var_type = Some("int".to_string());
        // 记录当前的可见度
        val.belongs_to = self.block_no.clone();
        self.walk_val_def(pair, &mut val);
        if !val.is_error {
            self.variable_dels.push(val);
        }
    }

    fn walk_val_def(&mut self, pair: Pair<Rule>, val: &mut VariableDel) {
        let inner_pairs = pair.into_inner();
        for inner_pair in inner_pairs {
            match inner_pair.as_rule() {
                Rule::Ident => {
                    let name = inner_pair.as_str();
                    val.name = Some(name.to_string());
                    let line_no = inner_pair.line_col().0;
                    // 校验变量是否重复定义
                    self.check_redefine(name, line_no, 0);
                }
                Rule::ArrayDims => {
                    let inner_pairs = inner_pair.into_inner();
                    for inner_pair in inner_pairs {
                        if inner_pair.as_rule() == Rule::ConstExp {
                            val.array_dims.push(inner_pair.as_str().to_string());
                        }
                    }
                }
                Rule::InitVal => {
                    let inners = inner_pair.into_inner();
                    for inner in inners {
                        if inner.as_rule() == Rule::Exp {
                            self.check_expr(inner);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn has_operator(&self, pair: Pair<Rule>) -> bool {
        match pair.as_rule() {
            Rule::Plus
            | Rule::Minus
            | Rule::Mul
            | Rule::Div
            | Rule::Mod
            | Rule::Not
            | Rule::And
            | Rule::Or
            | Rule::Less
            | Rule::Greater
            | Rule::LessEqual
            | Rule::GreaterEqual
            | Rule::Equal
            | Rule::NotEqual => true,
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

    fn walk_return_stmt(&mut self, pair: Pair<Rule>, func_def: &mut FuncDef) {
        match pair.as_rule() {
            Rule::Ident => {
                let line_no = pair.line_col().0;
                let pair_str = pair.as_str();
                let func_type = func_def.return_type.clone();
                let (v_defined, v) = self.var_contains(&pair_str);
                let f_defined = self.func_contains(&pair_str);
                if let Some(func_type) = func_type {
                    if f_defined || v {
                        self.add_check_result(
                            line_no,
                            ErrorKind::ReturnMismatch,
                            Some(pair_str.to_string()),
                        );
                        return;
                    }
                    if v_defined && !v {
                        if func_type == "void".to_string() {
                            self.add_check_result(
                                line_no,
                                ErrorKind::ReturnMismatch,
                                Some(pair_str.to_string()),
                            );
                        }
                    }
                }
            }
            Rule::LVal => {
                let pair_str = pair.as_str();
                if pair_str.ends_with("]") {
                    let ident = pair.into_inner().next().unwrap();
                    self.walk_return_stmt(ident, func_def);
                } else {
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
                        self.add_check_result(
                            line_no,
                            ErrorKind::ReturnMismatch,
                            Some(pair_str.to_string()),
                        );
                    }
                }
            }
            _ => {
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    self.walk_return_stmt(inner_pair, func_def);
                }
            }
        }
    }

    /// 获取ident的类型
    /// -1 -> 未定义
    /// 0 -> 变量
    /// 1 -> 数组
    /// 2 -> 函数
    fn get_ident_type(&mut self, ident: &str, line_no: usize, op: usize) -> i8 {
        let (v_defined, v) = self.var_contains(ident);
        let f_defined = self.func_contains(ident);
        let kind = if op == 0 {
            ErrorKind::UndefinedVal
        } else {
            ErrorKind::UndefinedFunc
        };
        if v_defined && v {
            return 1;
        }
        if v_defined && !v {
            return 0;
        }
        if f_defined {
            return 2;
        }
        if let Some(func) = self.get_current_context() {
            if let Some(func_name) = func.name.clone() {
                if func_name == ident {
                    return 2;
                }
            }
            let param = func.get_param(ident);
            if let Some(p) = param {
                if p.is_array_type() {
                    return 1;
                } else {
                    return 0;
                }
            }
        }
        self.add_check_result(line_no, kind, Some(ident.to_string()));
        -1
    }

    /// 检查标识符是否被重复定义。
    ///
    /// 该方法会在以下几种情况中进行重定义检测:
    ///
    /// 1. **与已有函数重名**
    ///    如果传入的标识符名称已存在于函数定义中，则记录为函数或变量重定义错误。
    ///
    /// 2. **在同一作用域内重复定义变量**
    ///    检查 `self.variable_dels` 中是否已经存在同名且属于当前 `block_no` 的变量，
    ///    若存在则判定为重定义。
    ///
    /// 3. **函数参数重复定义**
    ///    若当前上下文存在参数列表，则检查是否已有同名参数，若存在则判定为重定义。
    ///
    /// # 参数
    /// - `ident`: 待检查的标识符名称。
    /// - `line_no`: 出现重定义的源码行号，用于错误提示。
    /// - `op`: 指示错误种类的操作码:
    ///   - `0`: 变量重定义（[`ErrorKind::RedefineVal`]）。
    ///   - `1`: 函数重定义（[`ErrorKind::RedefineFunc`]）。
    ///
    /// # 示例
    /// ```ignore
    /// checker.check_redefine("x", 10, 0); // 检查变量 x 是否在第10行重定义
    /// checker.check_redefine("foo", 15, 1); // 检查函数 foo 是否在第15行重定义
    /// ```
    ///
    /// # 注意
    /// - 函数会根据检测结果调用 [`add_check_result`] 记录错误信息。
    /// - 这个函数只适合用于对函数、变量使用，对于函数参数不适用
    fn check_redefine(&mut self, ident: &str, line_no: usize, op: usize) -> bool {
        let (var_defined, _) = self.var_contains(&ident);
        let func_defined = self.func_contains(&ident);
        let kind = if op == 0 {
            ErrorKind::RedefineVal
        } else {
            ErrorKind::RedefineFunc
        };
        // 检查是否与函数重名
        if func_defined {
            self.add_check_result(line_no, kind, Some(ident.to_string()));
            return true;
        }

        // 检查是否在同一作用域内重复定义变量
        if var_defined {
            // 查找同名变量在当前作用域的定义
            let same_scope_var = self.variable_dels.iter().find(|e| {
                eq_option_string(&e.name, &Some(ident.to_string())) && e.belongs_to == self.block_no
            });

            if same_scope_var.is_some() {
                self.add_check_result(line_no, kind, Some(ident.to_string()));
                return true;
            }
        }

        // 检查函数参数重复定义
        if let Some(func) = self.get_current_context()
            && op == 0
        {
            if func.contains(ident) {
                self.add_check_result(line_no, kind, Some(ident.to_string()));
                return true;
            }
        }
        false
    }

    fn func_contains(&self, ident: &str) -> bool {
        self.func_dels
            .iter()
            .find(|e| eq_option_string(&e.name, &Some(ident.to_string())))
            .is_some()
    }

    fn var_contains(&self, ident: &str) -> (bool, bool) {
        // 先校验函数入参
        if let Some(func) = self.get_current_context() {
            let result = func
                .params
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
                    && v.belongs_to <= self.block_no
                //需要标准清楚 可见性问题 【变量所属作用域级别 小于 当前作用域】即不可见
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
    fn get_current_context(&self) -> Option<&FuncDef> {
        Option::from(self.context_stack.last().unwrap())
    }
    
    fn get_func_by_name(&self, name: &str) -> Option<&FuncDef> {
        let result = self.func_dels.iter().find(|f| eq_option_string(&f.name, &Some(name.to_string())));
        if result.is_some() {
             result
        }else {
            self.get_current_context()
        }
    }

    fn add_check_result(&mut self, line_no: usize, error_kind: ErrorKind, tip: Option<String>) {
        let check_result =
            PairCheckResult::new(line_no.to_string(), Some(CheckError::new(error_kind, tip)));
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

    fn get_param(&self, ident: &str) -> Option<&FuncParam> {
        self.params
            .iter()
            .find(|p| eq_option_string(&p.name, &Some(ident.to_string())))
    }

    fn contains(&self, ident: &str) -> bool {
        if self.params.is_empty() {
            false
        } else {
            self.params
                .iter()
                .any(|p| eq_option_string(&p.name, &Some(ident.to_string())))
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

    fn eq_type(&self, other_type: String) -> bool {
        (self.is_array_type() && other_type.eq("1")) ||
           (!self.is_array_type()&& other_type.eq("0"))
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
                "The left-hand side of an assignment must be a variable.",
                error_tip,
            ),
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

    #[test]
    #[ignore]
    fn test_number() {
        assert_eq!("0x10".parse::<i32>().is_ok(), true)
    }
}
