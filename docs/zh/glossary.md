---
title: 术语表
description: SysY→RISC-V 编译器项目的领域专属术语
---

# 术语表

本文档定义了项目文档中使用的领域专属术语。术语按字母顺序排列。

---

## ABI / 调用约定 (ABI / Calling Convention)

规定函数之间如何传递参数、返回值以及在调用过程中保留寄存器的一组规则。

在本项目中，最小主线追踪（minimal steel thread）不涉及参数传递或调用者/被调用者保存寄存器的处理，因为 `main()` 没有参数且不进行函数调用。程序终止使用 `li a7, 93; ecall` 序列。

**用于：** `steel_thread_trace.md`、`functional_design.md`

---

## AllocatedInnerVar

寄存器分配器中的一个分发器类型，根据 `only_stack` 标志在纯栈分配和线性扫描寄存器分配之间进行选择。

在本项目中，它在 RISC-V 后端寄存器分配模块中定义，负责将活跃区间路由到 `NoAlloc` 或 `LinearScan`。

**用于：** `functional_design.md`

---

## AsmBuilder

一个字符串缓冲区包装器，用于生成格式化的 RISC-V 汇编文本，包括指示符、ALU 指令、内存操作、分支指令以及函数序言/尾声。

在本项目中，它为后端使用的每个伪指令和指示符提供了 `emit_*` 方法，其累积的缓冲区最终被写入 `.s` 文件。

**用于：** `functional_design.md`、`implementation.md`、`steel_thread_trace.md`

---

## 抽象语法树 (Abstract Syntax Tree, AST)

源代码语法结构的树形表示。

在本项目中，格式化器**不**构建显式的 AST 数据结构；它直接操作 `pest` 解析树节点对（parse-tree pairs）。语义检查器和 IR 生成器同样遍历解析树，而非专用的 AST。

**用于：** `functional_design.md`、`steel_thread_trace.md`

---

## 基本块 (Basic Block)

一段具有单一入口和单一出口的最大连续指令序列（除末尾外不含分支）。

在本项目中，基本块在 LLVM IR 生成过程中创建（例如 `if_true`、`if_false`、`whileCond`），RISC-V 后端使用它们划分指令以进行活跃性分析。

**用于：** `functional_design.md`、`implementation.md`

---

## clap

一个基于 Rust 派生宏的命令行参数解析器。

在本项目中，`clap` 驱动 `sysy-compiler` 的命令行子命令（`tokenize`、`fmt`、`check`、`gen-ir`、`gen-asm`）以及向后兼容的位置参数模式。

**用于：** `functional_design.md`、`implementation.md`、`requirements.md`

---

## 控制流 (Control Flow)

由条件、循环和跳转决定的各条语句和指令的执行顺序。

在本项目中，支持的 SysY 子集包括 `if`/`else`、`while`、`break`、`continue` 和 `return`。后端生成分支指令和标签，例如 `whileCond` 和 `whileBody`。

**用于：** `requirements.md`、`functional_design.md`

---

## 错误恢复 (Error Recovery)

解析器在遇到语法错误后恢复解析的能力，从而可以在一次遍历中报告多个错误。

在本项目中，发生语法错误时，解析器会跳至下一个 `;` 或 `}` 并继续解析，使检查器能够一次性输出多条诊断信息。

**用于：** `requirements.md`

---

## ErrorKind

一个包含 11 个编号语义错误类别（例如 `UndefinedVal`、`TypeMismatch`、`ReturnMismatch`）以及一个通配变体 `Other` 的枚举。

在本项目中，语义检查器生成的 `ErrorKind` 变体格式为 `Error type <K> at Line <N>: <message>`。

**用于：** `functional_design.md`、`implementation.md`

---

## 证据类 (Evidence Class)

附加在设计声明上的分类标签，用于说明该声明的验证方式。

在本项目中，共有三类：
- `[EXEC-VERIFIED]` —— 通过源码审查、单元测试或集成测试确认；
- `[STATIC-INFERRED]` —— 在未进行运行时验证的情况下，从源码结构推断得出；
- `[PARTIAL]` —— 部分实现或部分测试。

**用于：** `functional_design.md`、`steel_thread_trace.md`

---

## 格式化器 (Formatter)

读取原始 SysY 源码并输出行重整代码的美化打印器，包含 4 空格缩进、控制流布局和对齐的数组维度。

在本项目中，它通过 `fmt` 子命令暴露，直接操作 `pest` 解析树节点对，而非显式的 AST。

**用于：** `functional_design.md`、`implementation.md`、`requirements.md`

---

## FunctionState

RISC-V 后端第一遍分析阶段使用的按函数累积器，用于收集指令、基本块边界、已分配的值以及循环信息。

在本项目中，它在第二遍代码生成开始前，将 def/use 数据提供给活跃性分析和寄存器分配。

**用于：** `implementation.md`

---

## GenContext

跟踪变量位置（寄存器、栈或全局）、栈大小以及是否使用纯栈模式的代码生成上下文。

在本项目中，RISC-V 后端在第二遍代码生成阶段为每个函数创建一个 `GenContext`。

**用于：** `implementation.md`

---

## InnerVar

一种按指令索引（`start_offset` 和 `end_offset`）表示变量活跃区间的结构体。

在本项目中，`InnerVar` 区间由 `LinearScan` 消费，以决定变量获得物理寄存器还是被溢出到栈上。

**用于：** `functional_design.md`、`implementation.md`

---

## Inkwell

基于 LLVM-C API 的安全 Rust 绑定，用于以编程方式构建 SSA 形式的 LLVM IR。

在本项目中，`gen-ir` 子命令使用 `inkwell`（配置为 `llvm14-0` 特性）创建模块、基本块和指令。

**用于：** `functional_design.md`、`implementation.md`、`requirements.md`

---

## 指令选择 (Instruction Selection)

将 LLVM 操作码（例如 `Add`、`ICmp`、`Load`）映射到具体目标机器指令（例如 RISC-V 的 `add`、`slt`、`lw`）的过程。

在本项目中，它由 RISC-V 后端通过对 `InstructionOpcode` 进行模式匹配完成。

**用于：** `functional_design.md`、`steel_thread_trace.md`

---

## 接口契约 (Interface Contract)

两个编译器阶段之间的文档化约定，规定了预期的输入、输出和不变量。

在本项目中，主线追踪验证五个契约（例如，Parser → AST、Checker → LLVM IR、LLVM IR → RISC-V Gen），并为每个契约标注证据类。

**用于：** `steel_thread_trace.md`

---

## 中间表示 (Intermediate Representation, IR)

一种用于前端和后端之间的机器无关的程序表示。

在本项目中，**LLVM IR** 是主要的中间表示。`gen-ir` 子命令输出 `.ll` 文件，RISC-V 后端首先生成 LLVM IR，然后解析它以生成汇编。

**用于：** `requirements.md`、`functional_design.md`、`implementation.md`、`steel_thread_trace.md`

---

## IrCore

围绕 Inkwell `Context` 的包装器，可以派生新的 `IrSession`。

在本项目中，它封装了长生命周期的 LLVM 上下文对象；`Scanner` 持有一个 `IrCore` 实例。

**用于：** `implementation.md`

---

## IrSession

一个编译单元包，包含 Inkwell 的 `Context`、`Module`、`Builder` 和 `ScopeStack`。

在本项目中，`IrCore::start_session` 创建一个 `IrSession`，`Scanner` 使用它为单个源文件生成 LLVM 指令。

**用于：** `functional_design.md`、`implementation.md`、`steel_thread_trace.md`

---

## 已知技术债 (Known Debt)

代码库中明确记录的缺陷、待办事项或架构折中。

在本项目中，已知技术债包括过大的模块、因缺少共享 AST 导致的检查器与 IR 生成器之间的紧耦合、缺失的 `lib.rs` 以及硬编码类型。

**用于：** `implementation.md`

---

## 词法分析器 (Lexer)

将原始源码文本转换为类型化词法单元流的组件。

在本项目中，词法分析器实现为**基于 pest 的 PEG 分词器**，而非手写的 DFA。它通过 `tokenize` 子命令暴露，并由 38 个词法分析器测试用例验证。

**用于：** `requirements.md`、`functional_design.md`、`implementation.md`、`steel_thread_trace.md`

---

## 线性扫描 (Linear Scan)

一种贪心寄存器分配算法，按活跃区间起始位置对变量排序，按优先级顺序分配空闲寄存器，并在寄存器池耗尽时溢出变量。

在本项目中，它是主要的分配器，使用临时寄存器 `t3–t6`、保存寄存器 `s0–s11` 和参数寄存器 `a2–a7`。它实现了**最远溢出**启发式策略。

**用于：** `requirements.md`、`functional_design.md`、`implementation.md`

---

## 活跃性 (Liveness)

一种分析，用于确定在每个程序点上哪些变量是“活跃的”（在被重新定义之前可能被使用）。

在本项目中，按函数计算活跃性，以生成供 `LinearScan` 消费的活跃区间。由循环携带的 `alloca` 变量的区间会被延长至函数末尾。

**用于：** `functional_design.md`、`implementation.md`

---

## LLVM

低级虚拟机编译器基础设施。

在本项目中，编译器目标为 LLVM 14，并使用 LLVM IR 作为 SysY 与 RISC-V 之间的中间表示。构建环境需要 `llvm-14-dev` 和 `libpolly-14-dev`。

**用于：** `requirements.md`、`functional_design.md`、`implementation.md`、`steel_thread_trace.md`

---

## Location

一个枚举，描述值在生成的 RISC-V 程序中的位置：物理寄存器、栈槽或全局标签。

在本项目中，它由寄存器分配器生成，在指令发射期间被消费，以生成正确的加载/存储或移动指令。

**用于：** `implementation.md`

---

## 语法分析器 (Parser)

识别语法结构的组件。

在本项目中，所有解析阶段都复用 **pest** 的 PEG 语法，而非手写的递归下降解析器。针对词法分析、格式化、语义检查和 IR 生成分别存在独立的语法文件。

**用于：** `requirements.md`、`functional_design.md`、`implementation.md`、`steel_thread_trace.md`

---

## pest

一个基于解析表达式文法（Parsing Expression Grammar, PEG）的 Rust 解析器生成器。

在本项目中，`pest`（配合 `pest_derive`）在编译时从 `.pest` 语法文件生成 `Parser` 实现，避免了为每个编译器阶段手写词法分析器和语法分析器。

**用于：** `requirements.md`、`functional_design.md`、`implementation.md`

---

## RARS

RISC-V 汇编与运行时模拟器，一个基于 Java 的模拟器，用于验证生成的 RISC-V 汇编。

在本项目中，集成测试套件运行 RARS，检查寄存器值、5 秒超时内的终止情况以及控制流分支目标。

**用于：** `requirements.md`、`functional_design.md`、`implementation.md`、`steel_thread_trace.md`

---

## 寄存器分配 (Register Allocation)

将无限数量的虚拟变量（或 SSA 值）映射到有限数量的物理 CPU 寄存器的过程，必要时将多余变量溢出到栈上。

在本项目中，RISC-V 后端支持带栈溢出的 `LinearScan` 以及备用的 `NoAlloc` 纯栈模式。

**用于：** `requirements.md`、`functional_design.md`、`implementation.md`

---

## RISC-V

一种开放标准的指令集架构（ISA），也是本编译器的目标汇编语言。

在本项目中，`gen-asm` 子命令输出包含 RISC-V 指令和伪指令的 `.s` 文件。分配器使用临时寄存器 `t0–t6` 和保存寄存器 `s0–s11`，保留 `a0–a1` 用于返回值。

**用于：** `requirements.md`、`functional_design.md`、`implementation.md`、`steel_thread_trace.md`

---

## ScopeKey

一个用于标识作用域帧的枚举：全局作用域、函数作用域或内部块作用域。

在本项目中，它被用作 `ScopeStack` 符号表哈希映射的键。在 IR 生成过程中，函数和块变体携带 Inkwell 句柄（`FunctionValue`、`BasicBlock`）。

**用于：** `functional_design.md`、`implementation.md`

---

## ScopeStack

一种嵌套词法作用域抽象，维护一个活跃作用域键的栈以及从键到符号表的映射。

在本项目中，语义检查器和 IR 生成器使用相同的 `ScopeStack` 设计，以跨嵌套块和函数边界解析标识符。

**用于：** `functional_design.md`、`implementation.md`

---

## 语义分析 (Semantic Analysis)

在解析之后验证类型正确性、作用域规则和语言约束的编译阶段。

在本项目中，语义检查器实现了 11 种错误类别（Error type 1–11 以及 A），并通过 `check` 子命令暴露。

**同义词：** 语义检查器

**用于：** `requirements.md`、`functional_design.md`、`implementation.md`、`steel_thread_trace.md`

---

## 溢出 (Spill)

当寄存器池耗尽时，将变量从寄存器存储到栈槽的行为。

在本项目中，`LinearScan::overflow_to_stack` 使用**最远溢出**启发式策略：如果结束偏移最晚的活跃变量的结束位置在当前变量之后，则改为溢出该活跃变量，并将其寄存器重新分配。

**用于：** `functional_design.md`、`implementation.md`

---

## 静态单赋值 (Static Single Assignment, SSA)

一种 IR 属性，其中每个变量恰好被赋值一次。

在本项目中，LLVM IR 采用 SSA 形式，RISC-V 后端在将 LLVM 指令结果名映射到寄存器或栈槽时，将其称为“SSA 名称”。

**用于：** `implementation.md`

---

## 主线追踪 (Steel-Thread Trace)

一个最小程序在每个编译器阶段中的端到端执行追踪，用于验证接口契约并证明流水线正确连接。

在本项目中，该追踪涵盖一个返回 `1 + 2` 的 `main()` 函数所经过的 `tokenize`、`fmt`、`check`、`gen-ir` 和 `gen-asm` 阶段。

**用于：** `steel_thread_trace.md`

---

## 符号表 (Symbol Table)

一种将标识符映射到其语义类型和当前作用域中位置的数据结构。

在本项目中，符号表按作用域拆分在 `ScopeStack` 内部；不存在单一的全局 `func_table` 或 `var_table`。函数、变量和数组全部作为当前作用域中的 `Type` 变体存储。

**用于：** `functional_design.md`、`implementation.md`

---

## SysY

一种用于编译器设计课程的类 C 教学语言。

在本项目中，支持的子集包括 `int` 和 `void` 类型、标量和数组变量、函数以及基本控制流结构（`if`、`while`、`break`、`continue`、`return`）。浮点类型、指针（数组参数退化除外）、`struct`、`union` 和标准 I/O 均不在范围内。

**用于：** `requirements.md`、`functional_design.md`、`implementation.md`、`steel_thread_trace.md`

---

## 词法单元 (Token)

由词法分析器产生的源码文本的最小有意义单元，例如标识符、运算符、关键字和整数字面量。

在本项目中，`Token` 枚举定义了六个变体：`Identifier`、`Operator`、`Type`、`Flow`、`IntegerConst` 和 `ErrorSyntax`。

**用于：** `requirements.md`、`functional_design.md`、`implementation.md`、`steel_thread_trace.md`

---

## 可追溯性矩阵 (Traceability Matrix)

一种将每个编译阶段映射到其输入、转换、实现函数或类型以及输出的表格。

在本项目中，主线追踪包含一个 14 步的可追溯性矩阵，涵盖词法分析器、语法分析器、语义检查器、LLVM IR 生成和 RISC-V 代码生成。

**用于：** `steel_thread_trace.md`

---

## 两遍后端 (Two-Pass Backend)

一种代码生成策略，第一遍分析指令以计算活跃性并分配寄存器，第二遍生成汇编文本。

在本项目中，RISC-V 后端按函数使用此方法：`FunctionState` 在第一遍收集 def/use 信息，`AsmBuilder` 在第二遍生成指令。

**用于：** `functional_design.md`、`implementation.md`
