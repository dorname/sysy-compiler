---
title: "主线追踪"
description: "一个最小 SysY 程序的完整 gen-asm 编译路径的端到端追踪"
commit_pin: "f5b8eb08d74b85cdf4c78c4ecfdae4880dcfeb1c"
---

# 主线追踪

## 使用的最小 SysY 程序

```c
int main() {
    return 1 + 2;
}
```

上述程序在首次尝试时即成功编译，无需简化。

## 命令输出

### 词法单元流（`tokenize`）

```
INT int at Line 1.
IDENT main at Line 1.
L_PAREN ( at Line 1.
R_PAREN ) at Line 1.
L_BRACE { at Line 1.
RETURN return at Line 2.
INTEGER_CONST 1 at Line 2.
PLUS + at Line 2.
INTEGER_CONST 2 at Line 2.
SEMICOLON ; at Line 2.
R_BRACE } at Line 3.
```

### 语义检查（`check`）

（无输出；程序通过语义分析且没有错误。）

### 格式化源码（`fmt`）

```c
int main() {
    return 1 + 2;
}
```

### 生成的 LLVM IR（`gen-ir`）

```llvm
; ModuleID = 'module'
source_filename = "module"

define i32 @main() {
mainEntry:
  ret i32 3
}
```

### 生成的 RISC-V 汇编（`gen-asm`）

```asm
  .text
  .globl  main
main:
  addi sp, sp, -0
mainEntry:
  li a0, 3
  addi sp, sp, 0
  li a7, 93
  ecall
```

## 可追溯性矩阵

| # | 阶段 | 输入 | 转换 | 函数 / 类型 | 输出 |
|---|------|------|------|-------------|------|
| 1 | **词法分析器** | 源码字符串（`&str`） | Pest 语法分词 | `lexer::tokenize` → `lexer::tokenizer` in `src/lexer.rs` | 词法单元流输出到 stderr |
| 2 | **词法分析器文法** | 原始源码字符 | PEG 匹配 | `ExpressionParser`（派生自 `pest_derive::Parser`）使用 `src/pests/lexer.pest` | 基于词法单元的 `Pairs<'_, Rule>` 迭代器 |
| 3 | **解析器（格式化器）** | 源码字符串（`&str`） | Pest 语法解析为解析树 | `format::fmt` → `Formatter::format_code` in `src/format.rs` | 格式化后的 SysY 源码 |
| 4 | **解析器文法（格式化器）** | 原始源码字符 | PEG 匹配 | `FParser`（派生自 `pest_derive::Parser`）使用 `src/pests/parser.pest` | 表示完整解析树的 `Pairs<'_, Rule>` |
| 5 | **语义检查器** | 源码字符串（`&str`） | 解析 + 类型 / 作用域验证 | `Checker::syn_check` in `src/check.rs` | 错误信息（有效输入时为空） |
| 6 | **语义分析** | 来自 `CParser` 的 `Pairs<'_, Rule>` | 符号表构建、作用域栈管理、类型检查 | `Checker::analyze_declaration` → `Checker::analyze_func_def` → `Checker::analyze_var_decl` in `src/check.rs` | 已检查的符号表及收集到的语义错误 |
| 7 | **LLVM IR 生成** | 源码字符串（`&str`） | 解析 → 通过 `inkwell` 构建 LLVM IR | `Scanner::scan_collect` in `src/gen_llvm_ir.rs` | 写入 `.ll` 文件的 LLVM IR |
| 8 | **LLVM IR 扫描** | 来自 `CParser` 的 `Pairs<'_, Rule>`（使用 `src/pests/scan.pest`） | 符号收集 + IR 发射 | `Scanner::scan_declaration` → `Scanner::scan_func_def` → `Scanner::collect_func_params` in `src/gen_llvm_ir.rs` | 已填充的 `IrSession`（`Module` + `Builder`） |
| 9 | **LLVM IR 会话** | 来自 `IrCore` 的 `Context` | 模块与构建器创建 | `IrCore::start_session` 返回 `IrSession<'_>` in `src/gen_llvm_ir.rs` | 准备好发射指令的 LLVM `Module` 和 `Builder` |
| 10 | **RISC-V 代码生成驱动** | 源码字符串（`&str`） | 两阶段流水线：生成 LLVM IR → 解析 IR → 发射汇编 | `generate_asm` in `src/riscv_codegen/mod.rs` | 写入 `.s` 文件的 RISC-V 汇编 |
| 11 | **RISC-V IR 解析** | `IrSession`（LLVM `Module`） | 提取全局变量、函数、基本块、指令 | `parse_llvm_ir` → `build_function` → `build_global_variable` in `src/riscv_codegen/mod.rs` | 已填充的 `AsmBuilder` 缓冲区 |
| 12 | **指令选择** | LLVM `InstructionValue`s | 操作码分派到 RISC-V 指令模式 | `generate_instruction` in `src/riscv_codegen/mod.rs` | 追加到 `AsmBuilder` 的 RISC-V 指令 |
| 13 | **寄存器分配** | `InnerVar` 生存期区间 | 线性扫描分配（寄存器 + 栈溢出） | `LinearScan::allocate`（实现 `RegisterAllocator`）in `src/riscv_codegen/register_alloc.rs` | 将 SSA 名称映射到寄存器或栈槽的 `HashMap<String, String>` |
| 14 | **汇编发射** | `AsmBuilder` 内部缓冲区 | 所有已发射的伪指令 / 指令的字符串拼接 | `AsmBuilder::emit` in `src/riscv_codegen/asm_builder.rs` | 最终的 `.s` 文件字符串 |

## 契约覆盖矩阵

| # | 接口契约 | 覆盖情况 | 备注 |
|---|----------|----------|------|
| 1 | **解析器 → AST** | `[EXEC-VERIFIED]` | 全部五个子命令（`tokenize`、`fmt`、`check`、`gen-ir`、`gen-asm`）均成功解析了源码。解析器文法（`parser.pest`、`check.pest`、`scan.pest`、`lexer.pest`）在每条路径上都生成了 `Pairs<'_, Rule>` 解析树。 |
| 2 | **AST → 语义检查器** | `[EXEC-VERIFIED]` | `Checker::syn_check` 消费解析树，构建作用域栈并验证类型。`check` 子命令对该最小程序输出零错误，确认该契约已被执行。 |
| 3 | **语义检查器 → LLVM IR** | `[STATIC-INFERRED]` | `gen-ir` 与 `gen-asm` 子命令直接从原始源码字符串实例化 `Scanner`，并**不**消费 `Checker` 的输出。两个阶段均独立重新解析。因此，语义检查器与 LLVM IR 之间的数据流契约在运行时未被实际执行；它仅由“输入已经过验证”这一假设隐含成立。 |
| 4 | **LLVM IR → RISC-V 生成** | `[EXEC-VERIFIED]` | `generate_asm` 内部调用 `Scanner::scan_collect_asm` 生成 LLVM IR，然后将得到的 `IrSession` 传递给 `parse_llvm_ir`，后者遍历 LLVM `Module` 并发射 RISC-V 指令。中间 IR 与最终汇编均已被捕获并验证。 |
| 5 | **ABI / 调用约定** | `[PARTIAL]` | 该最小程序定义了不带参数且无函数调用的 `main()`。因此，该主线并未执行参数传递、调用者/被调用者保存寄存器的处理，或 `jal` / `ret` 调用序列。虽然观察到了 `li a7, 93; ecall` 退出序列，但这属于程序终止系统调用，而非函数调用 ABI。 |
