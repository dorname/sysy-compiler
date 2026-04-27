---
title: 功能设计文档
description: Rust SysY 编译器（compiler）前端、中端与 RISC-V 后端的综合功能设计。
commit_pin: f5b8eb08d74b85cdf4c78c4ecfdae4880dcfeb1c
---

# 功能设计文档

## 1. 概述

### 1.1 系统目的

本项目是一个基于 Rust 的 SysY 语言（一种类 C 的教学语言）编译器（compiler），目标平台为 RISC-V 汇编。编译器流水线（compiler pipeline）包含以下阶段：

1. **词法分析（lexical analysis）** — 基于 pest 的 PEG 分词器
2. **语法分析 / 格式化（parsing / formatting）** — 基于 pest 的格式化器，带有布局规则
3. **语义检查（semantic checking）** — 支持作用域（scope）感知的类型检查器（type checker），包含 11 种错误类别
4. **LLVM IR 生成** — 使用 Inkwell  crate 进行的两阶段扫描
5. **RISC-V 后端（backend）** — 采用线性扫描寄存器分配（linear-scan register allocation）的两遍代码生成

### 1.2 设计理念

- **PEG 优先的前端（frontend）**：所有语法分析阶段均复用 pest 文法（`src/pests/lexer.pest`、`parser.pest`、`check.pest`、`scan.pest`），而非手写的递归下降解析器。 [EXEC-VERIFIED]
- **作用域栈符号表（scope-stack symbol tables）**：语义检查器和 IR 生成器均使用相同的 `ScopeStack` / `ScopeKey` 抽象来处理嵌套词法作用域（nested lexical scoping）。 [EXEC-VERIFIED]
- **后续阶段假设输入格式正确**：`gen_llvm_ir.rs` 和 `riscv_codegen/` 不输出面向用户的诊断信息；它们假设源文件已通过语义分析。 [STATIC-INFERRED]
- **两遍后端（two-pass backend）**：RISC-V 后端首先收集 def/use 信息并计算变量活跃性（liveness），然后使用分配结果发出指令。 [EXEC-VERIFIED]

---

## 2. 词法分析器功能设计（`src/lexer.rs`）

### 2.1 输入与输出

| 方面 | 说明 |
|------|------|
| **输入** | 原始 SysY 源字符串（`&str`）。 |
| **输出** | 词法单元流（token stream）写入 `io::Write` 接收端，或通过 `tokenize` 写入 `stderr`。 |
| **失效模式** | pest 的解析失败被静默吞掉；仅发出成功解析的词法单元（或由文法识别的错误词法单元）。 [EXEC-VERIFIED] |

### 2.2 词法单元类型

`src/lexer.rs` 中的 `Token` 枚举定义了六个顶层变体 [EXEC-VERIFIED]：

| 变体 | 说明 | 显示示例 |
|------|------|----------|
| `Token::Identifier(String)` | 非关键字标识符 | `IDENT foo` |
| `Token::Operator(Operator)` | 运算符与标点符号 | `PLUS +` |
| `Token::Type(Type)` | 类型关键字 | `INT int` |
| `Token::Flow(Flow)` | 控制流关键字 | `IF if` |
| `Token::IntegerConst(IntegerConst)` | 十进制 / 十六进制 / 八进制字面量 | `INTEGER_CONST 42` |
| `Token::ErrorSyntax(ErrorSyntax)` | 词法错误词法单元 | `INVALID_HEX 0xGH` |

`IntegerConst` 包含三个子变体：`Hex(String)`、`Octal(String)`、`Dec(String)`。十六进制和八进制值在显示时通过 `src/utils.rs` 中的 `hex_to_int` 和 `oct_to_int` 转换为十进制。 [EXEC-VERIFIED]

`Operator` 覆盖 `+ - * / % = == != < <= > >= && || ! ( ) { } [ ] , ;`。 [EXEC-VERIFIED]

`Type` 覆盖 `int`、`float`、`void`、`const`。 [EXEC-VERIFIED]

`Flow` 覆盖 `if`、`else`、`while`、`break`、`continue`、`return`。 [EXEC-VERIFIED]

### 2.3 词法错误类别

`ErrorSyntax` 枚举了由 `lexer.pest` 识别的可恢复词法错误 [EXEC-VERIFIED]：

| 变体 | 触发条件 |
|------|----------|
| `VarError` | （保留 / 当前在文法中未使用） |
| `InvalidHex` | `0x` / `0X` 前缀后跟非十六进制数字 |
| `InvalidOctal` | `0` 前缀后跟非八进制数字 |
| `InvalidInteger` | （保留 / 当前未使用） |
| `InvalidOperator` | 无法识别的单字符，如 `#`、`&`、`|`、`~` |
| `UnKnownError` | 捕获其他任意意外字符的兜底规则 |

### 2.4 关键函数

- `parse_file(input: &str) -> Option<Pairs<'_, Rule>>` — 使用 `ExpressionParser`（派生自 `lexer.pest`）解析整个文件。任何 pest 错误均返回 `None`。 [EXEC-VERIFIED]
- `tokenizer<W: Write>(input: &str, w: W) -> io::Result<()>` — 主入口。若存在错误词法单元，仅输出行错误信息（`Error type A at Line N: …`）；否则输出完整词法单元流。 [EXEC-VERIFIED]
- `tokenize(input: &str)` — 便捷包装器，写入 `stderr`。 [EXEC-VERIFIED]
- `impl From<Pair<'_, Rule>> for Token` — 对 pest 配对进行递归下降以生成 `Token` 值。 [EXEC-VERIFIED]

### 2.5 不变式与失效模式

- **不变式**：输入中的每个字符要么被作为词法单元消耗，要么被某条 `ErrorSyntax` 规则匹配。 [STATIC-INFERRED]
- **失效模式**：若 pest 在顶层失败（例如未闭合的块注释），`parse_file` 返回 `None`，`tokenizer` 返回 `Ok(())` 且不输出任何内容。 [EXEC-VERIFIED]

---

## 3. 解析器 / 格式化器功能设计（`src/format.rs`）

### 3.1 输入与输出

| 方面 | 说明 |
|------|------|
| **输入** | 原始 SysY 源字符串（`&str`）。 |
| **输出** | 重新格式化的源代码（或错误行）写入 `io::Write` 接收端。 |
| **失效模式** | 若 pest 解析失败，不写入任何内容。 [EXEC-VERIFIED] |

### 3.2 AST 表示

格式化器不构建显式的抽象语法树（AST）数据结构；它直接操作由 `FParser`（派生自 `src/pests/parser.pest`）生成的 `pest::iterators::Pair<Rule>` 值。 [EXEC-VERIFIED]

### 3.3 格式化规则

`src/format.rs` 中的 `Formatter::fmt` 实现了以下布局策略 [EXEC-VERIFIED]：

| 构造 | 规则 |
|------|------|
| **缩进** | 4 空格块。`deep` 跟踪嵌套深度。`OpenBrace` 递增，`CloseBrace` 递减。 |
| **控制流** | `)` 后的语句（例如 `while (…) stmt`）缩进 +1 并放置在新行。`else` 后接非 `if` 语句时同样缩进。 |
| **数组对齐** | 数组维度（`ArrayDims`）按行内遍历，不添加额外空格。 |
| **分号** | 分号前的尾部空格被去除。分号后追加换行，除非设置了 `forbid_semicolon_newline`。 |
| **空行** | `clean_extra_blank_lines` 移除冗余空行，在函数定义之间恰好保留一个空行。`should_keep_blank_line` 保留前一条非空行以 `}` 结尾且下一条以 `int` / `void` / `const` 开头的行。 |

### 3.4 关键函数

- `Formatter::new(deep: usize, input: &'a str, writer: &'a mut W) -> Self` — 构造缩进为零的格式化器。 [EXEC-VERIFIED]
- `Formatter::fmt(&mut self, pair: Pair<Rule>)` — 对解析器规则进行递归分发。 [EXEC-VERIFIED]
- `Formatter::format_code(&mut self) -> io::Result<()>` — 编排解析过程，对每个顶层配对调用 `fmt`，然后输出 `err_output` 或清理后的格式化文本。 [EXEC-VERIFIED]
- `fmt(input: &str)` — 便捷包装器，写入 `stdout`。 [EXEC-VERIFIED]

### 3.5 不变式与失效模式

- **不变式**：输出行在分号前从不包含尾部空格。 [EXEC-VERIFIED]
- **失效模式**：`ErrorStmt` 配对在 `err_output` 中产生 `Error type A at Line N: …` 行；若存在任何此类错误，格式化输出被抑制，仅输出错误。 [EXEC-VERIFIED]

---

## 4. 语义检查器功能设计（`src/check.rs`）

### 4.1 输入与输出

| 方面 | 说明 |
|------|------|
| **输入** | 原始 SysY 源字符串（`&str`）。 |
| **输出** | 错误消息写入提供的 `io::Write` 接收端。空输出表示无语义错误。 |
| **失效模式** | 语法错误导致输出单行 `Syntax error`。 [EXEC-VERIFIED] |

### 4.2 作用域分析

#### ScopeStack、ScopeKey、ScopeFrame

检查器使用三种协同类型 [EXEC-VERIFIED]：

- `ScopeKey` — 枚举，包含 `Global`、`Ident(String)`、`InnerBlock`。
- `Scope` — 持有 `symbol_table: HashMap<String, Type>`。
- `ScopeStack` — 持有 `Vec<ScopeKey>`（栈）和 `HashMap<ScopeKey, Scope>`（帧）。

`ScopeStack` 的关键方法 [EXEC-VERIFIED]：

| 方法 | 行为 |
|------|------|
| `push(key: ScopeKey)` | 创建新的空 `Scope` 并压入键。 |
| `pop()` | 移除顶部帧。 |
| `get_current_scope() -> Option<&Scope>` | 返回顶部帧。 |
| `get_current_scope_mut() -> Option<&mut Scope>` | 可变地返回顶部帧（使用克隆以避免借用冲突）。 |
| `contains(name: &str) -> bool` | 自顶向下搜索栈中是否存在该名称。 |
| `get(name: &str) -> Option<Type>` | 查找最近的定义。 |

默认栈以 `ScopeKey::Global` 初始化。 [EXEC-VERIFIED]

### 4.3 类型检查

检查器支持的语义类型（`check::Type`）[EXEC-VERIFIED]：

| 变体 | 含义 |
|------|------|
| `Type::Int` | 32 位整数 |
| `Type::Void` | 空类型（用于函数返回） |
| `Type::Func(Func)` | 函数签名（参数 + 返回类型） |
| `Type::Array(ArrayStruct)` | 多维数组 |

检查器强制执行的兼容性规则 [EXEC-VERIFIED]：

- 赋值左侧不能是函数（`UnexpectedFuncAssign`）。
- 数组只能赋值给维度数完全相同的数组（`TypeMismatch`）。
- `int` 不能赋值给具有非零维度的数组（`TypeMismatch`）。
- 算术 / 关系 / 逻辑运算符要求两个操作数均为 `Int`（`TypeMismatchOp`）。
- 数组下标只允许用于 `Array` 类型（`NotArrayAssign`）。
- 函数调用参数数量和类型必须与被调用者签名匹配（`Inappropriate`）。
- 返回表达式类型必须与所在函数的返回类型匹配（`ReturnMismatch`）。

### 4.4 错误分类体系

检查器实现了 11 种编号错误类型（`ErrorKind`）[EXEC-VERIFIED]：

| ID | 枚举变体 | 消息 |
|----|----------|------|
| 1 | `UndefinedVal` | `Undefined variable` |
| 2 | `UndefinedFunc` | `Undefined function` |
| 3 | `RedefineVal` | `Redefined variable` |
| 4 | `RedefineFunc` | `Redefined function` |
| 5 | `TypeMismatch` | `Type mismatched for assignment` |
| 6 | `TypeMismatchOp` | `Type mismatched for op` |
| 7 | `ReturnMismatch` | `Type mismatched for return` |
| 8 | `Inappropriate` | `Function is not applicable for arguments` |
| 9 | `NotArrayAssign` | `Not an array` |
| 10 | `UnlegalFuncCall` | `Not a function` |
| 11 | `UnexpectedFuncAssign` | `The left-hand side of an assignment must be a variable` |
| 0 | `Other` | `other` |

错误累积在 `Checker::errors: Vec<SemanticError>` 中，并由 `generate_error_output` 刷新输出。 [EXEC-VERIFIED]

### 4.5 符号表

符号表（symbol table）按作用域拆分；不存在全局独立的 `func_table` 或 `var_table`。取而代之的是：

- 函数作为 `Type::Func` 插入**当前作用域**（通常为 `Global`）。 [EXEC-VERIFIED]
- 变量和数组作为 `Type::Int` 或 `Type::Array` 插入**当前作用域**。 [EXEC-VERIFIED]
- 函数参数作为 `Type::Int` 或 `Type::Array` 插入**函数体作用域**。 [EXEC-VERIFIED]

### 4.6 关键函数

- `Checker::new(input, writer) -> Self` — 以根作用域 `Global` 初始化检查器，使用空作用域栈。 [EXEC-VERIFIED]
- `Checker::syn_check(&mut self) -> io::Result<()>` — 使用 `CParser` 进行解析，并为每个顶层声明驱动 `analyze_declaration`。 [EXEC-VERIFIED]
- `Checker::analyze_declaration`、`analyze_var_decl`、`analyze_func_def`、`analyze_block_item`、`analyze_stmt` — 递归的 AST 遍历器。 [EXEC-VERIFIED]
- `Checker::analyze_exp` / `analyze_add_exp` / `analyze_mul_exp` / `analyze_unary_exp` / `analyze_call_exp` — 表达式类型推断，返回 `Option<Type>`。 [EXEC-VERIFIED]
- `Checker::check_same_ident` — 仅检测当前作用域内的重定义。 [EXEC-VERIFIED]
- `Checker::check_undefine` — 通过扫描所有活跃作用域检测使用前未定义。 [EXEC-VERIFIED]
- `Checker::collect_error` — 将 `SemanticError` 追加到内部错误向量。 [EXEC-VERIFIED]

### 4.7 不变式与失效模式

- **不变式**：`ScopeStack::get` 始终返回*最近的*外围定义（跨作用域允许遮蔽（shadowing），同一作用域内禁止）。 [EXEC-VERIFIED]
- **失效模式**：若 `parse_file` 返回 `None`，仅输出 `Syntax error`，不执行语义分析。 [EXEC-VERIFIED]

---

## 5. LLVM IR 生成器功能设计（`src/gen_llvm_ir.rs`）

### 5.1 输入与输出

| 方面 | 说明 |
|------|------|
| **输入** | 原始 SysY 源字符串（`&str`）和输出文件路径（`&str`）。 |
| **输出** | LLVM IR 文本文件（`.ll`），以及通过 `scan_collect_asm` 调用时可选的 RISC-V 汇编文件（`.s`）。 |
| **失效模式** | 解析失败、IR 验证失败或文件写入失败时返回 `Err(String)`。 [EXEC-VERIFIED] |

### 5.2 IR 构建策略

生成器使用**单阶段扫描**（在传统前端 / 后端意义上并非两阶段），交错进行符号收集与 IR 生成 [EXEC-VERIFIED]：

1. 使用 `CParser`（`scan.pest`）解析源文件。
2. 创建 `IrSession`（Inkwell 的 `Context`、`Module`、`Builder`，加上 `ScopeStack`）。
3. 遍历声明：
   - **全局变量**（`const` / `var`）：发出 `GlobalValue` 或 `alloca` + `store`，然后插入当前作用域。
   - **函数**：声明 `FunctionValue`，压入函数作用域，为参数发出 `alloca`，然后遍历函数体。
4. 使用 `module.verify()` 验证模块。
5. 使用 `module.print_to_file(...)` 打印到文件。

> 注意：模块头部注释提到两阶段扫描，但实际实现中符号收集与 IR 生成在单次遍历中完成，因为 pest 树按深度优先处理，且在任何使用出现之前符号已被加入作用域栈。 [STATIC-INFERRED]

### 5.3 Inkwell 使用模式

| 模式 | 位置 |
|------|------|
| `context.i32_type()` | 所有整数类型均为 32 位。 [EXEC-VERIFIED] |
| `builder.build_alloca(i32_type, name)` | 局部变量与参数。 [EXEC-VERIFIED] |
| `builder.build_store(ptr, value)` | 赋值与初始化。 [EXEC-VERIFIED] |
| `builder.build_load(ptr, name)` | 左值读取。 [EXEC-VERIFIED] |
| `builder.build_int_add/sub/mul/sdiv/srem` | 算术运算。 [EXEC-VERIFIED] |
| `builder.build_int_compare(...)` + `build_int_z_extend` | 关系运算与逻辑运算。 [EXEC-VERIFIED] |
| `builder.build_conditional_branch(cond, true_bb, false_bb)` | 短路求值 `&&` / `\|\|`。 [EXEC-VERIFIED] |
| `module.add_global(i32_type, None, name)` | 全局变量。 [EXEC-VERIFIED] |
| `module.add_function(name, fn_type, None)` | 函数声明。 [EXEC-VERIFIED] |
| `builder.build_return(Some/None)` | 返回语句；若用户代码缺少终止符，则追加默认终止符。 [EXEC-VERIFIED] |

### 5.4 IR 生成中的作用域管理

`gen_llvm_ir.rs` 中的 `ScopeStack<'ctx>` 镜像了语义检查器的设计，但使用 Inkwell 句柄作为键 [EXEC-VERIFIED]：

- `ScopeKey::Global` — 全局作用域。
- `ScopeKey::Ident(FunctionValue<'ctx>)` — 函数作用域。
- `ScopeKey::InnerBlock(BasicBlock<'ctx>)` — 基本块（basic block）作用域，用于 `{ … }` 块。

`Type<'ctx>` 枚举存储 `GlobalVar(PointerValue)`、`LocalVar(PointerValue)` 或 `Func(FunctionValue)`。 [EXEC-VERIFIED]

### 5.5 关键函数

- `Scanner::new(input, output) -> Self` — 包装 `IrCore::new()`。 [EXEC-VERIFIED]
- `Scanner::scan_collect(&self) -> Result<(), String>` — 仅生成 `.ll`。 [EXEC-VERIFIED]
- `Scanner::scan_collect_asm(&self, ir_output, asm_build_fn) -> Result<(), String>` — 生成 `.ll`，然后调用提供的 `asm_build_fn` 生成汇编。 [EXEC-VERIFIED]
- `Scanner::scan_func_def` — 构建 LLVM 函数、入口基本块、参数 `alloca` 和函数体。 [EXEC-VERIFIED]
- `Scanner::scan_if_stmt` / `scan_while_stmt` — 为基本块和分支指令创建控制流。 [EXEC-VERIFIED]
- `Scanner::scan_cond_for_branches` / `scan_l_or_exp_for_branches` / `scan_l_and_exp_for_branches` — 使用块级跳转实现短路布尔求值。 [EXEC-VERIFIED]

### 5.6 不变式与失效模式

- **不变式**：每个函数均以终止符结尾（`ret` 或默认的 `ret 0` / `ret void`）。 [EXEC-VERIFIED]
- **失效模式**：若源文件包含语法错误，`scan_collect` 返回 `Err("语法解析失败")`。 [EXEC-VERIFIED]
- **失效模式**：若 Inkwell IR 验证失败，错误字符串向上传播。 [EXEC-VERIFIED]

---

## 6. RISC-V 后端功能设计（`src/riscv_codegen/`）

### 6.1 输入与输出

| 方面 | 说明 |
|------|------|
| **输入** | SysY 源字符串（`&str`）和输出汇编路径（`&str`）。内部首先通过 `Scanner::scan_collect_asm` 生成 LLVM IR。 [EXEC-VERIFIED] |
| **输出** | RISC-V 汇编文本文件（`.s`）。 |
| **失效模式** | `generate_asm` 返回 `Result<(), String>`；LLVM 阶段的错误向上传播。 [EXEC-VERIFIED] |

### 6.2 两遍架构

`src/riscv_codegen/mod.rs` 中的 `parse_llvm_ir` 对每个函数驱动两遍处理过程 [EXEC-VERIFIED]：

**第一遍 — 分析 / 区间收集（`FunctionState`）**：
- 记录每个基本块（basic block）起始的指令索引（`record_block_start`）。
- 检测 `alloca` 指令（`record_allocated_val`）。
- 检测后向分支以识别循环（`mark_loop`）。
- 对每个非终止符指令，记录 `(idx, result_name, vec[use_names])`（`record_instruction`）。
- 计算活跃区间（`compute_liveness`）：每个变量名的首次和末次使用。循环内携带的 alloca 变量其区间被扩展到函数末尾。
- 过滤掉全局变量名（它们固定为 `Location::Global`）。

**第二遍 — 指令发射**：
- 生成 `.text`、`.globl`、函数标签和函数序言（`addi sp, sp, -N`）。
- 为每个基本块发射标签。
- 将每条 LLVM 指令分发给生成器（`generate_instruction`）。
- 生成函数尾声（`addi sp, sp, N`）并在 `Return` 时生成 `li a7, 93; ecall`。 [EXEC-VERIFIED]

### 6.3 指令选择策略

后端通过 `generate_instruction` 中的模式匹配将 LLVM 操作码映射为 RISC-V 指令 [EXEC-VERIFIED]：

| LLVM 操作码 | RISC-V 发射 |
|-------------|-------------|
| `Add` / `Sub` / `Mul` / `SDiv` / `SRem` | `add` / `sub` / `mul` / `div` / `rem` |
| `ICmp(EQ/NE/SGT/SLT/SLE/SGE)` | `sub`+`seqz` / `sub`+`snez` / `sgt` / `slt` / `sle` / `sge` |
| `Load` | `la`+`lw`（全局）或 `lw` / `mv`（局部） |
| `Store` | `la`+`sw`（全局）或 `sw` / `mv`（局部） |
| `Br`（1 个操作数） | `j label` |
| `Br`（3 个操作数） | `bne cond, x0, true_label; j false_label` |
| `ZExt` | `mv` 或 `sw`（空操作，因为 RISC-V 寄存器已为 32 位） |
| `Return` | 将返回值加载到 `a0`，尾声，`li a7, 93; ecall` |

辅助函数 `get_value_from_reg` 和 `load_value_to_reg` 解析操作数位置（寄存器、栈偏移或全局标签）并发出必要的加载 / 移动指令。 [EXEC-VERIFIED]

### 6.4 寄存器分配

#### 分配器层次结构

位于 `src/riscv_codegen/register_alloc.rs` [EXEC-VERIFIED]：

| 类型 | 算法 |
|------|------|
| `NoAlloc` | 纯栈分配；重叠生命周期的变量获得不同的栈槽，非重叠生命周期的变量复用槽位。 |
| `LinearScan` | 线性扫描寄存器分配（linear-scan register allocation），支持溢出到栈（spill-to-stack）。 |
| `AllocatedInnerVar` | 分发器，当 `only_stack == true` 时选择 `NoAlloc`，否则选择 `LinearScan`。 |

#### 寄存器池

`LinearScan::new()` 按优先级顺序初始化寄存器池 [EXEC-VERIFIED]：

1. 临时寄存器：`t3`、`t4`、`t5`、`t6`（`t0`–`t2` 保留给操作数 / 结果临时变量）。
2. 保存寄存器：`s0`–`s11`。
3. 参数寄存器：`a2`–`a7`（`a0`–`a1` 保留给返回值 / 系统调用）。

#### 溢出策略

`LinearScan::overflow_to_stack` 实现了**溢出最远**启发式策略 [EXEC-VERIFIED]：

- 若活跃变量中结束偏移最晚者的结束位置在当前变量之后，则溢出该活跃变量并将其寄存器分配给当前变量。
- 否则，直接溢出当前变量。
- 被溢出的变量放置在负偏移的栈上（后续转换为自帧底起的正偏移）。

#### 栈对齐

序言 / 尾声大小按 16 字节对齐：`((stack_size + 15) / 16) * 16`。 [EXEC-VERIFIED]

### 6.5 AsmBuilder

`src/riscv_codegen/asm_builder.rs` 中的 `AsmBuilder` 是一个字符串缓冲区包装器，用于发出格式化的 RISC-V 文本 [EXEC-VERIFIED]：

- **段 / 符号指令**：`emit_data_section`、`emit_text_section`、`emit_global_symbol`、`emit_label`、`emit_word`。
- **函数帧**：`emit_function_prologue`、`emit_function_epilogue`、`emit_exit_syscall`。
- **ALU**：`emit_add`、`emit_sub`、`emit_mul`、`emit_div`、`emit_rem`、`emit_addi`、`emit_subi`。
- **内存**：`emit_lw`、`emit_sw`。
- **分支 / 比较**：`emit_beq`、`emit_bne`、`emit_blt`、`emit_bgt`、`emit_ble`、`emit_bge`、`emit_slt`、`emit_sgt`、`emit_seqz`、`emit_snez`、`emit_sle`、`emit_sge`。
- **跳转**：`emit_j`、`emit_jal`、`emit_jr`。
- **移动**：`emit_mv`。
- **系统调用**：`emit_ecall`、`emit_syscall`。
- **检索缓冲区**：`emit()` 返回累积的 `String`。 [EXEC-VERIFIED]

### 6.6 不变式与失效模式

- **不变式**：全局变量从不传递给寄存器分配器；它们保持为 `Location::Global`。 [EXEC-VERIFIED]
- **不变式**：`t0`、`t1`、`t2` 从不分配给用户 SSA 值；它们保留给操作数加载和临时结果。 [STATIC-INFERRED]
- **失效模式**：若 LLVM 阶段失败，不发出任何汇编。 [EXEC-VERIFIED]
- **失效模式**：`generate_instruction` 中未知的 LLVM 操作码被静默跳过（不触发 panic）。 [EXEC-VERIFIED]

---

## 7. CLI 功能设计（`src/main.rs`）

### 7.1 子命令规范

CLI 使用 `clap` 构建，公开以下接口 [EXEC-VERIFIED]：

| 子命令 | 参数 | 行为 |
|--------|------|------|
| `tokenize` | `file: String` | 读取文件，调用 `lexer::tokenize(&input)` 写入 `stderr`。 |
| `fmt` | `file: String` | 读取文件，调用 `format::fmt(&input)` 写入 `stdout`。 |
| `check` | `file: String` | 读取文件，创建 `Checker`，运行 `syn_check()`，将错误写入 `stderr`。 |
| `gen-ir` | `input: String`、`--output <String>` | 读取输入，创建 `Scanner`，运行 `scan_collect()` 写入 LLVM IR。 |
| `gen-asm` | `input: String`、`--output <String>` | 读取输入，运行 `generate_asm()` 写入 RISC-V 汇编。 |

### 7.2 兼容模式

若二进制文件以两个裸位置参数调用（无子命令），且第一个参数不以 `-` 开头且不是已知的子命令名称，则将其视为 `gen-asm <input> <output>` [EXEC-VERIFIED]：

```rust
if args.len() >= 3
    && !args[1].starts_with('-')
    && args[1] != "tokenize"
    && ...
    && !args[2].starts_with('-')
{
    run_gen_asm(&args[1], &args[2]);
    return;
}
```

这同样在 `Cli` 中通过 `input: Option<String>` 和 `output: Option<String>` 镜像实现，由 clap 在没有子命令时解析。 [EXEC-VERIFIED]

### 7.3 错误处理与日志

- **tklog**：`log_init()` 配置 `tklog::LOG`，输出到控制台，级别为 `Info`，格式化器显示级别、时间、短文件名和消息。 [EXEC-VERIFIED]
- **致命错误**：文件读取失败和后端错误调用 `eprintln!` 和 `std::process::exit(1)`。 [EXEC-VERIFIED]
- **静默假设**：`gen_llvm_ir.rs` 和 `riscv_codegen/` 在生产路径中不向 `stderr` 写入，以避免在线评测系统（Online Judge）输出污染。 [STATIC-INFERRED]

### 7.4 关键函数

- `read_file(path: &str) -> String` — 读取文件，否则以代码 1 退出。 [EXEC-VERIFIED]
- `run_gen_asm(input: &str, output: &str)` — 初始化日志并驱动 `generate_asm`。 [EXEC-VERIFIED]
- `main()` — 解析 CLI，分发给子命令，处理兼容模式。 [EXEC-VERIFIED]

---

## 8. 测试功能设计

### 8.1 测试组织

测试以夹具文件（fixture files）加预期输出（如适用）的形式组织在 `tests/` 下 [EXEC-VERIFIED]：

| 目录 | 用途 | 验证方法 |
|------|------|----------|
| `tests/lexer/` | `.in` 源文件 + `.out` 预期词法单元 / 错误输出 | `src/lexer.rs` 中的单元测试将缓冲区输出与 `.out` 文件比较。 |
| `tests/formatter/` | `.txt` 输入文件 | `src/format.rs` 中的单元测试运行格式化并断言成功。 |
| `tests/semantic/` | `.sy` 源文件 + `.out` 预期错误输出 | `src/check.rs` 中的单元测试将 `Checker` 输出与 `.out` 文件比较。 |
| `tests/llvm_ir/` | `.sy` 源文件 + `.ll` 官方 / 参考 IR + `_out.ll` 生成的 IR | `src/gen_llvm_ir.rs` 中的单元测试通过 `lli-14` 比较执行结果。 |
| `tests/codegen/` | `.sy` 源文件 + `target_*.s` 基准汇编 | `tests/end_to_end_codegen.rs` 中的端到端 Rust 测试使用 RARS 与基准比较。 |

### 8.2 端到端验证策略（RARS 模拟器）

`tests/end_to_end_codegen.rs` 集成测试套件使用 RARS 模拟器（`tests/codegen/rars.jar`）验证 RISC-V 后端 [EXEC-VERIFIED]：

1. **编译**：使用编译器二进制文件（`env!("CARGO_BIN_EXE_COMPILER")`）传入输入 / 输出路径以生成 `.s` 文件。 [EXEC-VERIFIED]
2. **模拟**：运行 `timeout 5s java -jar rars.jar <asm> a0` 执行生成的程序并捕获 `a0` 寄存器值。 [EXEC-VERIFIED]
3. **正确性检查**：
   - `codegen_program_terminates_with_correct_result`：生成汇编与目标汇编必须对欧几里得算法夹具均产生 `a0 == 0x0000000e`（14）。 [EXEC-VERIFIED]
   - `codegen_program_does_not_timeout`：断言模拟器不返回退出代码 124（`timeout`）。 [EXEC-VERIFIED]
   - `codegen_control_flow_branches_correct`：检查标签（`whileCond`、`whileBody`、`if_true`、`if_false`）以验证分支目标。 [EXEC-VERIFIED]
4. **性能预算**：
   - `codegen_stack_frame_within_budget`：生成的栈帧大小不得超过基准 `target_register_alloc.s`。 [EXEC-VERIFIED]
   - `codegen_load_store_count_within_budget`：生成的加载 / 存储次数不得超过基准 + 20。 [EXEC-VERIFIED]

### 8.3 LLVM IR 验证

在 `src/gen_llvm_ir.rs`（测试模块）内部，测试辅助函数 `execute_llvm_ir` 对生成的 `.ll` 文件运行 `lli-14`。测试要么与官方 `.ll` 文件比较（`compare_execution_results`），要么与源文件中嵌入的 `// output N` 注释所表示的预期返回代码比较（`compare_with_expected_output`）。 [EXEC-VERIFIED]

### 8.4 不变式与失效模式

- **不变式**：所有 `.out` 夹具文件使用平台适配的换行符（测试在比较前将 `\r\n` 规范化为 `\n`）。 [EXEC-VERIFIED]
- **失效模式**：若缺少 `lli-14` 或 RARS，对应的集成测试将在运行时失败。 [STATIC-INFERRED]
- **失效模式**：忽略预期输出比较的单元测试（例如许多格式化器测试）仅断言格式化不会 panic；它们不验证文本正确性。 [PARTIAL]

---

## 证据图例

- **[EXEC-VERIFIED]** — 行为通过仓库中的源代码、单元测试或集成测试的直接检查确认。
- **[STATIC-INFERRED]** — 行为通过源代码结构、注释或跨模块调用图推导得出，未经显式运行时验证。
- **[PARTIAL]** — 行为已部分实现或部分测试；边界情况可能未完全覆盖。
