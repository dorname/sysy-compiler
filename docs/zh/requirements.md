---
title: SysY 编译器需求
date: 2026-04-27
version: 0.1.0
---

# SysY 编译器需求

本文档规定了基于 Rust 实现的 SysY→RISC-V 教学编译器的功能需求与非功能需求。基线源码版本记录在 `docs/.pin` 中。

## 1. 支持的 SysY 语言子集

该编译器接受 SysY 语言的一个子集，其文法定义位于 `src/pests/lexer.pest`、`src/pests/parser.pest` 和 `src/pests/check.pest` 中。

### 1.1 类型

仅支持 `int` 和 `void` 类型。词法分析器（Lexer）会对 `float` 进行分词，但语法分析器（Parser）和语义检查器不接受浮点类型声明（`src/pests/lexer.pest`、`src/pests/check.pest`）。

### 1.2 变量与常量

标量变量和常量可以在文件作用域或块作用域中声明。声明形式为 `int x;`、`const int x = 1;` 或 `int x = 1;`。数组声明支持常量维度，例如 `int a[2][3];`。数组参数可以省略第一维，例如 `int f(int a[][3])`。这些结构在 `tests/semantic/normaltest01.sy`、`tests/codegen/register_alloc.sy` 和 `tests/llvm_ir/edge_case_01.sy` 中进行了验证。

### 1.3 函数

函数由返回类型（`int` 或 `void`）、名称、可选参数列表和块体定义。函数调用属于表达式。示例见 `tests/semantic/example01.sy` 和 `tests/codegen/euclidean_algorithm.sy`。

### 1.4 语句

支持的语句包括：
- 表达式语句（可为空 `;`）
- 赋值语句（`LVal = Exp;`）
- 块语句（`{ ... }`）
- `if`/`else` 条件语句
- `while` 循环
- `break` 和 `continue`
- 带可选表达式的 `return`

这些语句在 `tests/codegen/euclidean_algorithm.sy`、`tests/llvm_ir/example07.sy` 以及语义测试套件 `tests/semantic/` 中进行了验证。

### 1.5 表达式

表达式包括算术运算（`+`、`-`、`*`、`/`、`%`）、关系运算（`<`、`<=`、`>`、`>=`）、相等性运算（`==`、`!=`）、逻辑运算（`&&`、`||`、`!`）、一元运算（`+`、`-`、`!`）、带括号的子表达式、数组元素访问（`a[i][j]`）以及函数调用。该子集由 `tests/llvm_ir/edge_case_01.sy` 和 `tests/lexer/arrays_and_radix.in` 覆盖。

### 1.6 字面量与注释

支持十进制、八进制（`0...`）和十六进制（`0x...`）的整数字面量（`tests/lexer/comments_and_hex.in`）。行注释（`//`）和块注释（`/* */`）由词法分析器剔除（`tests/lexer/comments_and_hex.in`）。

### 1.7 错误恢复

语法分析器执行错误同步：在发生语法错误时，它会跳过到下一个 `;` 或 `}` 并继续解析，从而可以在一次遍历中报告多个错误（`src/pests/check.pest`）。

## 2. CLI 子命令

该编译器是一个以 `sysy-compiler` 调用的二进制包（binary crate）。所有行为均由 `src/main.rs` 分发。

### 2.1 `tokenize`
**用途：** 将 SysY 源码转换为词元（token）流。
**输入：** 单个文件路径参数（`tokenize <file>`）。
**输出：** 词元行写入标准错误。每行格式为 `<TOKEN> at Line <N>.`，或者当存在词法错误时为 `Error type A at Line <N>: <description>.`（`src/lexer.rs`）。
**错误处理：** 如果文件无法读取，进程以退出码 1 退出并输出 `读取文件失败: ...`（`src/main.rs`）。如果解析完全失败，则不产生输出且命令静默返回。

### 2.2 `fmt`
**用途：** 美化打印 SysY 源码。
**输入：** 单个文件路径参数（`fmt <file>`）。
**输出：** 格式化后的 SysY 代码写入标准输出，包含表达式缩进、控制流布局和数组维度对齐（`src/format.rs`）。
**错误处理：** 如果文件无法读取，进程以退出码 1 退出（`src/main.rs`）。如果源码包含阻止解析的语法错误，格式化器不输出任何内容且无误返回（`src/format.rs`）。

### 2.3 `check`
**用途：** 执行语义分析（类型检查和作用域分析）。
**输入：** 单个文件路径参数（`check <file>`）。
**输出：** 诊断信息写入标准错误，每行一条，格式为 `Error type <K> at Line <N>: <message>`。检查器实现了 11 种错误类型（Error type 1–11 以及 Error type A）（`src/check.rs`）。
**错误处理：** 如果文件无法读取，进程以退出码 1 退出（`src/main.rs`）。如果发现语义错误，进程在输出所有诊断信息后以退出码 1 退出（`src/main.rs`）。如果没有发现错误，则以退出码 0 退出。代表性测试用例位于 `tests/semantic/normaltest01.sy` 至 `tests/semantic/normaltest11.sy`。

### 2.4 `gen-ir`
**用途：** 将 SysY 编译为 LLVM 14 中间表示（Intermediate Representation）。
**输入：** 输入文件路径和通过 `-o` 指定的输出文件路径（`gen-ir <input> -o <output>`）。
**输出：** LLVM IR 文本文件（`.ll`）写入指定路径（`src/gen_llvm_ir.rs`）。
**错误处理：** 如果文件无法读取，进程以退出码 1 退出（`src/main.rs`）。如果编译失败，进程向标准错误输出 `编译错误: ...` 并以退出码 1 退出（`src/main.rs`）。该流水线由 `tests/llvm_ir/` 中的 93 个文件进行验证。

### 2.5 `gen-asm`
**用途：** 将 SysY 编译为 RISC-V 汇编，采用线性扫描寄存器分配（linear-scan register allocation）。
**输入：** 输入文件路径和通过 `-o` 指定的输出文件路径（`gen-asm <input> -o <output>`）。
**输出：** RISC-V 汇编文本文件（`.s`）写入指定路径（`src/riscv_codegen/mod.rs`）。分配器依次使用临时寄存器 `t0–t6`，然后使用保存寄存器 `s0–s11`，为返回值保留 `a0–a1`（`docs/register_allocation_guide.md`）。
**错误处理：** 如果文件无法读取，进程以退出码 1 退出（`src/main.rs`）。如果编译失败，进程向标准错误输出 `编译错误: ...` 并以退出码 1 退出（`src/main.rs`）。代表性测试包括 `tests/codegen/register_alloc.sy` 和 `tests/codegen/euclidean_algorithm.sy`。

## 3. 向后兼容的位置参数模式

为向后兼容，编译器支持一种不使用子命令的旧式调用风格。当第一个和第二个参数既不是标志（以 `-` 开头）也不是已知子命令名称（`tokenize`、`fmt`、`check`、`gen-ir`、`gen-asm`）时，编译器将它们视为输入和输出文件路径，行为与 `gen-asm` 子命令完全一致（`src/main.rs`）。

示例：
```bash
compiler input.sy output.s
```
等效于：
```bash
compiler gen-asm input.sy -o output.s
```

## 4. 测试清单

测试套件按类别组织在 `tests/` 下的目录中。

| 类别 | 目录 | 数量 | 说明 |
|------|------|------|------|
| 词法分析器（Lexer） | `tests/lexer/` | 38 个文件 | 词元识别、整数字面量、注释和无效字符错误报告。示例：`tests/lexer/simple.in`、`tests/lexer/arrays_and_radix.in`。 |
| 格式化器（Formatter） | `tests/formatter/` | 8 个文件 | 表达式、控制流和数组的美化打印。示例：`tests/formatter/input1.txt`、`tests/formatter/example1.txt`。 |
| 语义（Semantic） | `tests/semantic/` | 23 个文件 | 类型检查、作用域分析和 11 种诊断错误类型。示例：`tests/semantic/normaltest01.sy`、`tests/semantic/example01.sy`。 |
| LLVM IR | `tests/llvm_ir/` | 93 个文件 | 函数、数组、控制流和边界情况的 LLVM IR 生成。示例：`tests/llvm_ir/edge_case_01.sy`、`tests/llvm_ir/example07.sy`。 |
| 代码生成（Codegen） | `tests/codegen/` | 6 项 | RISC-V 汇编源码、RARS 模拟器（`tests/codegen/rars.jar`）和辅助脚本（`tests/codegen/tests.sh`）。源码包括 `tests/codegen/register_alloc.sy` 和 `tests/codegen/euclidean_algorithm.sy`。 |
| 端到端（End-to-end） | `tests/end_to_end_codegen.rs` | 5 个测试函数 | 栈帧预算、加载/存储预算、通过 RARS 的运行时正确性、终止性和控制流分支验证。 |

## 5. 非功能需求

- **Rust 版本：** 该包目标为 Rust 2024 版本（`Cargo.toml`）。
- **LLVM 依赖：** 需要 LLVM 14 开发库，因为 `inkwell` 依赖配置了 `llvm14-0` 特性（`Cargo.toml`）。构建环境必须提供 `llvm-14-dev` 和 `libpolly-14-dev`（`README.md`）。
- **构建命令：** `cargo build --release` 在 `target/release/compiler` 生成二进制文件（`README.md`）。
- **Java 运行时：** 执行端到端代码生成测试时需要 Java 运行时来运行 RARS 模拟器（`README.md`、`tests/end_to_end_codegen.rs`）。
- **日志记录：** `gen-asm` 子命令初始化 `tklog` 文件日志，输出到 `tklogsize.txt`（`src/main.rs`）。

## 6. 非目标

以下 SysY 特性和编译器能力明确不在范围内：

- **浮点类型：** 尽管词法分析器会对 `float` 和 `double` 进行分词，但语法分析器和语义检查器不支持它们（`src/pests/lexer.pest`、`src/pests/check.pest`）。
- **额外控制流结构：** `for`、`do-while`、`switch`/`case` 和 `goto` 不在文法中（`src/pests/check.pest`）。
- **指针类型和指针算术：** 除了数组参数的隐式退化外，不支持指针（`src/pests/check.pest`）。
- **复合类型：** 不支持 `struct`、`union` 和 `typedef`（`src/pests/check.pest`）。
- **字符和字符串字面量：** 不支持 `char` 类型和字符串字面量（`src/pests/lexer.pest`）。
- **标准 I/O 库：** 不提供也不链接运行时函数，如 `getint`、`putint`、`getch`、`putch`、`getarray`、`putarray`、`starttime` 和 `stoptime`（`src/pests/check.pest`）。
- **高级优化：** 仅实现了线性扫描寄存器分配。不进行循环展开（loop unrolling）、常量传播（constant propagation）、死代码消除（dead-code elimination）或指令调度（instruction scheduling）（`docs/register_allocation_guide.md`）。
