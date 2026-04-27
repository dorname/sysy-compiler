---
title: Functional Design Document
project: SysY→RISC-V Compiler
description: Comprehensive functional design for the Rust SysY compiler frontend, middle-end, and RISC-V backend.
commit_pin: f5b8eb08d74b85cdf4c78c4ecfdae4880dcfeb1c
---

# Functional Design Document

## 1. Overview

### 1.1 System Purpose

This project is a Rust-based compiler for the SysY language (a C-like teaching language) targeting RISC-V assembly. The compiler pipeline consists of:

1. **Lexical analysis** — pest-driven PEG tokenizer
2. **Parsing / Formatting** — pest-driven formatter with layout rules
3. **Semantic checking** — scope-aware type checker with 11 error categories
4. **LLVM IR generation** — two-phase scan using the Inkwell crate
5. **RISC-V backend** — two-pass code generation with linear-scan register allocation

### 1.2 Design Philosophy

- **PEG-first front-end**: All syntactic analysis stages reuse pest grammars (`src/pests/lexer.pest`, `parser.pest`, `check.pest`, `scan.pest`) rather than hand-written recursive-descent parsers. [EXEC-VERIFIED]
- **Scope-stack symbol tables**: Both the semantic checker and the IR generator use an identical `ScopeStack`/`ScopeKey` abstraction for nested lexical scoping. [EXEC-VERIFIED]
- **Assume-well-formed inputs for later stages**: `gen_llvm_ir.rs` and `riscv_codegen/` do not emit user-facing diagnostics; they assume the source has already passed semantic analysis. [STATIC-INFERRED]
- **Two-pass backend**: The RISC-V backend first collects def/use information and computes variable liveness, then emits instructions using the allocation results. [EXEC-VERIFIED]

---

## 2. Lexer Functional Design (`src/lexer.rs`)

### 2.1 Inputs & Outputs

| Aspect | Description |
|--------|-------------|
| **Input** | Raw SysY source string (`&str`). |
| **Output** | Token stream written to an `io::Write` sink, or `stderr` via `tokenize`. |
| **Failure mode** | Parse failures from pest are silently swallowed; only successfully parsed tokens (or error tokens recognized by the grammar) are emitted. [EXEC-VERIFIED] |

### 2.2 Token Types

The `Token` enum in `src/lexer.rs` defines six top-level variants [EXEC-VERIFIED]:

| Variant | Description | Display Example |
|---------|-------------|-----------------|
| `Token::Identifier(String)` | Non-keyword identifiers | `IDENT foo` |
| `Token::Operator(Operator)` | Operators and punctuation | `PLUS +` |
| `Token::Type(Type)` | Type keywords | `INT int` |
| `Token::Flow(Flow)` | Control-flow keywords | `IF if` |
| `Token::IntegerConst(IntegerConst)` | Decimal / hex / octal literals | `INTEGER_CONST 42` |
| `Token::ErrorSyntax(ErrorSyntax)` | Lexical error tokens | `INVALID_HEX 0xGH` |

`IntegerConst` has three sub-variants: `Hex(String)`, `Octal(String)`, `Dec(String)`. Hex and octal values are converted to decimal at display time using `hex_to_int` and `oct_to_int` from `src/utils.rs`. [EXEC-VERIFIED]

`Operator` covers `+ - * / % = == != < <= > >= && || ! ( ) { } [ ] , ;`. [EXEC-VERIFIED]

`Type` covers `int`, `float`, `void`, `const`. [EXEC-VERIFIED]

`Flow` covers `if`, `else`, `while`, `break`, `continue`, `return`. [EXEC-VERIFIED]

### 2.3 Lexical Error Categories

`ErrorSyntax` enumerates recoverable lexical errors recognized by `lexer.pest` [EXEC-VERIFIED]:

| Variant | Trigger |
|---------|---------|
| `VarError` | (reserved / currently unused in grammar) |
| `InvalidHex` | `0x` / `0X` prefix with non-hex digits following |
| `InvalidOctal` | `0` prefix with non-octal digits following |
| `InvalidInteger` | (reserved / currently unused) |
| `InvalidOperator` | Unrecognized single characters such as `#`, `&`, `\|`, `~` |
| `UnKnownError` | Catch-all for any other unexpected character |

### 2.4 Key Functions

- `parse_file(input: &str) -> Option<Pairs<'_, Rule>>` — parses the whole file with `ExpressionParser` (derived from `lexer.pest`). Returns `None` on any pest error. [EXEC-VERIFIED]
- `tokenizer<W: Write>(input: &str, w: W) -> io::Result<()>` — main entry point. If error tokens are present, only error lines are emitted (`Error type A at Line N: …`). Otherwise the full token stream is emitted. [EXEC-VERIFIED]
- `tokenize(input: &str)` — convenience wrapper writing to `stderr`. [EXEC-VERIFIED]
- `impl From<Pair<'_, Rule>> for Token` — recursive descent over pest pairs to produce `Token` values. [EXEC-VERIFIED]

### 2.5 Invariants & Failure Modes

- **Invariant**: Every character in the input is either consumed as a token or matched by one of the `ErrorSyntax` rules. [STATIC-INFERRED]
- **Failure mode**: If pest fails at the top level (e.g., unmatched block comment), `parse_file` returns `None` and `tokenizer` returns `Ok(())` without emitting anything. [EXEC-VERIFIED]

---

## 3. Parser / Formatter Functional Design (`src/format.rs`)

### 3.1 Inputs & Outputs

| Aspect | Description |
|--------|-------------|
| **Input** | Raw SysY source string (`&str`). |
| **Output** | Reformatted source code (or error lines) written to an `io::Write` sink. |
| **Failure mode** | If pest parsing fails, nothing is written. [EXEC-VERIFIED] |

### 3.2 AST Representation

The formatter does not build an explicit AST data structure; it operates directly on `pest::iterators::Pair<Rule>` values produced by `FParser` (derived from `src/pests/parser.pest`). [EXEC-VERIFIED]

### 3.3 Formatting Rules

`Formatter::fmt` in `src/format.rs` implements the following layout policy [EXEC-VERIFIED]:

| Construct | Rule |
|-----------|------|
| **Indentation** | 4-space blocks. `deep` tracks nesting depth. `OpenBrace` increments, `CloseBrace` decrements. |
| **Control flow** | Statements following `) ` (e.g., `while (…) stmt`) are indented +1 and placed on a new line. `else` followed by a non-`if` statement is similarly indented. |
| **Array alignment** | Array dimensions (`ArrayDims`) are traversed inline without extra spacing. |
| **Semicolons** | Trailing spaces before `;` are stripped. A newline is appended after `;` unless `forbid_semicolon_newline` is set. |
| **Blank lines** | `clean_extra_blank_lines` removes redundant blank lines, preserving exactly one blank line between function definitions. `should_keep_blank_line` retains lines where the previous non-empty line ends with `}` and the next starts with `int` / `void` / `const`. |

### 3.4 Key Functions

- `Formatter::new(deep: usize, input: &'a str, writer: &'a mut W) -> Self` — constructs a formatter with zero indentation. [EXEC-VERIFIED]
- `Formatter::fmt(&mut self, pair: Pair<Rule>)` — recursive dispatch over parser rules. [EXEC-VERIFIED]
- `Formatter::format_code(&mut self) -> io::Result<()>` — orchestrates parsing, calls `fmt` on each top-level pair, then emits either `err_output` or the cleaned formatted text. [EXEC-VERIFIED]
- `fmt(input: &str)` — convenience wrapper writing to `stdout`. [EXEC-VERIFIED]

### 3.5 Invariants & Failure Modes

- **Invariant**: Output lines never contain trailing spaces before semicolons. [EXEC-VERIFIED]
- **Failure mode**: `ErrorStmt` pairs produce `Error type A at Line N: …` lines in `err_output`; if any exist, formatting output is suppressed and only errors are emitted. [EXEC-VERIFIED]

---

## 4. Semantic Checker Functional Design (`src/check.rs`)

### 4.1 Inputs & Outputs

| Aspect | Description |
|--------|-------------|
| **Input** | Raw SysY source string (`&str`). |
| **Output** | Error messages written to the provided `io::Write` sink. Empty output means no semantic errors. |
| **Failure mode** | Syntax errors cause a single `Syntax error` line to be emitted. [EXEC-VERIFIED] |

### 4.2 Scope Analysis

#### ScopeStack, ScopeKey, ScopeFrame

The checker uses three cooperating types [EXEC-VERIFIED]:

- `ScopeKey` — enum with `Global`, `Ident(String)`, `InnerBlock`.
- `Scope` — holds `symbol_table: HashMap<String, Type>`.
- `ScopeStack` — holds a `Vec<ScopeKey>` (the stack) and `HashMap<ScopeKey, Scope>` (the frames).

Key methods on `ScopeStack` [EXEC-VERIFIED]:

| Method | Behavior |
|--------|----------|
| `push(key: ScopeKey)` | Creates a new empty `Scope` and pushes the key. |
| `pop()` | Removes the top frame. |
| `get_current_scope() -> Option<&Scope>` | Returns the top frame. |
| `get_current_scope_mut() -> Option<&mut Scope>` | Returns the top frame mutably (uses clone-to-avoid-borrow). |
| `contains(name: &str) -> bool` | Searches stack top-down for the name. |
| `get(name: &str) -> Option<Type>` | Looks up the nearest definition. |

The default stack initializes with `ScopeKey::Global`. [EXEC-VERIFIED]

### 4.3 Type Checking

Supported semantic types (`check::Type`) [EXEC-VERIFIED]:

| Variant | Meaning |
|---------|---------|
| `Type::Int` | 32-bit integer |
| `Type::Void` | Void (for function returns) |
| `Type::Func(Func)` | Function signature (params + return type) |
| `Type::Array(ArrayStruct)` | Multi-dimensional array |

Compatibility rules enforced by the checker [EXEC-VERIFIED]:

- Assignment LHS must not be a function (`UnexpectedFuncAssign`).
- Arrays can only be assigned to arrays of identical dimension count (`TypeMismatch`).
- `int` cannot be assigned to an array with non-zero dimensions (`TypeMismatch`).
- Arithmetic / relational / logical operators require both operands to be `Int` (`TypeMismatchOp`).
- Array subscripts are only permitted on `Array` types (`NotArrayAssign`).
- Function call argument count and types must match the callee signature (`Inappropriate`).
- Return expression type must match the enclosing function’s return type (`ReturnMismatch`).

### 4.4 Error Taxonomy

The checker implements 11 numbered error kinds (`ErrorKind`) [EXEC-VERIFIED]:

| ID | Enum Variant | Message |
|----|--------------|---------|
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

Errors are accumulated in `Checker::errors: Vec<SemanticError>` and flushed by `generate_error_output`. [EXEC-VERIFIED]

### 4.5 Symbol Table

The symbol table is split per-scope; there is no standalone `func_table` or `var_table` globally. Instead:

- Functions are inserted into the **current scope** (usually `Global`) as `Type::Func`. [EXEC-VERIFIED]
- Variables and arrays are inserted into the **current scope** as `Type::Int` or `Type::Array`. [EXEC-VERIFIED]
- Function parameters are inserted into the **function body scope** as `Type::Int` or `Type::Array`. [EXEC-VERIFIED]

### 4.6 Key Functions

- `Checker::new(input, writer) -> Self` — initializes the checker with an empty scope stack rooted at `Global`. [EXEC-VERIFIED]
- `Checker::syn_check(&mut self) -> io::Result<()>` — parses with `CParser` and drives `analyze_declaration` for each top-level declaration. [EXEC-VERIFIED]
- `Checker::analyze_declaration`, `analyze_var_decl`, `analyze_func_def`, `analyze_block_item`, `analyze_stmt` — recursive AST walkers. [EXEC-VERIFIED]
- `Checker::analyze_exp` / `analyze_add_exp` / `analyze_mul_exp` / `analyze_unary_exp` / `analyze_call_exp` — expression type inference returning `Option<Type>`. [EXEC-VERIFIED]
- `Checker::check_same_ident` — detects redefinition in the current scope only. [EXEC-VERIFIED]
- `Checker::check_undefine` — detects use-before-define by scanning all active scopes. [EXEC-VERIFIED]
- `Checker::collect_error` — appends a `SemanticError` to the internal error vector. [EXEC-VERIFIED]

### 4.7 Invariants & Failure Modes

- **Invariant**: `ScopeStack::get` always returns the *nearest* enclosing definition (shadowing is allowed across scopes, prohibited within the same scope). [EXEC-VERIFIED]
- **Failure mode**: If `parse_file` returns `None`, only `Syntax error` is emitted and no semantic analysis runs. [EXEC-VERIFIED]

---

## 5. LLVM IR Generator Functional Design (`src/gen_llvm_ir.rs`)

### 5.1 Inputs & Outputs

| Aspect | Description |
|--------|-------------|
| **Input** | Raw SysY source string (`&str`) and an output file path (`&str`). |
| **Output** | LLVM IR text file (`.ll`) and optionally a RISC-V assembly file (`.s`) when invoked via `scan_collect_asm`. |
| **Failure mode** | Returns `Err(String)` on parse failure, IR verification failure, or file write failure. [EXEC-VERIFIED] |

### 5.2 IR Construction Strategy

The generator uses a **single-phase scan** (not two-phase in the traditional frontend/backend sense) that interleaves symbol collection and IR emission [EXEC-VERIFIED]:

1. Parse the source with `CParser` (`scan.pest`).
2. Create an `IrSession` (Inkwell `Context`, `Module`, `Builder`, plus `ScopeStack`).
3. Walk declarations:
   - **Globals** (`const` / `var`): emit `GlobalValue` or `alloca` + `store`, then insert into the current scope.
   - **Functions**: declare the `FunctionValue`, push a function scope, emit parameter `alloca`s, then walk the body.
4. Verify the module with `module.verify()`.
5. Print to file with `module.print_to_file(...)`.

> Note: The module header comment mentions a two-phase scan, but the actual implementation performs symbol collection and IR generation in a single traversal because the pest tree is processed depth-first and symbols are added to the scope stack before any uses are encountered. [STATIC-INFERRED]

### 5.3 Inkwell Usage Patterns

| Pattern | Location |
|---------|----------|
| `context.i32_type()` | All integer types are 32-bit. [EXEC-VERIFIED] |
| `builder.build_alloca(i32_type, name)` | Local variables and parameters. [EXEC-VERIFIED] |
| `builder.build_store(ptr, value)` | Assignment and initialization. [EXEC-VERIFIED] |
| `builder.build_load(ptr, name)` | L-value reads. [EXEC-VERIFIED] |
| `builder.build_int_add/sub/mul/sdiv/srem` | Arithmetic. [EXEC-VERIFIED] |
| `builder.build_int_compare(...)` + `build_int_z_extend` | Relational and logical operations. [EXEC-VERIFIED] |
| `builder.build_conditional_branch(cond, true_bb, false_bb)` | Short-circuit `&&` / `\|\|`. [EXEC-VERIFIED] |
| `module.add_global(i32_type, None, name)` | Global variables. [EXEC-VERIFIED] |
| `module.add_function(name, fn_type, None)` | Function declarations. [EXEC-VERIFIED] |
| `builder.build_return(Some/None)` | Return statements; a default terminator is appended if the user code lacks one. [EXEC-VERIFIED] |

### 5.4 Scope Management in IR Generation

`ScopeStack<'ctx>` in `gen_llvm_ir.rs` mirrors the semantic checker design but uses Inkwell handles as keys [EXEC-VERIFIED]:

- `ScopeKey::Global` — global scope.
- `ScopeKey::Ident(FunctionValue<'ctx>)` — function scope.
- `ScopeKey::InnerBlock(BasicBlock<'ctx>)` — basic-block scope for `{ … }` blocks.

The `Type<'ctx>` enum stores `GlobalVar(PointerValue)`, `LocalVar(PointerValue)`, or `Func(FunctionValue)`. [EXEC-VERIFIED]

### 5.5 Key Functions

- `Scanner::new(input, output) -> Self` — wraps `IrCore::new()`. [EXEC-VERIFIED]
- `Scanner::scan_collect(&self) -> Result<(), String>` — generates `.ll` only. [EXEC-VERIFIED]
- `Scanner::scan_collect_asm(&self, ir_output, asm_build_fn) -> Result<(), String>` — generates `.ll`, then calls the provided `asm_build_fn` to produce assembly. [EXEC-VERIFIED]
- `Scanner::scan_func_def` — builds the LLVM function, entry block, parameter `alloca`s, and body. [EXEC-VERIFIED]
- `Scanner::scan_if_stmt` / `scan_while_stmt` — create basic blocks and branch instructions for control flow. [EXEC-VERIFIED]
- `Scanner::scan_cond_for_branches` / `scan_l_or_exp_for_branches` / `scan_l_and_exp_for_branches` — short-circuit boolean evaluation using block-level jumps. [EXEC-VERIFIED]

### 5.6 Invariants & Failure Modes

- **Invariant**: Every function ends with a terminator (`ret` or default `ret 0` / `ret void`). [EXEC-VERIFIED]
- **Failure mode**: If the source contains a syntax error, `scan_collect` returns `Err("语法解析失败")`. [EXEC-VERIFIED]
- **Failure mode**: If Inkwell IR verification fails, the error string is propagated upward. [EXEC-VERIFIED]

---

## 6. RISC-V Backend Functional Design (`src/riscv_codegen/`)

### 6.1 Inputs & Outputs

| Aspect | Description |
|--------|-------------|
| **Input** | SysY source string (`&str`) and output assembly path (`&str`). Internally it first generates LLVM IR via `Scanner::scan_collect_asm`. [EXEC-VERIFIED] |
| **Output** | RISC-V assembly text file (`.s`). |
| **Failure mode** | `generate_asm` returns `Result<(), String>`; errors from the LLVM phase are propagated. [EXEC-VERIFIED] |

### 6.2 Two-Pass Architecture

`parse_llvm_ir` in `src/riscv_codegen/mod.rs` drives a two-pass process per function [EXEC-VERIFIED]:

**Pass 1 — Analysis / Interval Collection (`FunctionState`)**:
- Record the instruction index of every basic block start (`record_block_start`).
- Detect `alloca` instructions (`record_allocated_val`).
- Detect backward branches to identify loops (`mark_loop`).
- For every non-terminator instruction, record `(idx, result_name, vec[use_names])` (`record_instruction`).
- Compute liveness intervals (`compute_liveness`): first and last use of each variable name. Loop-carried alloca variables have their interval extended to the end of the function.
- Filter out global variable names (they are fixed to `Location::Global`).

**Pass 2 — Instruction Emission**:
- Generate `.text`, `.globl`, function label, and prologue (`addi sp, sp, -N`).
- Emit a label for each basic block.
- Dispatch each LLVM instruction to a generator (`generate_instruction`).
- Generate epilogue (`addi sp, sp, N`) and `li a7, 93; ecall` on `Return`. [EXEC-VERIFIED]

### 6.3 Instruction Selection Strategy

The backend maps LLVM opcodes to RISC-V instructions via pattern matching in `generate_instruction` [EXEC-VERIFIED]:

| LLVM Opcode | RISC-V Emission |
|-------------|-----------------|
| `Add` / `Sub` / `Mul` / `SDiv` / `SRem` | `add` / `sub` / `mul` / `div` / `rem` |
| `ICmp(EQ/NE/SGT/SLT/SLE/SGE)` | `sub`+`seqz` / `sub`+`snez` / `sgt` / `slt` / `sle` / `sge` |
| `Load` | `la`+`lw` (global) or `lw` / `mv` (local) |
| `Store` | `la`+`sw` (global) or `sw` / `mv` (local) |
| `Br` (1 operand) | `j label` |
| `Br` (3 operands) | `bne cond, x0, true_label; j false_label` |
| `ZExt` | `mv` or `sw` (no-op because RISC-V registers are already 32-bit) |
| `Return` | load return value to `a0`, epilogue, `li a7, 93; ecall` |

Helper functions `get_value_from_reg` and `load_value_to_reg` resolve operand locations (register, stack offset, or global label) and emit the necessary load/move instructions. [EXEC-VERIFIED]

### 6.4 Register Allocation

#### Allocator Hierarchy

Located in `src/riscv_codegen/register_alloc.rs` [EXEC-VERIFIED]:

| Type | Algorithm |
|------|-----------|
| `NoAlloc` | Pure stack allocation; overlapping lifetimes receive distinct stack slots, non-overlapping lifetimes reuse slots. |
| `LinearScan` | Linear-scan register allocation with spill-to-stack. |
| `AllocatedInnerVar` | Dispatcher that chooses `NoAlloc` when `only_stack == true`, otherwise `LinearScan`. |

#### Register Pool

`LinearScan::new()` initializes the pool in priority order [EXEC-VERIFIED]:

1. Temporary registers: `t3`, `t4`, `t5`, `t6` (`t0`–`t2` are reserved for operand/result temporaries).
2. Saved registers: `s0`–`s11`.
3. Argument registers: `a2`–`a7` (`a0`–`a1` are reserved for return values / syscall).

#### Spill Strategy

`LinearScan::overflow_to_stack` implements a **spill-farthest** heuristic [EXEC-VERIFIED]:

- If the active variable with the latest end-offset ends after the current variable, spill that active variable and give its register to the current one.
- Otherwise, spill the current variable directly.
- Spilled variables are placed on the stack at negative offsets (later converted to positive offsets from the frame bottom).

#### Stack Alignment

The prologue/epilogue size is aligned to 16 bytes: `((stack_size + 15) / 16) * 16`. [EXEC-VERIFIED]

### 6.5 AsmBuilder

`AsmBuilder` in `src/riscv_codegen/asm_builder.rs` is a string-buffer wrapper that emits formatted RISC-V text [EXEC-VERIFIED]:

- **Section / symbol directives**: `emit_data_section`, `emit_text_section`, `emit_global_symbol`, `emit_label`, `emit_word`.
- **Function frame**: `emit_function_prologue`, `emit_function_epilogue`, `emit_exit_syscall`.
- **ALU**: `emit_add`, `emit_sub`, `emit_mul`, `emit_div`, `emit_rem`, `emit_addi`, `emit_subi`.
- **Memory**: `emit_lw`, `emit_sw`.
- **Branch / compare**: `emit_beq`, `emit_bne`, `emit_blt`, `emit_bgt`, `emit_ble`, `emit_bge`, `emit_slt`, `emit_sgt`, `emit_seqz`, `emit_snez`, `emit_sle`, `emit_sge`.
- **Jump**: `emit_j`, `emit_jal`, `emit_jr`.
- **Move**: `emit_mv`.
- **System call**: `emit_ecall`, `emit_syscall`.
- **Retrieve buffer**: `emit()` returns the accumulated `String`. [EXEC-VERIFIED]

### 6.6 Invariants & Failure Modes

- **Invariant**: Global variables are never passed to the register allocator; they remain `Location::Global`. [EXEC-VERIFIED]
- **Invariant**: `t0`, `t1`, `t2` are never allocated to user SSA values; they are reserved for operand loading and temporary results. [STATIC-INFERRED]
- **Failure mode**: If the LLVM phase fails, no assembly is emitted. [EXEC-VERIFIED]
- **Failure mode**: Unknown LLVM opcodes in `generate_instruction` are silently skipped (no panic). [EXEC-VERIFIED]

---

## 7. CLI Functional Design (`src/main.rs`)

### 7.1 Subcommand Specifications

The CLI is built with `clap` and exposes the following interface [EXEC-VERIFIED]:

| Subcommand | Arguments | Behavior |
|------------|-----------|----------|
| `tokenize` | `file: String` | Reads file, calls `lexer::tokenize(&input)` which writes to `stderr`. |
| `fmt` | `file: String` | Reads file, calls `format::fmt(&input)` which writes to `stdout`. |
| `check` | `file: String` | Reads file, creates `Checker`, runs `syn_check()`, writing errors to `stderr`. |
| `gen-ir` | `input: String`, `--output <String>` | Reads input, creates `Scanner`, runs `scan_collect()` to write LLVM IR. |
| `gen-asm` | `input: String`, `--output <String>` | Reads input, runs `generate_asm()` to write RISC-V assembly. |

### 7.2 Compatibility Mode

If the binary is invoked with two bare positional arguments (no subcommand) and the first argument does not start with `-` and is not a known subcommand name, it is treated as `gen-asm <input> <output>` [EXEC-VERIFIED]:

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

This is also mirrored in `Cli` via `input: Option<String>` and `output: Option<String>` parsed by clap when no subcommand is present. [EXEC-VERIFIED]

### 7.3 Error Handling and Logging

- **tklog**: `log_init()` configures `tklog::LOG` with console output, level `Info`, and a formatter showing level, time, short file name, and message. [EXEC-VERIFIED]
- **Fatal errors**: File read failures and backend errors call `eprintln!` and `std::process::exit(1)`. [EXEC-VERIFIED]
- **Silent assumption**: `gen_llvm_ir.rs` and `riscv_codegen/` do not write to `stderr` in production paths to avoid Online Judge output pollution. [STATIC-INFERRED]

### 7.4 Key Functions

- `read_file(path: &str) -> String` — reads a file or exits with code 1. [EXEC-VERIFIED]
- `run_gen_asm(input: &str, output: &str)` — initializes logging and drives `generate_asm`. [EXEC-VERIFIED]
- `main()` — parses CLI, dispatches to subcommands, handles compatibility mode. [EXEC-VERIFIED]

---

## 8. Test Functional Design

### 8.1 Test Organization

Tests are organized as fixture files plus expected outputs (where applicable) under `tests/` [EXEC-VERIFIED]:

| Directory | Purpose | Validation Method |
|-----------|---------|-------------------|
| `tests/lexer/` | `.in` source files + `.out` expected token/error output | Unit tests in `src/lexer.rs` compare buffer output against `.out` files. |
| `tests/formatter/` | `.txt` input files | Unit tests in `src/format.rs` run formatting and assert success. |
| `tests/semantic/` | `.sy` source files + `.out` expected error output | Unit tests in `src/check.rs` compare `Checker` output against `.out` files. |
| `tests/llvm_ir/` | `.sy` source files + `.ll` official/reference IR + `_out.ll` generated IR | Unit tests in `src/gen_llvm_ir.rs` compare execution results via `lli-14`. |
| `tests/codegen/` | `.sy` source files + `target_*.s` baseline assembly | End-to-end Rust tests in `tests/end_to_end_codegen.rs` compare against baselines using RARS. |

### 8.2 End-to-End Validation Strategy (RARS Simulator)

The `tests/end_to_end_codegen.rs` integration test suite validates the RISC-V backend using the RARS simulator (`tests/codegen/rars.jar`) [EXEC-VERIFIED]:

1. **Compilation**: Invoke the compiler binary (`env!("CARGO_BIN_EXE_COMPILER")`) with input/output paths to produce `.s` files. [EXEC-VERIFIED]
2. **Simulation**: Run `timeout 5s java -jar rars.jar <asm> a0` to execute the generated program and capture the `a0` register value. [EXEC-VERIFIED]
3. **Correctness checks**:
   - `codegen_program_terminates_with_correct_result`: Generated and target assembly must both produce `a0 == 0x0000000e` (14) for the Euclidean algorithm fixture. [EXEC-VERIFIED]
   - `codegen_program_does_not_timeout`: Asserts the simulator does not return exit code 124 (`timeout`). [EXEC-VERIFIED]
   - `codegen_control_flow_branches_correct`: Inspects labels (`whileCond`, `whileBody`, `if_true`, `if_false`) to verify branch targets. [EXEC-VERIFIED]
4. **Performance budgets**:
   - `codegen_stack_frame_within_budget`: Generated stack frame size must not exceed the baseline `target_register_alloc.s`. [EXEC-VERIFIED]
   - `codegen_load_store_count_within_budget`: Generated load/store count must not exceed baseline + 20. [EXEC-VERIFIED]

### 8.3 LLVM IR Validation

Inside `src/gen_llvm_ir.rs` (test module), the test helper `execute_llvm_ir` runs `lli-14` on generated `.ll` files. Tests compare either against an official `.ll` file (`compare_execution_results`) or against an expected return code embedded in a `// output N` comment in the source (`compare_with_expected_output`). [EXEC-VERIFIED]

### 8.4 Invariants & Failure Modes

- **Invariant**: All `.out` fixture files use platform-appropriate line endings (tests normalize `\r\n` to `\n` before comparison). [EXEC-VERIFIED]
- **Failure mode**: If `lli-14` or RARS is missing, the corresponding integration tests will fail at runtime. [STATIC-INFERRED]
- **Failure mode**: Unit tests that ignore expected-output comparison (e.g., many formatter tests) only assert that formatting does not panic; they do not verify textual correctness. [PARTIAL]

---

## Evidence Legend

- **[EXEC-VERIFIED]** — Behavior confirmed by direct inspection of source code, unit tests, or integration tests in the repository.
- **[STATIC-INFERRED]** — Behavior deduced from source structure, comments, or cross-module call graph without explicit runtime verification.
- **[PARTIAL]** — Behavior partially implemented or partially tested; edge cases may not be fully covered.
