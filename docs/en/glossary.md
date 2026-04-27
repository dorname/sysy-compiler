---
title: Glossary
description: Domain-specific terms for the SysY→RISC-V compiler project
---

# Glossary

This document defines domain-specific terms used across the project documentation. Terms are sorted alphabetically.

---

## ABI / Calling Convention

The set of rules governing how functions pass arguments, return values, and preserve registers across calls.

In this project, the minimal steel thread does not exercise parameter passing or caller/callee-saved register handling because `main()` has no parameters and makes no function calls. The `li a7, 93; ecall` sequence is used for program termination.

**Used in:** `steel_thread_trace.md`, `functional_design.md`

---

## AllocatedInnerVar

A dispatcher type in the register allocator that chooses between pure stack allocation and linear-scan register allocation based on a `only_stack` flag.

In this project, it is defined in the RISC-V backend register-allocation module and routes intervals to `NoAlloc` or `LinearScan`.

**Used in:** `functional_design.md`

---

## AsmBuilder

A string-buffer wrapper that emits formatted RISC-V assembly text, including directives, ALU instructions, memory operations, branches, and function prologue/epilogue.

In this project, it provides `emit_*` methods for every pseudo-instruction and directive used by the backend, and its accumulated buffer is written to the final `.s` file.

**Used in:** `functional_design.md`, `implementation.md`, `steel_thread_trace.md`

---

## AST (Abstract Syntax Tree)

A tree representation of the syntactic structure of source code.

In this project, the formatter does **not** build an explicit AST data structure; it operates directly on `pest` parse-tree pairs. The semantic checker and IR generator similarly traverse parse trees rather than a dedicated AST.

**Used in:** `functional_design.md`, `steel_thread_trace.md`

---

## Basic Block

A maximal sequence of consecutive instructions with a single entry point and a single exit point (no branches except at the end).

In this project, basic blocks are created during LLVM IR generation (e.g., `if_true`, `if_false`, `whileCond`) and are used by the RISC-V backend to partition instructions for liveness analysis.

**Used in:** `functional_design.md`, `implementation.md`

---

## clap

A Rust derive-based command-line argument parser.

In this project, `clap` drives the `sysy-compiler` CLI subcommands (`tokenize`, `fmt`, `check`, `gen-ir`, `gen-asm`) and the backward-compatible positional-args mode.

**Used in:** `functional_design.md`, `implementation.md`, `requirements.md`

---

## Control Flow

The order in which individual statements and instructions are executed, determined by conditionals, loops, and jumps.

In this project, the supported SysY subset includes `if`/`else`, `while`, `break`, `continue`, and `return`. The backend emits branch instructions and labels such as `whileCond` and `whileBody`.

**Used in:** `requirements.md`, `functional_design.md`

---

## Error Recovery

The parser's ability to resume parsing after encountering a syntax error so that multiple errors can be reported in one pass.

In this project, on a syntax error the parser skips to the next `;` or `}` and continues parsing, allowing the checker to emit several diagnostics at once.

**Used in:** `requirements.md`

---

## ErrorKind

An enumeration of 11 numbered semantic error categories (e.g., `UndefinedVal`, `TypeMismatch`, `ReturnMismatch`) plus a catch-all `Other`.

In this project, the semantic checker produces `ErrorKind` variants formatted as `Error type <K> at Line <N>: <message>`.

**Used in:** `functional_design.md`, `implementation.md`

---

## Evidence Class

A classification tag appended to design claims indicating how the claim was verified.

In this project, the three classes are:
- `[EXEC-VERIFIED]` — confirmed by source inspection, unit tests, or integration tests;
- `[STATIC-INFERRED]` — deduced from source structure without runtime verification;
- `[PARTIAL]` — partially implemented or tested.

**Used in:** `functional_design.md`, `steel_thread_trace.md`

---

## Formatter

The pretty-printer that reads raw SysY source and emits reformatted code with 4-space indentation, control-flow layout, and aligned array dimensions.

In this project, it is exposed via the `fmt` subcommand and operates directly on `pest` parse-tree pairs rather than an explicit AST.

**Used in:** `functional_design.md`, `implementation.md`, `requirements.md`

---

## FunctionState

A per-function accumulator used during the first analysis pass of the RISC-V backend to collect instructions, basic-block boundaries, allocated values, and loop information.

In this project, it feeds def/use data into liveness analysis and register allocation before the second emission pass begins.

**Used in:** `implementation.md`

---

## GenContext

The code-generation context that tracks variable locations (register, stack, or global), stack size, and whether to use pure-stack mode.

In this project, a `GenContext` is created per function during the second emission pass of the RISC-V backend.

**Used in:** `implementation.md`

---

## InnerVar

A structure representing a variable's live interval by instruction index (`start_offset` and `end_offset`).

In this project, `InnerVar` intervals are consumed by `LinearScan` to decide whether a variable gets a physical register or is spilled to the stack.

**Used in:** `functional_design.md`, `implementation.md`

---

## Inkwell

Safe Rust bindings over the LLVM-C API, used to construct SSA-form LLVM IR programmatically.

In this project, the `gen-ir` subcommand uses `inkwell` (configured with the `llvm14-0` feature) to create modules, basic blocks, and instructions.

**Used in:** `functional_design.md`, `implementation.md`, `requirements.md`

---

## Instruction Selection

The process of mapping LLVM opcodes (e.g., `Add`, `ICmp`, `Load`) to concrete target-machine instructions (e.g., RISC-V `add`, `slt`, `lw`).

In this project, it is performed in the RISC-V backend via pattern matching on `InstructionOpcode`.

**Used in:** `functional_design.md`, `steel_thread_trace.md`

---

## Interface Contract

A documented agreement between two compiler phases specifying the expected inputs, outputs, and invariants.

In this project, the steel-thread trace validates five contracts (e.g., Parser → AST, Checker → LLVM IR, LLVM IR → RISC-V Gen) and marks each with an evidence class.

**Used in:** `steel_thread_trace.md`

---

## IR (Intermediate Representation)

A machine-independent representation of a program used between the frontend and backend.

In this project, **LLVM IR** is the primary IR. The `gen-ir` subcommand emits `.ll` files, and the RISC-V backend first generates LLVM IR and then parses it to emit assembly.

**Used in:** `requirements.md`, `functional_design.md`, `implementation.md`, `steel_thread_trace.md`

---

## IrCore

A wrapper around the Inkwell `Context` that can spawn a new `IrSession`.

In this project, it encapsulates the long-lived LLVM context object; the `Scanner` holds an `IrCore` instance.

**Used in:** `implementation.md`

---

## IrSession

A compilation-unit bundle containing an Inkwell `Context`, `Module`, `Builder`, and `ScopeStack`.

In this project, `IrCore::start_session` creates an `IrSession`, which the `Scanner` uses to emit LLVM instructions for one source file.

**Used in:** `functional_design.md`, `implementation.md`, `steel_thread_trace.md`

---

## Known Debt

Explicitly documented shortcomings, TODOs, or architectural compromises in the codebase.

In this project, known debt includes oversized modules, tight coupling between the checker and IR generator due to the lack of a shared AST, missing `lib.rs`, and hard-coded types.

**Used in:** `implementation.md`

---

## Lexer

The component that converts raw source text into a stream of typed tokens.

In this project, the lexer is implemented as a **pest-driven PEG tokenizer** rather than a hand-written DFA. It is exposed via the `tokenize` subcommand and exercised by 38 lexer test fixtures.

**Used in:** `requirements.md`, `functional_design.md`, `implementation.md`, `steel_thread_trace.md`

---

## Linear Scan

A greedy register-allocation algorithm that sorts variables by live-interval start, assigns free registers in priority order, and spills variables when the register pool is exhausted.

In this project, it is the primary allocator, using temporaries `t3–t6`, saved registers `s0–s11`, and argument registers `a2–a7`. It implements a **spill-farthest** heuristic.

**Used in:** `requirements.md`, `functional_design.md`, `implementation.md`

---

## Liveness

An analysis that determines which variables are "live" (may be used before being redefined) at each program point.

In this project, liveness is computed per function to produce live intervals consumed by `LinearScan`. Loop-carried `alloca` variables have their intervals extended to the end of the function.

**Used in:** `functional_design.md`, `implementation.md`

---

## LLVM

The Low Level Virtual Machine compiler infrastructure.

In this project, the compiler targets LLVM 14 and uses LLVM IR as the intermediate representation between SysY and RISC-V. The build environment requires `llvm-14-dev` and `libpolly-14-dev`.

**Used in:** `requirements.md`, `functional_design.md`, `implementation.md`, `steel_thread_trace.md`

---

## Location

An enum describing where a value resides in the generated RISC-V program: a physical register, a stack slot, or a global label.

In this project, it is produced by the register allocator and consumed during instruction emission to emit correct load/store or move instructions.

**Used in:** `implementation.md`

---

## Parser

The component that recognizes syntactic structure.

In this project, all parsing stages reuse **pest** PEG grammars rather than hand-written recursive-descent parsers. Separate grammars exist for lexing, formatting, semantic checking, and IR generation.

**Used in:** `requirements.md`, `functional_design.md`, `implementation.md`, `steel_thread_trace.md`

---

## pest

A Rust parser generator based on Parsing Expression Grammars (PEG).

In this project, `pest` (with `pest_derive`) generates `Parser` implementations at compile time from `.pest` grammar files, avoiding hand-written lexers and parsers for each compiler stage.

**Used in:** `requirements.md`, `functional_design.md`, `implementation.md`

---

## RARS

RISC-V Assembler and Runtime Simulator, a Java-based simulator used to validate generated RISC-V assembly.

In this project, the integration test suite runs RARS and checks register values, termination within a 5-second timeout, and control-flow branch targets.

**Used in:** `requirements.md`, `functional_design.md`, `implementation.md`, `steel_thread_trace.md`

---

## Register Allocation

The process of mapping an unbounded set of virtual variables (or SSA values) to a finite set of physical CPU registers, spilling excess variables to the stack when necessary.

In this project, the RISC-V backend supports `LinearScan` with spill-to-stack and a fallback `NoAlloc` pure-stack mode.

**Used in:** `requirements.md`, `functional_design.md`, `implementation.md`

---

## RISC-V

An open-standard instruction-set architecture (ISA) and the target assembly language of this compiler.

In this project, the `gen-asm` subcommand emits `.s` files containing RISC-V instructions and pseudo-instructions. The allocator uses temporaries `t0–t6` and saved registers `s0–s11`, preserving `a0–a1` for return values.

**Used in:** `requirements.md`, `functional_design.md`, `implementation.md`, `steel_thread_trace.md`

---

## ScopeKey

An enum identifying a scope frame: global scope, function scope, or inner block scope.

In this project, it is used as the key in `ScopeStack`'s hash map of symbol tables. In IR generation, the function and block variants carry Inkwell handles (`FunctionValue`, `BasicBlock`).

**Used in:** `functional_design.md`, `implementation.md`

---

## ScopeStack

A nested lexical scoping abstraction that maintains a stack of active scope keys and a map from keys to symbol tables.

In this project, an identical `ScopeStack` design is used by both the semantic checker and the IR generator to resolve identifiers across nested blocks and function boundaries.

**Used in:** `functional_design.md`, `implementation.md`

---

## Semantic Analysis

The compilation phase that validates type correctness, scope rules, and language constraints after parsing.

In this project, the semantic checker implements 11 error kinds (Error type 1–11 plus A) and is exposed via the `check` subcommand.

**Synonym:** semantic checker

**Used in:** `requirements.md`, `functional_design.md`, `implementation.md`, `steel_thread_trace.md`

---

## Spill

The act of storing a variable from a register to a stack slot when the register pool is exhausted.

In this project, `LinearScan::overflow_to_stack` uses a **spill-farthest** heuristic: if the active variable with the latest end-offset ends after the current variable, that active variable is spilled instead, and its register is reassigned.

**Used in:** `functional_design.md`, `implementation.md`

---

## SSA (Static Single Assignment)

An IR property where each variable is assigned exactly once.

In this project, LLVM IR is in SSA form, and the RISC-V backend refers to LLVM instruction result names as "SSA names" when mapping them to registers or stack slots.

**Used in:** `implementation.md`

---

## Steel-Thread Trace

An end-to-end execution trace of a minimal program through every compiler phase, used to validate interface contracts and demonstrate that the pipeline is wired together correctly.

In this project, the trace covers `tokenize`, `fmt`, `check`, `gen-ir`, and `gen-asm` for a `main()` function returning `1 + 2`.

**Used in:** `steel_thread_trace.md`

---

## Symbol Table

A data structure that maps identifiers to their semantic types and locations within the current scope.

In this project, symbol tables are split per-scope inside `ScopeStack`; there is no single global `func_table` or `var_table`. Functions, variables, and arrays are all stored as `Type` variants in the current scope.

**Used in:** `functional_design.md`, `implementation.md`

---

## SysY

A C-like teaching language used in compiler-design courses.

In this project, the supported subset includes `int` and `void` types, scalar and array variables, functions, and basic control-flow constructs (`if`, `while`, `break`, `continue`, `return`). Floating-point types, pointers (except array-parameter decay), `struct`, `union`, and standard I/O are out of scope.

**Used in:** `requirements.md`, `functional_design.md`, `implementation.md`, `steel_thread_trace.md`

---

## Token

The smallest meaningful unit of source text produced by the lexer, such as identifiers, operators, keywords, and integer literals.

In this project, the `Token` enum defines six variants: `Identifier`, `Operator`, `Type`, `Flow`, `IntegerConst`, and `ErrorSyntax`.

**Used in:** `requirements.md`, `functional_design.md`, `implementation.md`, `steel_thread_trace.md`

---

## Traceability Matrix

A table that maps each compilation phase to its input, transformation, implementing function or type, and output.

In this project, the steel-thread trace contains a 14-step traceability matrix covering the lexer, parser, semantic checker, LLVM IR generation, and RISC-V codegen.

**Used in:** `steel_thread_trace.md`

---

## Two-Pass Backend

A code-generation strategy where the first pass analyzes instructions to compute liveness and allocate registers, and the second pass emits assembly text.

In this project, the RISC-V backend uses this approach per function: `FunctionState` collects def/use information in pass 1, and `AsmBuilder` emits instructions in pass 2.

**Used in:** `functional_design.md`, `implementation.md`
