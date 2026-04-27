---
title: "Steel-Thread Trace"
description: "End-to-end trace of one complete gen-asm compilation path for a minimal SysY program"
commit_pin: "f5b8eb08d74b85cdf4c78c4ecfdae4880dcfeb1c"
---

# Steel-Thread Trace

## Minimal SysY Program Used

```c
int main() {
    return 1 + 2;
}
```

The program above compiled successfully on the first attempt. No simplification was required.

## Command Outputs

### Token Stream (`tokenize`)

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

### Semantic Check (`check`)

(No output; program passes semantic analysis without errors.)

### Formatted Source (`fmt`)

```c
int main() {
    return 1 + 2;
}
```

### Generated LLVM IR (`gen-ir`)

```llvm
; ModuleID = 'module'
source_filename = "module"

define i32 @main() {
mainEntry:
  ret i32 3
}
```

### Generated RISC-V Assembly (`gen-asm`)

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

## Traceability Matrix

| # | Phase | Input | Transformation | Function / Type | Output |
|---|-------|-------|----------------|-----------------|--------|
| 1 | **Lexer** | Source string (`&str`) | Pest grammar tokenization | `lexer::tokenize` → `lexer::tokenizer` in `src/lexer.rs` | Token stream printed to stderr |
| 2 | **Lexer Grammar** | Raw source characters | PEG matching | `ExpressionParser` (derived from `pest_derive::Parser`) using `src/pests/lexer.pest` | `Pairs<'_, Rule>` iterator over tokens |
| 3 | **Parser (Formatter)** | Source string (`&str`) | Pest grammar parsing into parse tree | `format::fmt` → `Formatter::format_code` in `src/format.rs` | Formatted SysY source code |
| 4 | **Parser Grammar (Formatter)** | Raw source characters | PEG matching | `FParser` (derived from `pest_derive::Parser`) using `src/pests/parser.pest` | `Pairs<'_, Rule>` representing the full parse tree |
| 5 | **Semantic Checker** | Source string (`&str`) | Parse + type / scope validation | `Checker::syn_check` in `src/check.rs` | Error messages (empty for valid input) |
| 6 | **Semantic Analysis** | `Pairs<'_, Rule>` from `CParser` | Symbol-table construction, scope-stack management, type checking | `Checker::analyze_declaration` → `Checker::analyze_func_def` → `Checker::analyze_var_decl` in `src/check.rs` | Checked symbol tables and collected semantic errors |
| 7 | **LLVM IR Generation** | Source string (`&str`) | Parse → LLVM-IR construction via `inkwell` | `Scanner::scan_collect` in `src/gen_llvm_ir.rs` | LLVM IR written to `.ll` file |
| 8 | **LLVM IR Scanning** | `Pairs<'_, Rule>` from `CParser` (using `src/pests/scan.pest`) | Symbol collection + IR emission | `Scanner::scan_declaration` → `Scanner::scan_func_def` → `Scanner::collect_func_params` in `src/gen_llvm_ir.rs` | Populated `IrSession` (`Module` + `Builder`) |
| 9 | **LLVM IR Session** | `Context` from `IrCore` | Module and builder creation | `IrCore::start_session` returning `IrSession<'_>` in `src/gen_llvm_ir.rs` | LLVM `Module` and `Builder` ready for instruction emission |
| 10 | **RISC-V Codegen Driver** | Source string (`&str`) | Two-phase pipeline: generate LLVM IR → parse IR → emit assembly | `generate_asm` in `src/riscv_codegen/mod.rs` | RISC-V assembly written to `.s` file |
| 11 | **RISC-V IR Parsing** | `IrSession` (LLVM `Module`) | Extract globals, functions, basic blocks, instructions | `parse_llvm_ir` → `build_function` → `build_global_variable` in `src/riscv_codegen/mod.rs` | Populated `AsmBuilder` buffer |
| 12 | **Instruction Selection** | LLVM `InstructionValue`s | Opcode dispatch to RISC-V instruction patterns | `generate_instruction` in `src/riscv_codegen/mod.rs` | RISC-V instructions appended to `AsmBuilder` |
| 13 | **Register Allocation** | `InnerVar` lifetime intervals | Linear-scan allocation (registers + stack spill) | `LinearScan::allocate` (impl `RegisterAllocator`) in `src/riscv_codegen/register_alloc.rs` | `HashMap<String, String>` mapping SSA names to registers or stack slots |
| 14 | **Assembly Emission** | `AsmBuilder` internal buffer | String concatenation of all emitted directives / instructions | `AsmBuilder::emit` in `src/riscv_codegen/asm_builder.rs` | Final `.s` file string |

## Contract Coverage Matrix

| # | Interface Contract | Coverage | Notes |
|---|-------------------|----------|-------|
| 1 | **Parser → AST** | `[EXEC-VERIFIED]` | All five subcommands (`tokenize`, `fmt`, `check`, `gen-ir`, `gen-asm`) successfully parsed the source. The parser grammar (`parser.pest`, `check.pest`, `scan.pest`, `lexer.pest`) produces a `Pairs<'_, Rule>` parse tree on every path. |
| 2 | **AST → Semantic Checker** | `[EXEC-VERIFIED]` | `Checker::syn_check` consumes the parse tree, builds scope stacks, and validates types. The `check` subcommand produced zero errors for the minimal program, confirming the contract is exercised. |
| 3 | **Checker → LLVM IR** | `[STATIC-INFERRED]` | The `gen-ir` and `gen-asm` subcommands instantiate `Scanner` directly from the raw source string and do **not** consume the output of `Checker`. Both phases re-parse independently. Therefore the data-flow contract between Checker and LLVM IR is not exercised at runtime; it is only implied by the assumption that the input has already been validated. |
| 4 | **LLVM IR → RISC-V Gen** | `[EXEC-VERIFIED]` | `generate_asm` internally calls `Scanner::scan_collect_asm` to produce LLVM IR, then passes the resulting `IrSession` to `parse_llvm_ir`, which walks the LLVM `Module` and emits RISC-V instructions. Both the intermediate IR and final assembly were captured and verified. |
| 5 | **ABI / Calling Convention** | `[PARTIAL]` | The minimal program defines `main()` with no parameters and no function calls. Consequently, the steel thread does not exercise parameter passing, caller/callee-saved register handling, or the `jal` / `ret` calling sequence. The `li a7, 93; ecall` exit sequence is observed, but this is the program-termination syscall rather than a function-call ABI. |
