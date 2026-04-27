---
name: 实现参考
description: SysY→RISC-V 编译器 (compiler) 的综合实现指南
commit: f5b8eb08d74b85cdf4c78c4ecfdae4880dcfeb1c
---

# 实现参考

本文档描述 SysY→RISC-V 编译器 (compiler) 的内部架构、数据结构、算法以及构建/测试流程。所有行号引用均固定于提交 `f5b8eb08d74b85cdf4c78c4ecfdae4880dcfeb1c`。

---

## 1. 环境搭建

### 前置条件

- **Rust 工具链** — 最新 stable 频道（`rustc` + `cargo`）。
- **LLVM 14** 开发库。
- **Java** 运行时（用于端到端测试中的 RARS 模拟器）。

### LLVM 14 安装

```bash
sudo apt-get update
sudo apt-get install llvm-14-dev libpolly-14-dev zlib1g-dev
```

> 有关最新的操作系统特定说明，请参见 `README.md`。

### 仓库结构

```
src/
├── main.rs                      # CLI 入口点
├── lexer.rs                     # 分词器 (tokenizer)
├── format.rs                    # 代码格式化器
├── check.rs                     # 语义检查器 (semantic checker)
├── gen_llvm_ir.rs               # LLVM IR 生成器
├── riscv_codegen/
│   ├── mod.rs                   # RISC-V 代码生成驱动
│   ├── register_alloc.rs        # 线性扫描分配器
│   └── asm_builder.rs           # 汇编文本构建器
├── utils.rs                     # 十六进制/八进制转换辅助函数
└── tests/                       # 单元测试与集成测试 (integration tests)
```

---

## 2. 构建与运行

### 发布构建

```bash
cargo build --release
```

生成的二进制文件位于 `target/release/compiler`。

### CLI 子命令

| 子命令 | 用途 | 示例调用 |
|--------|------|----------|
| `tokenize` | 词法分析 → token 流 | `cargo run -- tokenize tests/lexer/simple.in` |
| `fmt` | 美化打印 SysY 源码 | `cargo run -- fmt tests/formatter/input1.txt` |
| `check` | 语义分析（类型 + 作用域 (scope)） | `cargo run -- check tests/semantic/normaltest01.sy` |
| `gen-ir` | 输出 LLVM IR | `cargo run -- gen-ir tests/llvm_ir/example01.sy -o out.ll` |
| `gen-asm` | 输出 RISC-V 汇编 | `cargo run -- gen-asm tests/codegen/register_alloc.sy -o out.s` |

### 兼容模式

省略子命令并传入两个位置参数时，默认使用 `gen-asm`：

```bash
cargo run -- tests/codegen/register_alloc.sy out.s
```

调度逻辑位于 <code>src/main.rs:85-142</code>。

---

## 3. 核心数据结构

### 3.1 Token 及相关枚举

**文件：** <code>src/lexer.rs:9-182</code>

```rust
// src/lexer.rs:9-11
#[derive(Parser)]
#[grammar = "pests/lexer.pest"]
pub struct ExpressionParser;

// src/lexer.rs:14-21
pub enum Token {
    Identifier(String),
    Operator(Operator),
    Type(Type),
    Flow(Flow),
    IntegerConst(IntegerConst),
    ErrorSyntax(ErrorSyntax)
}
```

- **`Token`** — 统一的词法 token。每个变体包裹一个更具体的枚举。
- **`Operator`** (<code>src/lexer.rs:128-152</code>) — 从 `+` 到 `;` 共 21 个运算符。
- **`Flow`** (<code>src/lexer.rs:86-93</code>) — 控制流关键字（`if`、`else`、`while`、`break`、`continue`、`return`）。
- **`Type`** (<code>src/lexer.rs:109-114</code>) — `int`、`float`、`void`、`const`。
- **`IntegerConst`** (<code>src/lexer.rs:62-66</code>) — 十进制、八进制或十六进制整数字面量。
- **`ErrorSyntax`** (<code>src/lexer.rs:39-46</code>) — 词法错误变体（无效十六进制、无效八进制、未知运算符等）。

### 3.2 格式化器 (Formatter)

**文件：** <code>src/format.rs:20-29</code>

```rust
pub struct Formatter<'a, W: Write> {
    deep: usize,                       // 当前缩进深度
    input: &'a str,                    // 源文本
    writer: &'a mut W,                 // 输出目标
    output: String,                    // 累积的格式化文本
    err_output: String,                // 语法错误信息
    forbid_newline: bool,              // 抑制换行（例如在 Decl 内部）
    forbid_semicolon_newline: bool,    // 抑制分号后的换行
    is_first_function: bool,           // 控制函数前的空行
}
```

### 3.3 检查器 (Checker) 与语义类型

**文件：** <code>src/check.rs:47-58</code>

```rust
pub struct Checker<'a, W: Write> {
    input: &'a str,                    // 源文本
    writer: &'a mut W,                 // 诊断输出
    output: String,                    // 缓冲的诊断信息
    scope_stack: ScopeStack,           // 嵌套作用域 (scope) 管理器
    errors: Vec<SemanticError<'a>>,    // 收集的语义错误
}
```

**文件：** <code>src/check.rs:1016-1021</code>

```rust
pub struct ScopeStack {
    stack: Vec<ScopeKey>,              // 作用域 (scope) 键栈
    scopes: HashMap<ScopeKey, Scope>,  // 键 → 符号表 (symbol table) 的映射
}
```

**文件：** <code>src/check.rs:1137-1142</code>

```rust
pub enum Type {
    Int,
    Func(Func),
    Void,
    Array(ArrayStruct),
}
```

**文件：** <code>src/check.rs:1180-1186</code>

```rust
pub struct Func {
    pub params: Vec<Type>,
    pub return_type: Box<Type>,
}
```

**文件：** <code>src/check.rs:1200-1211</code>

```rust
pub struct ArrayStruct {
    pub item_type: Box<Type>,
    pub dim_nos: Vec<usize>,
    pub tpl_size: HashMap<usize, usize>,
    pub array_dims: usize,
    pub crr_no: usize,
}
```

**文件：** <code>src/check.rs:1275-1300</code>

```rust
pub enum ErrorKind {
    UndefinedVal = 1,   UndefinedFunc = 2,
    RedefineVal  = 3,   RedefineFunc  = 4,
    TypeMismatch = 5,   TypeMismatchOp = 6,
    ReturnMismatch = 7, Inappropriate = 8,
    NotArrayAssign = 9, UnlegalFuncCall = 10,
    UnexpectedFuncAssign = 11,
    Other = 0,
}
```

### 3.4 IR 生成类型

**文件：** <code>src/gen_llvm_ir.rs:58-63</code>

```rust
pub struct Scanner<'a> {
    input: &'a str,
    output: &'a str,
    ir_core: IrCore,
}
```

`Scanner` 是 LLVM IR 生成的顶层驱动。它使用 `pests/scan.pest` 解析 SysY 源码，遍历 AST，并通过 `inkwell` 输出 LLVM 指令。

**文件：** <code>src/gen_llvm_ir.rs:1780-1789</code>

```rust
pub struct IrCore {
    context: Context,
}

impl IrCore {
    pub fn start_session(&self, module_name: &str) -> IrSession<'_> { ... }
}
```

**文件：** <code>src/gen_llvm_ir.rs:1803-1808</code>

```rust
pub struct IrSession<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    pub scope_stack: ScopeStack<'ctx>,
}
```

`IrSession` 将 LLVM 上下文 (context)、模块 (module)、IR 构建器 (builder) 和符号表 (symbol table) 打包为一个编译单元。

**文件：** <code>src/gen_llvm_ir.rs:1835-1840</code>

```rust
pub struct ScopeStack<'ctx> {
    stack: Vec<ScopeKey<'ctx>>,
    scopes: HashMap<ScopeKey<'ctx>, Scope<'ctx>>,
}
```

**文件：** <code>src/gen_llvm_ir.rs:1961-1965</code>

```rust
pub enum Type<'ctx> {
    GlobalVar(PointerValue<'ctx>),
    Func(FunctionValue<'ctx>),
    LocalVar(PointerValue<'ctx>),
}
```

### 3.5 RISC-V 代码生成类型

**文件：** <code>src/riscv_codegen/mod.rs:67-74</code>

```rust
pub struct GenContext {
    var_locations: HashMap<String, Location>,
    stack_size: usize,
    global_names: Vec<String>,
    temp_val_num: usize,
    allocated_vals: HashSet<String>,
    pure_stack_mode: bool,
}
```

- `var_locations` — 将 LLVM 值名映射到 `Location`（寄存器、栈槽或全局标签）。
- `pure_stack_mode` — 为 `true` 时绕过寄存器分配，将所有变量放在栈上。

**文件：** <code>src/riscv_codegen/mod.rs:199-205</code>

```rust
struct FunctionState {
    instructions: Vec<(usize, String, Vec<String>)>,
    allocated_vals: HashSet<String>,
    block_start_idxes: HashMap<String, usize>,
    loop_branch: bool,
    idx: usize,
}
```

`FunctionState` 是一个按函数累积的器，在**第一遍**中用于收集 def/use 信息并检测后向分支（循环）。

**文件：** <code>src/riscv_codegen/register_alloc.rs:8-12</code>

```rust
pub struct InnerVar {
    name: String,
    start_offset: usize,
    end_offset: usize,
}
```

按指令索引表示变量的活跃区间 (live interval)。

**文件：** <code>src/riscv_codegen/register_alloc.rs:49-53</code>

```rust
pub enum Location {
    Reg(String),
    Stack(i32),
    Global(String),
}
```

**文件：** <code>src/riscv_codegen/register_alloc.rs:137-150</code>

```rust
pub struct LinearScan {
    regs: Vec<String>,              // 空闲寄存器池
    active_vars: BTreeSet<InnerVar>,
    var_reg_map: HashMap<String, String>,
    stack_offset: i32,
    stack_slots: Vec<(i32, String)>,
    stack_map: HashMap<String, (i32, String)>,
}
```

### 3.6 AsmBuilder

**文件：** <code>src/riscv_codegen/asm_builder.rs:2-4</code>

```rust
pub struct AsmBuilder {
    buf: String,
}
```

`AsmBuilder` 是对 `String` 的轻量级包装。它为后端使用的每条 RISC-V 伪指令和指令提供 `emit_*` 方法（例如 `emit_add`、`emit_lw`、`emit_function_prologue`）。

---

## 4. 关键算法

### 4.1 词法分析流水线

**文件与范围：**
- <code>src/lexer.rs:184-202</code> — `parse_file`
- <code>src/lexer.rs:204-234</code> — `tokenizer`
- <code>src/lexer.rs:241-321</code> — `impl From<Pair<Rule>> for Token`

**描述：**
该词法分析器是一个**基于解析器生成器 (parser generator) 的分词器**，而非手写 DFA。`pest` 语法 `pests/lexer.pest` 定义了 token 规则。`ExpressionParser::parse(Rule::File, input)` 生成一棵 `Pair` 树，随后被展平为 `Vec<(usize, Token)>`。

**分步说明：**
1. 调用 `parse_file(input)` → 在整个文件上调用 `pest` 解析器。
2. 解包顶层 `File` pair 并遍历其子节点（<code>src/lexer.rs:209-220</code>）。
3. 跳过 `Rule::EOI`；对于其他每个 pair，读取 `pair.line_col().0` 并通过 `Token::from(pair)` 转换。
4. 如果存在任何 `ErrorSyntax` token，则打印错误诊断；否则打印 token 流（<code>src/lexer.rs:221-233</code>）。
5. `From<Pair>` 根据 `Rule`（`Operator`、`Flow`、`Type`、`IntegerConst`、`ErrorSyntax`）进行分发，并递归一层以提取具体词素。

### 4.2 格式化递归下降 (recursive descent)

**文件与范围：**
- <code>src/format.rs:45-228</code> — `Formatter::fmt`
- <code>src/format.rs:369-392</code> — `Formatter::format_code`
- <code>src/format.rs:280-367</code> — 空行清理

**描述：**
`Formatter` 遍历 `pest` 解析树（`pests/parser.pest`）并输出美化后的 SysY 源码。它**不是**独立的 AST 美化打印器；它直接从 `Pair<Rule>` 节点进行格式化。

**分步说明：**
1. `format_code` 解包嵌套的 `File → CompUnit` 结构，并对每个顶层声明调用 `self.fmt(pair)`（<code>src/format.rs:369-383</code>）。
2. `fmt` 根据 `Rule` 匹配：
   - 容器节点（`FuncDef`、`VarDecl`、`Block` 等）递归进入子节点。
   - `Rule::Stmt` 处理 `if` / `else` 体的特殊缩进规则（<code>src/format.rs:94-133</code>）。
   - `Rule::OpenBrace` 增加 `deep` 并追加换行 + 缩进；`Rule::CloseBrace` 减少 `deep`。
   - 运算符两侧加空格；分号触发换行插入，除非被抑制。
3. 遍历完成后，`clean_extra_blank_lines` 移除冗余空行，同时保留函数定义之间的单个空行（<code>src/format.rs:280-367</code>）。

### 4.3 语义检查遍 (passes)

**文件与范围：**
- <code>src/check.rs:75-96</code> — `syn_check`
- <code>src/check.rs:108-124</code> — `analyze_declaration`
- <code>src/check.rs:130-142</code> — `analyze_var_decl`
- <code>src/check.rs:144-193</code> — `analyze_func_def`
- <code>src/check.rs:256-305</code> — `analyze_assign_stmt`
- <code>src/check.rs:307-405</code> — 表达式类型检查（`analyze_exp` … `analyze_unary_op_exp`）
- <code>src/check.rs:407-486</code> — `analyze_call_exp`

**描述：**
检查器对 `pests/check.pest` 解析树执行**单遍递归下降 (recursive descent)**。它维护一个 `ScopeStack`（全局 → 函数 → 内部块）并收集 11 类语义错误。

**分步说明：**
1. `syn_check` 解析文件，解包 `CompUnit`，并遍历声明（<code>src/check.rs:76-88</code>）。
2. 对每个声明：
   - `VarDecl` / `ConstDecl` → `analyze_var_decl` → `analyze_def`。
   - `FuncDef` → 压入函数作用域 (scope)，解析参数，解析函数体，弹出作用域 (scope)（<code>src/check.rs:144-193</code>）。
3. 表达式自下而上进行类型检查：
   - `analyze_add_exp` / `analyze_mul_exp` 验证两个操作数均为 `Int`（或兼容数组）并传播 `Type::Int`。
   - `analyze_call_exp` 验证函数是否存在、参数数量，并检查参数类型（<code>src/check.rs:407-486</code>）。
4. 错误被收集到 `Vec<SemanticError>` 中，最后按源码顺序输出（<code>src/check.rs:98-106</code>）。

### 4.4 两阶段 IR 生成

**文件与范围：**
- <code>src/gen_llvm_ir.rs:122-156</code> — `scan_collect`
- <code>src/gen_llvm_ir.rs:377-482</code> — `scan_func_def`
- <code>src/gen_llvm_ir.rs:663-766</code> — `scan_if_stmt`
- <code>src/gen_llvm_ir.rs:768-816</code> — `scan_while_stmt`
- <code>src/gen_llvm_ir.rs:850-928</code> — 算术表达式降层

**描述：**
IR 生成**并未拆分为前端 AST + 后端 IR**；它直接从 `pest` pair 降层为 LLVM 指令，使用 `inkwell`。

**分步说明：**
1. `scan_collect` 创建一个 `IrSession`（上下文 + 模块 + 构建器 + 作用域 (scope) 栈）并遍历声明（<code>src/gen_llvm_ir.rs:135-142</code>）。
2. `scan_func_def`：
   - 从 `Rule::Int` / `Rule::Void` 构建 LLVM 函数类型。
   - 将函数添加到模块；将其存入当前作用域 (scope)。
   - 压入函数作用域 (scope)；追加入口 `BasicBlock`。
   - 通过 `build_alloca` 为参数分配栈槽并存储参数值。
   - 遍历函数体；退出时如果基本块 (basic block) 缺少终结符，则插入默认 `ret`（<code>src/gen_llvm_ir.rs:468-479</code>）。
3. 控制流：
   - `if` 创建 `if_true`、`if_false`、`if_next` 基本块 (basic block)；使用 `scan_cond_for_branches` 求值条件并发出 `br`（<code>src/gen_llvm_ir.rs:663-766</code>）。
   - `while` 创建 `whileCond`、`whileBody`、`whileNext`；发出无条件分支到条件块，然后发出条件分支到体块/后继块（<code>src/gen_llvm_ir.rs:768-816</code>）。
4. 表达式：
   - `scan_add_exp` / `scan_mul_exp` 遍历左结合链并发出 `build_int_add`、`build_int_sub`、`build_int_mul`、`build_int_signed_div`、`build_int_signed_rem`（<code>src/gen_llvm_ir.rs:850-928</code>）。

### 4.5 线性扫描寄存器分配 (Linear Scan Register Allocation)

**文件与范围：**
- <code>src/riscv_codegen/register_alloc.rs:266-289</code> — `clear_inactive_vars`
- <code>src/riscv_codegen/register_alloc.rs:319-339</code> — `overflow_to_stack`
- <code>src/riscv_codegen/register_alloc.rs:410-450</code> — `LinearScan::allocate`

**描述：**
分配器接收一组 `InnerVar` 活跃区间 (live interval)（在 <code>src/riscv_codegen/mod.rs:256-296</code> 的 `FunctionState::compute_liveness` 中计算），并将每个变量映射到物理寄存器或栈槽。

**伪代码：**
```
proc allocate(intervals):
    sort intervals by start_offset ascending
    for var in intervals:
        clear_inactive_vars(var)          // 释放结束位置 < var.start 的寄存器 / 栈槽
        if free_registers not empty:
            reg = pop_reg()                // 优先级：t3-t6 > s0-s11 > a2-a7
            assign var → reg
            active_vars.insert(var)
        else:
            overflow_to_stack(var)         // 溢出结束位置最远的活跃区间
    convert negative stack offsets to positive
    return (var→location_map, stack_size)
```

- **溢出启发式 (spill heuristic)**（`overflow_to_stack`）：将结束位置最新的活跃变量与当前变量比较。如果活跃变量结束更晚，则将其溢出并把寄存器分配给当前变量；否则溢出当前变量。
- **寄存器优先级**：优先使用临时寄存器 (temporary registers) `t3–t6`，然后是保存寄存器 (saved registers) `s0–s11`，最后是参数寄存器 (argument registers) `a2–a7`（`a0–a1` 保留用于返回值）。

### 4.6 RISC-V 指令降层（两遍）

**文件与范围：**
- <code>src/riscv_codegen/mod.rs:42-64</code> — `parse_llvm_ir`
- <code>src/riscv_codegen/mod.rs:321-459</code> — `build_function`
- <code>src/riscv_codegen/mod.rs:601-635</code> — `generate_instruction`
- <code>src/riscv_codegen/mod.rs:256-296</code> — `FunctionState::compute_liveness`

**描述：**
后端对每个函数执行**两遍**处理。

**第一遍 — 分析：**
1. 枚举基本块 (basic block) 并记录每个块的起始指令索引（<code>src/riscv_codegen/mod.rs:346-354</code>）。
2. 遍历每条指令：
   - 在 `allocated_vals` 中记录 `alloca` 名称。
   - 通过将分支目标索引与当前基本块 (basic block) 起始索引比较来检测后向分支（循环）（<code>src/riscv_codegen/mod.rs:466-525</code>）。
   - 跳过终结符（`ret`、`br`）。
   - 为所有其他指令记录 `(idx, def_name, use_names)`（<code>src/riscv_codegen/mod.rs:386-392</code>）。
3. 计算活跃性 (liveness)：对每个变量，`first_use` 和 `last_use` 指令索引构成活跃区间 (live interval)。如果存在循环，则 `alloca` 变量被扩展到函数最大索引（<code>src/riscv_codegen/mod.rs:256-296</code>）。
4. 过滤掉全局名称（它们被预映射到 `Location::Global`）并运行寄存器分配 (register allocation)（<code>src/riscv_codegen/mod.rs:400-417</code>）。

**第二遍 — 发射：**
1. 发射 `.text`、`.globl`、函数标签和序言（`addi sp, sp, -N`）（<code>src/riscv_codegen/mod.rs:427-443</code>）。
2. 对每条指令调用 `generate_instruction`，它根据 `InstructionOpcode` 分发：
   - `Add/Sub/Mul/SDiv/SRem` → `generate_cal_instruction`（<code>src/riscv_codegen/mod.rs:1050-1148</code>）
   - `ICmp` → `generate_icmp_instruction`（<code>src/riscv_codegen/mod.rs:1295-1416</code>）
   - `Load` → `generate_load_instruction`（<code>src/riscv_codegen/mod.rs:721-792</code>）
   - `Store` → `generate_store_instruction`（<code>src/riscv_codegen/mod.rs:1191-1241</code>）
   - `Br` → `generate_br_instruction`（<code>src/riscv_codegen/mod.rs:854-892</code>）
   - `ZExt` → `generate_zext_instruction`（<code>src/riscv_codegen/mod.rs:952-980</code>）
   - `Return` → 将结果加载到 `a0`，发射尾声，`li a7, 93; ecall`（<code>src/riscv_codegen/mod.rs:638-664</code>）

---

## 5. 依赖说明

| Crate | 版本 | 角色 | 说明 |
|-------|------|------|------|
| **pest** | 2.8.1 | 解析器生成器 (parser generator) | 为避免为五个独立的实验阶段手写词法分析器/解析器而选择。PEG 语法（`pests/*.pest`）简洁且易于维护。 |
| **pest_derive** | 2.8.1 | pest 的派生宏 | pest 在编译时生成 `Parser` 实现所必需的。 |
| **inkwell** | 0.6.0 (llvm14-0) | LLVM IR 构建器 | 基于 LLVM-C API 的安全 Rust 绑定。无需编写原始 IR 文本即可构建 SSA、基本块 (basic block) 和指令发射。 |
| **clap** | 4.5.45 | CLI 参数解析 | 基于派生（`#[derive(Parser, Subcommand)]`）使 `main.rs` 简短且自文档化。 |
| **num-bigint** | 0.4.6 | 任意精度整数 | SysY 允许在解析期间可能超出 `i32` 的十六进制和八进制常量；`BigInt` 在 `utils.rs:4-20` 中用于截断前的安全转换。 |
| **num-traits** | 0.2.19 | num-bigint 的特质 | num-bigint 进行进制解析所必需的配套库。 |
| **tklog** | 0.3.0 | 结构化日志 | 在 RISC-V 代码生成中用于开发时的跟踪输出（文件 + 行号元数据）。 |
| **log** | 0.4.29 | 日志门面 | tklog 消费的标准接口。 |

---

## 6. 已知技术债务 (Known Debt)

### 6.1 超大模块

| 文件 | 行数 | 问题 |
|------|------|------|
| <code>src/gen_llvm_ir.rs</code> | 2,250 | 将解析、作用域 (scope) 管理、类型检查和 LLVM 发射混合在一个文件中。 |
| <code>src/check.rs</code> | 1,621 | 语义分析、符号表 (symbol table) 和错误格式化全部内联在一起。 |
| <code>src/riscv_codegen/mod.rs</code> | 2,069 | 包含活跃性分析 (liveness analysis)、寄存器分配 (register allocation) 编排和按操作码降层。 |

### 6.2 检查器与 IR 生成之间的紧耦合

**不存在共享 AST**。`check.rs` 和 `gen_llvm_ir.rs` 都维护独立的 `pest` 语法（`check.pest` 和 `scan.pest`）并独立遍历 `Pair` 树。这意味着：
- 语法变更必须手动保持同步。
- 检查器无法复用 IR 生成的符号表 (symbol table)，反之亦然。
- 语义分析期间计算的类型信息被丢弃；IR 生成从解析树形状重新推导一切。

### 6.3 TODO / FIXME

- <code>src/riscv_codegen/mod.rs:606</code>  
  `// TODO: 根据instruction类型生成相应代码` — `generate_instruction` 内部的占位注释；实际分发已在其实现下方完成，因此该 TODO 已过时。
- <code>src/check.rs:810-816</code>  
  `// TODO 实验阶段 暂时不处理` — 数组初始化维度检查被注释掉。

### 6.4 缺少 `lib.rs` / 纯二进制架构

<code>src/main.rs</code> 使用 `mod check; mod format; …` 声明模块并包含所有 CLI 逻辑。**不存在 `lib.rs`**，这意味着：
- 该编译器 (compiler) 无法作为库 crate 被消费。
- `tests/` 中的集成测试 (integration tests) 必须调用编译后的二进制文件（`env!("CARGO_BIN_EXE_compiler")`）而非链接到库。
- 内部单元测试分散在每个源文件的 `#[cfg(test)]` 块中。

### 6.5 临时 / 硬编码结构

- `gen_llvm_ir.rs` 将 `i32` 硬编码为唯一的整数类型；SysY `float` 存在于词法分析器中但未被降层。
- `LinearScan` 将 `t0–t2` 保留为临时寄存器，从不将它们分配给变量，从而减少了有效寄存器池。
- `AsmBuilder::emit_subi`（<code>src/riscv_codegen/asm_builder.rs:68-70</code>）发射一条可能不被所有 RISC-V 汇编器支持的伪指令；它在生成的代码中当前未被使用。

---

## 7. 测试执行指南

### 7.1 单元测试

每个主要模块都包含内联的 `#[cfg(test)]` 测试。

```bash
# 运行所有单元测试
cargo test

# 运行特定模块的测试
cargo test --lib lexer
cargo test --lib check
cargo test --lib gen_llvm_ir
cargo test --lib riscv_codegen
```

### 7.2 端到端代码生成测试

集成测试 (integration test) 套件 <code>tests/end_to_end_codegen.rs</code> 验证完整的 `SysY → RISC-V 汇编 → RARS` 流水线。

**前置条件：**
- `$PATH` 上有 `java`
- <code>tests/codegen/rars.jar</code> 存在

**运行：**

```bash
cargo test --test end_to_end_codegen -- --nocapture
```

**检查内容：**
1. **栈帧预算**（`codegen_stack_frame_within_budget`）— 生成的 `addi sp, sp, -N` 不得超过参考基线。
2. **加载/存储流量预算**（`codegen_load_store_count_within_budget`）— `lw`/`sw` 计数必须保持在目标值 + 20 以内。
3. **正确的运行时结果**（`codegen_program_terminates_with_correct_result`）— 编译 `euclidean_algorithm.sy`，在 RARS 中运行，并断言 `a0 == 0x0000000e`（14）。
4. **不超时**（`codegen_program_does_not_timeout`）— RARS 必须在 5 秒内完成。
5. **控制流分支目标**（`codegen_control_flow_branches_correct`）— 检查汇编文本以确保 `whileCond` 分支到 `whileBody` 和 `whileNext`，且循环内的 `if` 分支到 `if_true` / `if_false`。

### 7.3 手动 RARS 验证

```bash
cargo run -- gen-asm tests/codegen/euclidean_algorithm.sy -o /tmp/euclid.s
java -jar tests/codegen/rars.jar /tmp/euclid.s a0
```

在 RARS 输出窗口中查找 `a0`；应显示 `0x0000000e`。

---

*文档为提交 `f5b8eb08d74b85cdf4c78c4ecfdae4880dcfeb1c` 生成。*
