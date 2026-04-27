# SysY 编译器（Rust 实现）

## 项目背景

本项目是对 **Rust 编译器训练营** 6 个阶段性实验（lab1–lab6）的整合与改造：

| 实验阶段 | 功能 | 对应模块 |
|----------|------|----------|
| **lab1** | 词法分析 | `src/lexer.rs` |
| **lab2** | 语法分析与代码格式化 | `src/format.rs` |
| **lab3** | 语义检查 | `src/check.rs` |
| **lab5** | LLVM IR 生成 | `src/gen_llvm_ir.rs` |
| **lab6** | RISC-V 汇编生成 | `src/riscv_codegen/` |

在原有实验代码的基础上，进行了模块整合、CLI 统一入口、线性扫描寄存器分配算法修复、控制流 lowering 修正以及测试体系重构，最终形成一套可完整运行的 **SysY → RISC-V** 编译器。

## 功能特性

| 子命令 | 说明 |
|--------|------|
| `tokenize` | 词法分析，将 SysY 源码转为 Token 流 |
| `fmt` | 代码格式化，输出格式化后的 SysY 代码 |
| `check` | 语义检查，执行类型检查和作用域分析 |
| `gen-ir` | 生成 LLVM IR，将 SysY 编译为 LLVM 中间表示 |
| `gen-asm` | 生成 RISC-V 汇编，包含线性扫描寄存器分配优化 |

> **兼容模式**：支持直接传入输入文件和输出文件（等同于 `gen-asm`），例如 `cargo run -- input.sy output.s`。

## 依赖

- **Rust 工具链**（建议最新 stable 版本）
- **LLVM 14** 及相关开发库
  ```bash
  sudo apt-get install llvm-14-dev libpolly-14-dev
  sudo apt-get update
  sudo apt-get install zlib1g-dev
  ```
- **Java**（用于 RARS 模拟器运行端到端代码生成验证）

## 构建

```bash
cargo build --release
```

编译完成后，可执行文件位于 `target/release/compiler`。

## 使用示例

```bash
# 词法分析
cargo run -- tokenize tests/lexer/simple.in

# 代码格式化
cargo run -- fmt tests/formatter/input1.txt

# 语义检查
cargo run -- check tests/semantic/normaltest01.sy

# 生成 LLVM IR
cargo run -- gen-ir tests/llvm_ir/example01.sy -o out.ll

# 生成 RISC-V 汇编（子命令方式）
cargo run -- gen-asm tests/codegen/register_alloc.sy -o out.s

# 兼容模式：直接传参（等同于 gen-asm）
cargo run -- tests/codegen/register_alloc.sy out.s
```

## 测试

```bash
# 运行全部测试（单元测试 + 端到端集成测试）
cargo test

# 单独运行端到端代码生成验证（栈帧、load/store、控制流、运行结果）
cargo test --test end_to_end_codegen -- --nocapture
```

## 项目结构

```
├── src/
│   ├── lexer.rs              # 词法分析
│   ├── format.rs             # 代码格式化 / 语法分析
│   ├── check.rs              # 语义检查
│   ├── gen_llvm_ir.rs        # LLVM IR 生成
│   ├── riscv_codegen/        # RISC-V 汇编生成
│   │   ├── mod.rs
│   │   ├── register_alloc.rs # 线性扫描寄存器分配
│   │   └── asm_builder.rs
│   ├── main.rs               # CLI 入口与命令分发
│   └── utils.rs              # 工具函数
├── tests/
│   ├── lexer/                # 词法分析测试用例
│   ├── formatter/            # 格式化测试用例
│   ├── semantic/             # 语义检查测试用例
│   ├── llvm_ir/              # LLVM IR 生成测试用例
│   ├── codegen/              # RISC-V 汇编测试用例与 RARS
│   └── end_to_end_codegen.rs # 端到端代码生成验证
├── docs/
│   └── register_allocation_guide.md  # 线性扫描寄存器分配算法实现指南
├── Cargo.toml
└── README.md
```

## 技术实现

### 词法分析
基于 [`pest`](https://pest.rs/) Parser Generator，通过 `pests/lexer.pest` 定义 SysY 词法规则。支持标识符、关键字、运算符、十进制/八进制/十六进制整数常量，以及非法字符的错误识别与定位。

### 语法分析与格式化
基于 `pest` 的 PEG 语法（`pests/parser.pest`、`pests/check.pest`），实现 SysY 语法的解析与结构化格式化输出，包括表达式缩进、控制流排版和数组维度对齐。

### 语义检查
实现完整的静态语义分析，涵盖：
- 变量与函数声明的重复定义检测
- 表达式与赋值的类型兼容性检查
- 作用域管理与可见性规则
- 函数调用参数数量与类型匹配
- 数组访问验证
- 共 **11 类语义错误诊断**（Error type 1–11 + Error type A）

### LLVM IR 生成
基于 [`inkwell`](https://github.com/TheDan64/inkwell)（Rust 对 LLVM 的绑定），将 SysY AST 编译为 LLVM 14 中间表示。支持函数定义、局部/全局变量、控制流（if/while）、数组操作以及算术/逻辑/比较运算。

### RISC-V 汇编生成
采用**两次遍历架构**：
1. **第一次遍历**：基于 LLVM IR 收集变量的活跃区间（Live Interval），执行**线性扫描寄存器分配**，为变量分配 RISC-V 寄存器或栈位置。
2. **第二次遍历**：根据寄存器/栈分配结果，逐指令生成 RISC-V 汇编代码。

**寄存器分配策略**：
- 优先使用临时寄存器 `t0–t6`，其次使用保存寄存器 `s0–s11`
- 保留 `a0–a1` 用于函数返回值
- 全局变量不参与寄存器分配，直接通过 `la` + `lw/sw` 访问

## 许可证

本项目为学习用途的编译器实验代码。
