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
use std::process::id;

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
}

impl<'a, W: Write> Checker<'a, W> {
    fn new(input: &'a str, writer: &'a mut W) -> Self {
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
        }
    }
    fn syn_check(&mut self) -> io::Result<()> {
        if let Some(mut pairs) = parse_file(self.input) {
            // 把File规则的第一个pair拿出来
            let pairs = pairs.next().unwrap();
            // 继续把File规则的内容拿出来
            let pairs = pairs.into_inner().next().unwrap();
            // 把编译单元的内容拿出来
            let pairs = pairs.into_inner();
            dbg!(&pairs);
            for pair in pairs {
                self.check(pair);
            }
            writeln!(self.writer, "{}", self.output)?;
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
            Rule::LVal => {
                let line_no = pair.line_col().0;
                // 不考虑数组的赋值语句校验
                if !pair.as_str().contains("[") {
                    let ident = pair.into_inner().next().unwrap().as_str();
                    let (defined, v) = self.var_contains(ident);
                    if !defined {
                        // 变量未定义错误
                        let check_result = PairCheckResult::new(
                            line_no.to_string(),
                            Some(CheckError::new(
                                ErrorKind::UndefinedVal,
                                Some(ident.to_string()),
                            )),
                        );
                        self.check_results.push(check_result);
                    } else {
                        if defined && v {
                            // 类型错误
                            let check_result = PairCheckResult::new(
                                line_no.to_string(),
                                Some(CheckError::new(
                                    ErrorKind::UnexpectedType,
                                    Some(ident.to_string()),
                                )),
                            );
                            self.check_results.push(check_result);
                        }
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
            Rule::FuncFParams | Rule::Block | Rule::BlockItem | Rule::Exp | Rule::Stmt => {
                if pair.as_rule() == Rule::Block {
                    self.block_no += 1; // 进入一个新的函数作用域
                }
                let inner_pairs = pair.into_inner();
                for inner_pair in inner_pairs {
                    // dbg!(&inner_pair);
                    self.walk_func_def(inner_pair, func_def);
                }
            }
            Rule::LVal => {
                // 只校验单值
                self.check(pair);
            }
            Rule::Decl => {
                self.check(pair);
            }
            _ => {}
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
                // 去校验赋值表达式使用的变量是否已经定义
                self.check(pair);
            }
            Rule::Ident => {
                //1、校验调用的函数是否未定义
                let ident = pair.as_str();
                let line_no = pair.line_col().0;
                if !self.func_contains(&ident)  {
                    // 函数未定义
                    let check_result = PairCheckResult::new(
                        line_no.to_string(),
                        Some(CheckError::new(ErrorKind::RedefineFunc,Some(ident.to_string()))),
                    );
                    self.check_results.push(check_result);
                }else {
                    // TODO函数未定义
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
                def.name = Some(pair.as_str().to_string());
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
                "Error type {} at line{}: {}",
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
    ParamMismatch = 6,
    ReturnMismatch = 7,
    UnexpectedType = 8,
    UnexpectedOperator = 9,
    UnlegalFuncCall = 10,
    UnexpectedAssign = 11,
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
    ParamMismatch(ErrorKind, &'static str, Option<String>),
    ReturnMismatch(ErrorKind, &'static str, Option<String>),
    UnexpectedType(ErrorKind, &'static str, Option<String>),
    UnexpectedOperator(ErrorKind, &'static str, Option<String>),
    UnlegalFuncCall(ErrorKind, &'static str, Option<String>),
    UnexpectedAssign(ErrorKind, &'static str, Option<String>),
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
            ErrorKind::TypeMismatch => CheckError::TypeMismatch(kind, "Type mismatch", error_tip),
            ErrorKind::ParamMismatch => {
                CheckError::ParamMismatch(kind, "Parameter mismatch", error_tip)
            }
            ErrorKind::ReturnMismatch => {
                CheckError::ReturnMismatch(kind, "Return type mismatch", error_tip)
            }
            ErrorKind::UnexpectedType => {
                CheckError::UnexpectedType(kind, "Unexpected type", error_tip)
            }
            ErrorKind::UnexpectedOperator => {
                CheckError::UnexpectedOperator(kind, "Unexpected operator", error_tip)
            }
            ErrorKind::UnlegalFuncCall => {
                CheckError::UnlegalFuncCall(kind, "Illegal function call", error_tip)
            }
            ErrorKind::UnexpectedAssign => {
                CheckError::UnexpectedAssign(kind, "Unexpected assignment", error_tip)
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
            | CheckError::ParamMismatch(kind, _, _)
            | CheckError::ReturnMismatch(kind, _, _)
            | CheckError::UnexpectedType(kind, _, _)
            | CheckError::UnexpectedOperator(kind, _, _)
            | CheckError::UnlegalFuncCall(kind, _, _)
            | CheckError::UnexpectedAssign(kind, _, _)
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
            | CheckError::ParamMismatch(_, _, tip)
            | CheckError::ReturnMismatch(_, _, tip)
            | CheckError::UnexpectedType(_, _, tip)
            | CheckError::UnexpectedOperator(_, _, tip)
            | CheckError::UnlegalFuncCall(_, _, tip)
            | CheckError::UnexpectedAssign(_, _, tip)
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
            | CheckError::ParamMismatch(_, msg, _)
            | CheckError::ReturnMismatch(_, msg, _)
            | CheckError::UnexpectedType(_, msg, _)
            | CheckError::UnexpectedOperator(_, msg, _)
            | CheckError::UnlegalFuncCall(_, msg, _)
            | CheckError::UnexpectedAssign(_, msg, _)
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
    fn test_lab3_example() {
        let filename = FILE_PATH.to_string() + "test.sy";
        let file = std::fs::read_to_string(filename).expect("Failed to read file");
        let mut binding = stdout();
        let mut checker = Checker::new(&file, &mut binding);
        checker.syn_check().unwrap();
        dbg!(checker);
    }
}
