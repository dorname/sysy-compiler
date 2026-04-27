---
name: Implementation Reference
description: Comprehensive implementation guide for the SysY→RISC-V compiler
commit: f5b8eb08d74b85cdf4c78c4ecfdae4880dcfeb1c
---

# Implementation Reference

This document describes the internal architecture, data structures, algorithms, and build/test procedures for the SysY→RISC-V compiler. All line citations are pinned to commit `f5b8eb08d74b85cdf4c78c4ecfdae4880dcfeb1c`.

---

## 1. Environment Setup

### Prerequisites

- **Rust toolchain** — latest stable channel (`rustc` + `cargo`).
- **LLVM 14** development libraries.
- **Java** runtime (for RARS simulator in end-to-end tests).

### LLVM 14 Installation

```bash
sudo apt-get update
sudo apt-get install llvm-14-dev libpolly-14-dev zlib1g-dev
```

> See `README.md` for up-to-date OS-specific instructions.

### Repository Layout

```
src/
├── main.rs                      # CLI entry point
├── lexer.rs                     # Tokenizer
├── format.rs                    # Code formatter
├── check.rs                     # Semantic checker
├── gen_llvm_ir.rs               # LLVM IR generator
├── riscv_codegen/
│   ├── mod.rs                   # RISC-V codegen driver
│   ├── register_alloc.rs        # Linear scan allocator
│   └── asm_builder.rs           # Assembly text builder
├── utils.rs                     # Hex/oct conversion helpers
└── tests/                       # Unit & integration tests
```

---

## 2. Build & Run

### Release Build

```bash
cargo build --release
```

The resulting binary is at `target/release/compiler`.

### CLI Subcommands

| Subcommand | Purpose | Example invocation |
|------------|---------|--------------------|
| `tokenize` | Lexical analysis → token stream | `cargo run -- tokenize tests/lexer/simple.in` |
| `fmt` | Pretty-print SysY source | `cargo run -- fmt tests/formatter/input1.txt` |
| `check` | Semantic analysis (types + scopes) | `cargo run -- check tests/semantic/normaltest01.sy` |
| `gen-ir` | Emit LLVM IR | `cargo run -- gen-ir tests/llvm_ir/example01.sy -o out.ll` |
| `gen-asm` | Emit RISC-V assembly | `cargo run -- gen-asm tests/codegen/register_alloc.sy -o out.s` |

### Compatibility Mode

Omitting subcommands and passing two positional arguments defaults to `gen-asm`:

```bash
cargo run -- tests/codegen/register_alloc.sy out.s
```

The dispatch logic lives in <code>src/main.rs:85-142</code>.

---

## 3. Core Data Structures

### 3.1 Token & Related Enums

**File:** <code>src/lexer.rs:9-182</code>

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

- **`Token`** — unified lexical token. Each variant wraps a more specific enum.
- **`Operator`** (<code>src/lexer.rs:128-152</code>) — 21 operators from `+` to `;`.
- **`Flow`** (<code>src/lexer.rs:86-93</code>) — control-flow keywords (`if`, `else`, `while`, `break`, `continue`, `return`).
- **`Type`** (<code>src/lexer.rs:109-114</code>) — `int`, `float`, `void`, `const`.
- **`IntegerConst`** (<code>src/lexer.rs:62-66</code>) — decimal, octal, or hexadecimal integer literals.
- **`ErrorSyntax`** (<code>src/lexer.rs:39-46</code>) — lexical error variants (invalid hex, invalid octal, unknown operator, etc.).

### 3.2 Formatter

**File:** <code>src/format.rs:20-29</code>

```rust
pub struct Formatter<'a, W: Write> {
    deep: usize,                       // current indentation depth
    input: &'a str,                    // source text
    writer: &'a mut W,                 // output sink
    output: String,                    // accumulated formatted text
    err_output: String,                // syntax-error messages
    forbid_newline: bool,              // suppress newline (e.g. inside Decl)
    forbid_semicolon_newline: bool,    // suppress newline after semicolon
    is_first_function: bool,           // controls blank line before functions
}
```

### 3.3 Checker & Semantic Types

**File:** <code>src/check.rs:47-58</code>

```rust
pub struct Checker<'a, W: Write> {
    input: &'a str,                    // source text
    writer: &'a mut W,                 // diagnostic output
    output: String,                    // buffered diagnostics
    scope_stack: ScopeStack,           // nested scope manager
    errors: Vec<SemanticError<'a>>,    // collected semantic errors
}
```

**File:** <code>src/check.rs:1016-1021</code>

```rust
pub struct ScopeStack {
    stack: Vec<ScopeKey>,              // scope key stack
    scopes: HashMap<ScopeKey, Scope>,  // map key → symbol table
}
```

**File:** <code>src/check.rs:1137-1142</code>

```rust
pub enum Type {
    Int,
    Func(Func),
    Void,
    Array(ArrayStruct),
}
```

**File:** <code>src/check.rs:1180-1186</code>

```rust
pub struct Func {
    pub params: Vec<Type>,
    pub return_type: Box<Type>,
}
```

**File:** <code>src/check.rs:1200-1211</code>

```rust
pub struct ArrayStruct {
    pub item_type: Box<Type>,
    pub dim_nos: Vec<usize>,
    pub tpl_size: HashMap<usize, usize>,
    pub array_dims: usize,
    pub crr_no: usize,
}
```

**File:** <code>src/check.rs:1275-1300</code>

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

### 3.4 IR Generation Types

**File:** <code>src/gen_llvm_ir.rs:58-63</code>

```rust
pub struct Scanner<'a> {
    input: &'a str,
    output: &'a str,
    ir_core: IrCore,
}
```

`Scanner` is the top-level driver for LLVM IR generation. It parses SysY source with `pests/scan.pest`, walks the AST, and emits LLVM instructions via `inkwell`.

**File:** <code>src/gen_llvm_ir.rs:1780-1789</code>

```rust
pub struct IrCore {
    context: Context,
}

impl IrCore {
    pub fn start_session(&self, module_name: &str) -> IrSession<'_> { ... }
}
```

**File:** <code>src/gen_llvm_ir.rs:1803-1808</code>

```rust
pub struct IrSession<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    pub scope_stack: ScopeStack<'ctx>,
}
```

`IrSession` bundles the LLVM context, module, IR builder, and symbol tables for one compilation unit.

**File:** <code>src/gen_llvm_ir.rs:1835-1840</code>

```rust
pub struct ScopeStack<'ctx> {
    stack: Vec<ScopeKey<'ctx>>,
    scopes: HashMap<ScopeKey<'ctx>, Scope<'ctx>>,
}
```

**File:** <code>src/gen_llvm_ir.rs:1961-1965</code>

```rust
pub enum Type<'ctx> {
    GlobalVar(PointerValue<'ctx>),
    Func(FunctionValue<'ctx>),
    LocalVar(PointerValue<'ctx>),
}
```

### 3.5 RISC-V Codegen Types

**File:** <code>src/riscv_codegen/mod.rs:67-74</code>

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

- `var_locations` — maps LLVM value names to `Location` (register, stack slot, or global label).
- `pure_stack_mode` — when `true`, bypasses register allocation and places every variable on the stack.

**File:** <code>src/riscv_codegen/mod.rs:199-205</code>

```rust
struct FunctionState {
    instructions: Vec<(usize, String, Vec<String>)>,
    allocated_vals: HashSet<String>,
    block_start_idxes: HashMap<String, usize>,
    loop_branch: bool,
    idx: usize,
}
```

`FunctionState` is a per-function accumulator used in the **first pass** to collect def/use information and detect backward branches (loops).

**File:** <code>src/riscv_codegen/register_alloc.rs:8-12</code>

```rust
pub struct InnerVar {
    name: String,
    start_offset: usize,
    end_offset: usize,
}
```

Represents a variable’s live interval by instruction index.

**File:** <code>src/riscv_codegen/register_alloc.rs:49-53</code>

```rust
pub enum Location {
    Reg(String),
    Stack(i32),
    Global(String),
}
```

**File:** <code>src/riscv_codegen/register_alloc.rs:137-150</code>

```rust
pub struct LinearScan {
    regs: Vec<String>,              // free register pool
    active_vars: BTreeSet<InnerVar>,
    var_reg_map: HashMap<String, String>,
    stack_offset: i32,
    stack_slots: Vec<(i32, String)>,
    stack_map: HashMap<String, (i32, String)>,
}
```

### 3.6 AsmBuilder

**File:** <code>src/riscv_codegen/asm_builder.rs:2-4</code>

```rust
pub struct AsmBuilder {
    buf: String,
}
```

`AsmBuilder` is a thin wrapper around a `String`. It provides `emit_*` methods for every RISC-V pseudo-instruction and directive used by the backend (e.g. `emit_add`, `emit_lw`, `emit_function_prologue`).

---

## 4. Key Algorithms

### 4.1 Lexical Analysis Pipeline

**Files & ranges:**
- <code>src/lexer.rs:184-202</code> — `parse_file`
- <code>src/lexer.rs:204-234</code> — `tokenizer`
- <code>src/lexer.rs:241-321</code> — `impl From<Pair<Rule>> for Token`

**Description:**
The lexer is a **parser-generator-backed tokenizer** rather than a hand-written DFA. `pest` grammar `pests/lexer.pest` defines token rules. `ExpressionParser::parse(Rule::File, input)` produces a tree of `Pair`s, which are then flattened into a `Vec<(usize, Token)>`.

**Step-by-step:**
1. Call `parse_file(input)` → invoke `pest` parser on the whole file.
2. Unwrap the top-level `File` pair and iterate over its children (<code>src/lexer.rs:209-220</code>).
3. Skip `Rule::EOI`; for every other pair, read `pair.line_col().0` and convert via `Token::from(pair)`.
4. If any `ErrorSyntax` tokens exist, print error diagnostics; otherwise print the token stream (<code>src/lexer.rs:221-233</code>).
5. `From<Pair>` dispatches on `Rule` (`Operator`, `Flow`, `Type`, `IntegerConst`, `ErrorSyntax`) and recurses one level to extract the concrete lexeme.

### 4.2 Formatting Recursive Descent

**Files & ranges:**
- <code>src/format.rs:45-228</code> — `Formatter::fmt`
- <code>src/format.rs:369-392</code> — `Formatter::format_code`
- <code>src/format.rs:280-367</code> — blank-line cleanup

**Description:**
`Formatter` walks the `pest` parse tree (`pests/parser.pest`) and emits pretty-printed SysY source. It is **not** a separate AST pretty-printer; it formats directly from `Pair<Rule>` nodes.

**Step-by-step:**
1. `format_code` unwraps the nested `File → CompUnit` structure and calls `self.fmt(pair)` for every top-level declaration (<code>src/format.rs:369-383</code>).
2. `fmt` matches on `Rule`:
   - Container nodes (`FuncDef`, `VarDecl`, `Block`, etc.) recurse into children.
   - `Rule::Stmt` handles special indentation rules for `if` / `else` bodies (<code>src/format.rs:94-133</code>).
   - `Rule::OpenBrace` increases `deep` and appends a newline + indent; `Rule::CloseBrace` decreases `deep`.
   - Operators are wrapped in spaces; semicolons trigger newline insertion unless suppressed.
3. After the walk, `clean_extra_blank_lines` removes redundant blank lines while preserving single blank lines between function definitions (<code>src/format.rs:280-367</code>).

### 4.3 Semantic Checking Passes

**Files & ranges:**
- <code>src/check.rs:75-96</code> — `syn_check`
- <code>src/check.rs:108-124</code> — `analyze_declaration`
- <code>src/check.rs:130-142</code> — `analyze_var_decl`
- <code>src/check.rs:144-193</code> — `analyze_func_def`
- <code>src/check.rs:256-305</code> — `analyze_assign_stmt`
- <code>src/check.rs:307-405</code> — expression type checking (`analyze_exp` … `analyze_unary_op_exp`)
- <code>src/check.rs:407-486</code> — `analyze_call_exp`

**Description:**
The checker performs a **single-pass recursive descent** over the `pests/check.pest` parse tree. It maintains a `ScopeStack` (global → function → inner blocks) and collects 11 categories of semantic errors.

**Step-by-step:**
1. `syn_check` parses the file, unwraps `CompUnit`, and iterates over declarations (<code>src/check.rs:76-88</code>).
2. For each declaration:
   - `VarDecl` / `ConstDecl` → `analyze_var_decl` → `analyze_def`.
   - `FuncDef` → push function scope, parse parameters, parse body, pop scope (<code>src/check.rs:144-193</code>).
3. Expressions are type-checked bottom-up:
   - `analyze_add_exp` / `analyze_mul_exp` verify both operands are `Int` (or compatible arrays) and propagate `Type::Int`.
   - `analyze_call_exp` validates the function exists, counts parameters, and checks argument types (<code>src/check.rs:407-486</code>).
4. Errors are collected into `Vec<SemanticError>` and emitted in source order at the end (<code>src/check.rs:98-106</code>).

### 4.4 Two-Phase IR Generation

**Files & ranges:**
- <code>src/gen_llvm_ir.rs:122-156</code> — `scan_collect`
- <code>src/gen_llvm_ir.rs:377-482</code> — `scan_func_def`
- <code>src/gen_llvm_ir.rs:663-766</code> — `scan_if_stmt`
- <code>src/gen_llvm_ir.rs:768-816</code> — `scan_while_stmt`
- <code>src/gen_llvm_ir.rs:850-928</code> — arithmetic expression lowering

**Description:**
IR generation is **not split into frontend AST + backend IR**; it lowers directly from `pest` pairs to LLVM instructions using `inkwell`.

**Step-by-step:**
1. `scan_collect` creates an `IrSession` (context + module + builder + scope stack) and walks declarations (<code>src/gen_llvm_ir.rs:135-142</code>).
2. `scan_func_def`:
   - Builds LLVM function type from `Rule::Int` / `Rule::Void`.
   - Adds the function to the module; stores it in the current scope.
   - Pushes a function scope; appends an entry `BasicBlock`.
   - Allocates stack slots for parameters via `build_alloca` and stores param values.
   - Walks body items; on exit inserts a default `ret` if the block lacks a terminator (<code>src/gen_llvm_ir.rs:468-479</code>).
3. Control flow:
   - `if` creates `if_true`, `if_false`, `if_next` blocks; evaluates condition with `scan_cond_for_branches` and emits `br` (<code>src/gen_llvm_ir.rs:663-766</code>).
   - `while` creates `whileCond`, `whileBody`, `whileNext`; emits unconditional branch to cond, then conditional branch to body/next (<code>src/gen_llvm_ir.rs:768-816</code>).
4. Expressions:
   - `scan_add_exp` / `scan_mul_exp` iterate over left-associative chains and emit `build_int_add`, `build_int_sub`, `build_int_mul`, `build_int_signed_div`, `build_int_signed_rem` (<code>src/gen_llvm_ir.rs:850-928</code>).

### 4.5 Linear Scan Register Allocation

**Files & ranges:**
- <code>src/riscv_codegen/register_alloc.rs:266-289</code> — `clear_inactive_vars`
- <code>src/riscv_codegen/register_alloc.rs:319-339</code> — `overflow_to_stack`
- <code>src/riscv_codegen/register_alloc.rs:410-450</code> — `LinearScan::allocate`

**Description:**
The allocator receives a list of `InnerVar` live intervals (computed in `FunctionState::compute_liveness` at <code>src/riscv_codegen/mod.rs:256-296</code>) and maps each variable to either a physical register or a stack slot.

**Pseudocode:**
```
proc allocate(intervals):
    sort intervals by start_offset ascending
    for var in intervals:
        clear_inactive_vars(var)          // free regs / stack slots whose end < var.start
        if free_registers not empty:
            reg = pop_reg()                // priority: t3-t6 > s0-s11 > a2-a7
            assign var → reg
            active_vars.insert(var)
        else:
            overflow_to_stack(var)         // spill farthest-ending interval
    convert negative stack offsets to positive
    return (var→location_map, stack_size)
```

- **Spill heuristic** (`overflow_to_stack`): compare the latest-ending active variable with the current variable. If the active variable ends later, spill it and give its register to the current variable; otherwise spill the current variable.
- **Register priority**: temporary registers `t3–t6` are preferred, then saved registers `s0–s11`, then argument registers `a2–a7` (`a0–a1` are reserved for return values).

### 4.6 RISC-V Instruction Lowering (Two-Pass)

**Files & ranges:**
- <code>src/riscv_codegen/mod.rs:42-64</code> — `parse_llvm_ir`
- <code>src/riscv_codegen/mod.rs:321-459</code> — `build_function`
- <code>src/riscv_codegen/mod.rs:601-635</code> — `generate_instruction`
- <code>src/riscv_codegen/mod.rs:256-296</code> — `FunctionState::compute_liveness`

**Description:**
The backend operates in **two passes per function**.

**Pass 1 — Analysis:**
1. Enumerate basic blocks and record each block’s start instruction index (<code>src/riscv_codegen/mod.rs:346-354</code>).
2. Walk every instruction:
   - Record `alloca` names in `allocated_vals`.
   - Detect backward branches (loops) by comparing branch target indices to the current block start (<code>src/riscv_codegen/mod.rs:466-525</code>).
   - Skip terminators (`ret`, `br`).
   - Record `(idx, def_name, use_names)` for all other instructions (<code>src/riscv_codegen/mod.rs:386-392</code>).
3. Compute liveness: for each variable, `first_use` and `last_use` instruction indices form the live interval. If a loop is present, alloca variables are extended to the function’s max index (<code>src/riscv_codegen/mod.rs:256-296</code>).
4. Filter out global names (they are pre-mapped to `Location::Global`) and run register allocation (<code>src/riscv_codegen/mod.rs:400-417</code>).

**Pass 2 — Emission:**
1. Emit `.text`, `.globl`, function label, and prologue (`addi sp, sp, -N`) (<code>src/riscv_codegen/mod.rs:427-443</code>).
2. For each instruction call `generate_instruction`, which dispatches on `InstructionOpcode`:
   - `Add/Sub/Mul/SDiv/SRem` → `generate_cal_instruction` (<code>src/riscv_codegen/mod.rs:1050-1148</code>)
   - `ICmp` → `generate_icmp_instruction` (<code>src/riscv_codegen/mod.rs:1295-1416</code>)
   - `Load` → `generate_load_instruction` (<code>src/riscv_codegen/mod.rs:721-792</code>)
   - `Store` → `generate_store_instruction` (<code>src/riscv_codegen/mod.rs:1191-1241</code>)
   - `Br` → `generate_br_instruction` (<code>src/riscv_codegen/mod.rs:854-892</code>)
   - `ZExt` → `generate_zext_instruction` (<code>src/riscv_codegen/mod.rs:952-980</code>)
   - `Return` → load result into `a0`, emit epilogue, `li a7, 93; ecall` (<code>src/riscv_codegen/mod.rs:638-664</code>)

---

## 5. Dependency Rationale

| Crate | Version | Role | Rationale |
|-------|---------|------|-----------|
| **pest** | 2.8.1 | Parser generator | Chosen to avoid hand-writing lexers/parsers for five separate lab stages. PEG grammars (`pests/*.pest`) are concise and maintainable. |
| **pest_derive** | 2.8.1 | Derive macro for pest | Required by `pest` to generate `Parser` implementations at compile time. |
| **inkwell** | 0.6.0 (llvm14-0) | LLVM IR builder | Safe Rust bindings over LLVM-C API. Enables SSA construction, basic blocks, and instruction emission without writing raw IR text. |
| **clap** | 4.5.45 | CLI argument parsing | Derive-based (`#[derive(Parser, Subcommand)]`) keeps `main.rs` short and self-documenting. |
| **num-bigint** | 0.4.6 | Arbitrary-precision integers | SysY allows hexadecimal and octal constants that may exceed `i32` during parsing; `BigInt` is used in `utils.rs:4-20` for safe conversion before truncation. |
| **num-traits** | 0.2.19 | Traits for num-bigint | Required companion to `num-bigint` for radix parsing. |
| **tklog** | 0.3.0 | Structured logging | Used in RISC-V codegen for development-time trace output (file + line metadata). |
| **log** | 0.4.29 | Logging facade | Standard interface consumed by `tklog`. |

---

## 6. Known Debt

### 6.1 Oversized Modules

| File | Lines | Concern |
|------|-------|---------|
| <code>src/gen_llvm_ir.rs</code> | 2,250 | Mixes parsing, scope management, type checking, and LLVM emission in one file. |
| <code>src/check.rs</code> | 1,621 | Semantic analysis + symbol tables + error formatting all inline. |
| <code>src/riscv_codegen/mod.rs</code> | 2,069 | Contains liveness analysis, register allocation orchestration, and per-opcode lowering. |

### 6.2 Tight Coupling Between Checker and IR Gen

There is **no shared AST**. Both `check.rs` and `gen_llvm_ir.rs` maintain independent `pest` grammars (`check.pest` and `scan.pest`) and independently traverse `Pair` trees. This means:
- Grammar changes must be kept in sync manually.
- The checker cannot reuse IR-gen symbol tables, and vice-versa.
- Type information computed during semantic analysis is discarded; IR generation re-derives everything from parse-tree shape.

### 6.3 TODOs / FIXMEs

- <code>src/riscv_codegen/mod.rs:606</code>  
  `// TODO: 根据instruction类型生成相应代码` — placeholder comment inside `generate_instruction`; the actual dispatch is implemented below it, so the TODO is stale.
- <code>src/check.rs:810-816</code>  
  `// TODO 实验阶段 暂时不处理` — array initialization dimension check is commented out.

### 6.4 Missing `lib.rs` / Pure Binary Architecture

<code>src/main.rs</code> declares modules with `mod check; mod format; …` and contains all CLI logic. There is **no `lib.rs`**, which means:
- The compiler cannot be consumed as a library crate.
- Integration tests in `tests/` must invoke the compiled binary (`env!("CARGO_BIN_EXE_compiler")`) rather than linking against a library.
- Internal unit tests are scattered inside `#[cfg(test)]` blocks in each source file.

### 6.5 Temporary / Hard-Coded Structures

- `gen_llvm_ir.rs` hard-codes `i32` as the only integer type; SysY `float` exists in the lexer but is not lowered.
- `LinearScan` reserves `t0–t2` as scratch registers and never allocates them to variables, reducing the effective register pool.
- `AsmBuilder::emit_subi` (<code>src/riscv_codegen/asm_builder.rs:68-70</code>) emits a pseudo-instruction that may not be supported by all RISC-V assemblers; it is currently unused in generated code.

---

## 7. Test Execution Guide

### 7.1 Unit Tests

Every major module contains inline `#[cfg(test)]` tests.

```bash
# Run all unit tests
cargo test

# Run a specific module's tests
cargo test --lib lexer
cargo test --lib check
cargo test --lib gen_llvm_ir
cargo test --lib riscv_codegen
```

### 7.2 End-to-End Codegen Tests

The integration test suite <code>tests/end_to_end_codegen.rs</code> validates the full `SysY → RISC-V assembly → RARS` pipeline.

**Prerequisites:**
- `java` on `$PATH`
- <code>tests/codegen/rars.jar</code> present

**Run:**

```bash
cargo test --test end_to_end_codegen -- --nocapture
```

**What it checks:**
1. **Stack frame budget** (`codegen_stack_frame_within_budget`) — generated `addi sp, sp, -N` must not exceed the reference baseline.
2. **Load/store traffic budget** (`codegen_load_store_count_within_budget`) — count of `lw`/`sw` must stay within target + 20.
3. **Correct runtime result** (`codegen_program_terminates_with_correct_result`) — compiles `euclidean_algorithm.sy`, runs in RARS, and asserts `a0 == 0x0000000e` (14).
4. **No timeout** (`codegen_program_does_not_timeout`) — RARS must finish within 5 seconds.
5. **Control-flow branch targets** (`codegen_control_flow_branches_correct`) — inspects assembly text to ensure `whileCond` branches to `whileBody` and `whileNext`, and the `if` inside the loop branches to `if_true` / `if_false`.

### 7.3 Manual RARS Verification

```bash
cargo run -- gen-asm tests/codegen/euclidean_algorithm.sy -o /tmp/euclid.s
java -jar tests/codegen/rars.jar /tmp/euclid.s a0
```

Look for `a0` in the RARS output window; it should read `0x0000000e`.

---

*Document generated for commit `f5b8eb08d74b85cdf4c78c4ecfdae4880dcfeb1c`.*
