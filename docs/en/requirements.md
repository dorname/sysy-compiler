---
title: SysY Compiler Requirements
date: 2026-04-27
version: 0.1.0
---

# SysY Compiler Requirements

This document specifies the functional and non-functional requirements for the educational SysY→RISC-V compiler implemented in Rust. The baseline source revision is recorded in `docs/.pin`.

## 1. Supported SysY Language Subset

The compiler accepts a subset of the SysY language defined by the grammars in `src/pests/lexer.pest`, `src/pests/parser.pest`, and `src/pests/check.pest`.

### 1.1 Types
Only the types `int` and `void` are supported. The lexer tokenizes `float`, but the parser and semantic checker do not admit floating-point declarations (`src/pests/lexer.pest`, `src/pests/check.pest`).

### 1.2 Variables and Constants
Scalar variables and constants may be declared at file scope or block scope. A declaration has the form `int x;`, `const int x = 1;`, or `int x = 1;`. Array declarations are supported with constant dimensions, e.g., `int a[2][3];`. Array parameters may omit the first dimension, e.g., `int f(int a[][3])`. These constructs are exercised by `tests/semantic/normaltest01.sy`, `tests/codegen/register_alloc.sy`, and `tests/llvm_ir/edge_case_01.sy`.

### 1.3 Functions
Functions are defined with a return type (`int` or `void`), a name, an optional parameter list, and a block body. Function calls are expressions. Examples appear in `tests/semantic/example01.sy` and `tests/codegen/euclidean_algorithm.sy`.

### 1.4 Statements
The supported statements are:
- Expression statement (possibly empty `;`)
- Assignment statement (`LVal = Exp;`)
- Block statement (`{ ... }`)
- `if`/`else` conditional
- `while` loop
- `break` and `continue`
- `return` with optional expression

These are validated by `tests/codegen/euclidean_algorithm.sy`, `tests/llvm_ir/example07.sy`, and the semantic suite `tests/semantic/`.

### 1.5 Expressions
Expressions include arithmetic (`+`, `-`, `*`, `/`, `%`), relational (`<`, `<=`, `>`, `>=`), equality (`==`, `!=`), logical (`&&`, `||`, `!`), unary (`+`, `-`, `!`), parenthesized sub-expressions, array element access (`a[i][j]`), and function calls. This subset is covered by `tests/llvm_ir/edge_case_01.sy` and `tests/lexer/arrays_and_radix.in`.

### 1.6 Literals and Comments
Integer literals in decimal, octal (`0...`), and hexadecimal (`0x...`) are supported (`tests/lexer/comments_and_hex.in`). Line comments (`//`) and block comments (`/* */`) are stripped by the lexer (`tests/lexer/comments_and_hex.in`).

### 1.7 Error Recovery
The parser performs error synchronization: on a syntax error it skips to the next `;` or `}` and continues parsing so that multiple errors can be reported in one pass (`src/pests/check.pest`).

## 2. CLI Subcommands

The compiler is a binary crate invoked as `sysy-compiler`. All behavior is dispatched from `src/main.rs`.

### 2.1 `tokenize`
**Purpose:** Convert SysY source into a token stream.
**Input:** A single file path argument (`tokenize <file>`).
**Output:** Token lines written to standard error. Each line is either `<TOKEN> at Line <N>.` or, when lexical errors are present, `Error type A at Line <N>: <description>.` (`src/lexer.rs`).
**Error handling:** If the file cannot be read, the process exits with code 1 and prints `读取文件失败: ...` (`src/main.rs`). If parsing fails entirely, no output is produced and the command returns silently.

### 2.2 `fmt`
**Purpose:** Pretty-print SysY source code.
**Input:** A single file path argument (`fmt <file>`).
**Output:** Formatted SysY code written to standard output, with expression indentation, control-flow layout, and array dimension alignment (`src/format.rs`).
**Error handling:** If the file cannot be read, the process exits with code 1 (`src/main.rs`). If the source contains syntax errors that prevent parsing, the formatter outputs nothing and returns without error (`src/format.rs`).

### 2.3 `check`
**Purpose:** Perform semantic analysis (type checking and scope analysis).
**Input:** A single file path argument (`check <file>`).
**Output:** Diagnostic messages written to standard error, one per line, in the form `Error type <K> at Line <N>: <message>`. The checker implements 11 error kinds (Error type 1–11 plus Error type A) (`src/check.rs`).
**Error handling:** If the file cannot be read, the process exits with code 1 (`src/main.rs`). If semantic errors are found, the process exits with code 1 after printing all diagnostics (`src/main.rs`). If no errors are found, it exits with code 0. Representative test cases are in `tests/semantic/normaltest01.sy` through `tests/semantic/normaltest11.sy`.

### 2.4 `gen-ir`
**Purpose:** Compile SysY to LLVM 14 intermediate representation.
**Input:** An input file path and an output file path specified with `-o` (`gen-ir <input> -o <output>`).
**Output:** An LLVM IR text file (`.ll`) written to the specified path (`src/gen_llvm_ir.rs`).
**Error handling:** If the file cannot be read, the process exits with code 1 (`src/main.rs`). If compilation fails, the process prints `编译错误: ...` to standard error and exits with code 1 (`src/main.rs`). This pipeline is exercised by the 93 files in `tests/llvm_ir/`.

### 2.5 `gen-asm`
**Purpose:** Compile SysY to RISC-V assembly with linear-scan register allocation.
**Input:** An input file path and an output file path specified with `-o` (`gen-asm <input> -o <output>`).
**Output:** A RISC-V assembly text file (`.s`) written to the specified path (`src/riscv_codegen/mod.rs`). The allocator uses temporaries `t0–t6`, then saved registers `s0–s11`, preserving `a0–a1` for return values (`docs/register_allocation_guide.md`).
**Error handling:** If the file cannot be read, the process exits with code 1 (`src/main.rs`). If compilation fails, the process prints `编译错误: ...` to standard error and exits with code 1 (`src/main.rs`). Representative tests include `tests/codegen/register_alloc.sy` and `tests/codegen/euclidean_algorithm.sy`.

## 3. Backward-Compatible Positional-Args Mode

For backward compatibility, the compiler supports a legacy invocation style that does not use a subcommand. When the first two arguments are neither flags (starting with `-`) nor known subcommand names (`tokenize`, `fmt`, `check`, `gen-ir`, `gen-asm`), the compiler treats them as input and output file paths and behaves exactly like the `gen-asm` subcommand (`src/main.rs`).

Example:
```bash
compiler input.sy output.s
```
This is equivalent to:
```bash
compiler gen-asm input.sy -o output.s
```

## 4. Test Inventory

The test suite is organized into category directories under `tests/`.

| Category | Directory | Count | Description |
|----------|-----------|-------|-------------|
| Lexer | `tests/lexer/` | 38 files | Token recognition, integer literals, comments, and invalid-character error reporting. Examples: `tests/lexer/simple.in`, `tests/lexer/arrays_and_radix.in`. |
| Formatter | `tests/formatter/` | 8 files | Pretty-printing of expressions, control flow, and arrays. Examples: `tests/formatter/input1.txt`, `tests/formatter/example1.txt`. |
| Semantic | `tests/semantic/` | 23 files | Type checking, scope analysis, and 11 diagnostic error kinds. Examples: `tests/semantic/normaltest01.sy`, `tests/semantic/example01.sy`. |
| LLVM IR | `tests/llvm_ir/` | 93 files | LLVM IR generation for functions, arrays, control flow, and edge cases. Examples: `tests/llvm_ir/edge_case_01.sy`, `tests/llvm_ir/example07.sy`. |
| Codegen | `tests/codegen/` | 6 items | RISC-V assembly sources, the RARS simulator (`tests/codegen/rars.jar`), and a helper script (`tests/codegen/tests.sh`). Sources include `tests/codegen/register_alloc.sy` and `tests/codegen/euclidean_algorithm.sy`. |
| End-to-end | `tests/end_to_end_codegen.rs` | 5 test functions | Stack-frame budget, load/store budget, runtime correctness via RARS, termination, and control-flow branch verification. |

## 5. Non-Functional Requirements

- **Rust edition:** The crate targets the Rust 2024 edition (`Cargo.toml`).
- **LLVM dependency:** LLVM 14 development libraries are required because the `inkwell` dependency is configured with the `llvm14-0` feature (`Cargo.toml`). The build environment must provide `llvm-14-dev` and `libpolly-14-dev` (`README.md`).
- **Build command:** `cargo build --release` produces the binary at `target/release/compiler` (`README.md`).
- **Java runtime:** A Java runtime is required to execute the RARS simulator during end-to-end codegen tests (`README.md`, `tests/end_to_end_codegen.rs`).
- **Logging:** The `gen-asm` subcommand initializes `tklog` file logging to `tklogsize.txt` (`src/main.rs`).

## 6. Non-Goals

The following SysY features and compiler capabilities are explicitly out of scope:

- **Floating-point types:** `float` and `double` are not supported in the parser or semantic checker, despite being tokenized by the lexer (`src/pests/lexer.pest`, `src/pests/check.pest`).
- **Additional control-flow constructs:** `for`, `do-while`, `switch`/`case`, and `goto` are not in the grammar (`src/pests/check.pest`).
- **Pointer types and pointer arithmetic:** Except for the implicit decay of array parameters, pointers are not supported (`src/pests/check.pest`).
- **Composite types:** `struct`, `union`, and `typedef` are not supported (`src/pests/check.pest`).
- **Character and string literals:** The `char` type and string literals are not supported (`src/pests/lexer.pest`).
- **Standard I/O library:** Runtime functions such as `getint`, `putint`, `getch`, `putch`, `getarray`, `putarray`, `starttime`, and `stoptime` are not provided or linked (`src/pests/check.pest`).
- **Advanced optimizations:** Only linear-scan register allocation is implemented. No loop unrolling, constant propagation, dead-code elimination, or instruction scheduling is performed (`docs/register_allocation_guide.md`).
