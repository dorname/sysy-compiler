# Linear Scan Register Allocation Algorithm Implementation Guide

## Project Background

This project is a compiler that translates SysY language into RISC-V assembly code. The project adopts a two-pass architecture:
- First pass: Collect live interval information for variables
- Second pass: Generate assembly code based on register allocation results

In the current project structure, the `LinearScan` struct needs to implement the linear scan register allocation algorithm to optimize variable-to-register assignment and reduce the number of memory accesses.

## Algorithm Overview

Linear scan register allocation is an efficient register allocation algorithm with a time complexity of O(n log n). The core idea of the algorithm is:
1. Sort all variables by the starting position of their live intervals
2. Scan from left to right, allocating registers for each variable
3. When registers are insufficient, select the variable with the latest end time to spill to the stack

## RISC-V32 Register Conventions

In the RISC-V32 architecture, registers available for general-purpose computation include:

### Temporary Registers
- `t0-t6` (x5-x7, x28-x31): 7 temporary registers, caller-saved
- Used for storing temporary computation results; no need to preserve across function calls

### Saved Registers
- `s0-s11` (x8-x9, x18-x27): 12 saved registers, callee-saved
- Used for storing values that need to be preserved across function calls

### Argument Registers
- `a0-a7` (x10-x17): 8 argument registers
- Among them, `a0-a1` are also used as return value registers

**Recommended allocation priority**: t0-t6 > s0-s11 > a2-a7 (reserve a0-a1 for return values)

## Implementation Steps

### 1. Initialize Register Pool

Initialize the list of available registers in the `LinearScan::new()` method:

**Implementation approach**:
- Create a `Vec<String>` containing all available register names
- Add registers in priority order: temporary registers first, then saved registers
- Initialize other fields to default values

**Notes**:
- Reserve `a0` and `a1` for function return values
- Reserve special registers such as `sp` (stack pointer) and `ra` (return address)
- Register names are in string form, such as "t0", "s0", etc.

### 2. Implement Live Interval Sorting

**Implementation approach**:
- Sort the input `Vec<InnerVar>` in ascending order by `start_offset`
- Use Rust's `sort_by_key` method: `vars.sort_by_key(|v| v.get_start_offset())`

**Purpose**:
- Ensure variables are processed in the order of their live interval start times
- This is the core prerequisite of the linear scan algorithm

### 3. Implement Active Interval Management

**Data structure design**:
- Use `Vec<(String, usize)>` to store currently active variables, with elements as (variable name, end time)
- Keep this vector sorted in ascending order by end time

**Implementation approach**:
- Before processing each new variable, clean up active intervals that have already ended
- Cleanup method: iterate through the active list, removing variables whose end time is less than or equal to the current variable's start time
- When cleaning up, also release the corresponding register back to the register pool

### 4. Implement Register Allocation Logic

**Core algorithm flow**:

```
for each variable in the sorted variable list:
    1. Clean up ended active intervals
    2. if there is a free register:
         Allocate the register to the current variable
         Add the variable to the active list
         Update the register mapping table
    3. else:
         Find the active variable with the latest end time
         if that variable's end time > current variable's end time:
             Spill that variable to the stack
             Allocate its register to the current variable
             Update the active list and mapping table
         else:
             Directly allocate the current variable to the stack
```

**Implementation details**:
- Register allocation: `pop()` a register from the `regs` vector
- Stack allocation: use `stack_offset` to calculate stack position, each variable occupies 4 bytes
- Update mapping table: `reg_to_var` records the mapping from register to variable

### 5. Implement Spill Handling

**Spill strategy**:
- When all registers are occupied, select the variable with the latest end time for spilling
- Compare the end time of the current variable with the latest end variable, and choose the better allocation scheme

**Implementation approach**:
- Find the variable with the maximum end time in the active list (since the list is sorted, take the last element)
- If that variable's end time is greater than the current variable's, perform a spill swap
- When spilling, you need to:
  - Remove the spilled variable from the register mapping
  - Allocate stack space for the spilled variable
  - Allocate its register to the current variable
  - Update all related data structures

### 6. Implement Return Value Formatting

**Return value requirements**:
- `HashMap<String, String>`: mapping from variable name to location
- `i32`: total stack space size

**Location format**:
- Register location: directly use the register name, such as "t0", "s1"
- Stack location: use RISC-V memory access format, such as "0(sp)", "4(sp)"

**Implementation approach**:
- Iterate through all variables and build the mapping table according to allocation results
- Stack location format: `format!("{}(sp)", offset)`
- Return the total stack space size for function prologue generation

### 7. Handle Edge Cases

**Edge cases to consider**:

1. **Empty variable list**: directly return an empty mapping and 0 stack size
2. **Single variable**: allocate the first available register
3. **Variables exceed register count**: correctly handle spill logic
4. **Same live interval**: sort by variable name or other stable rules
5. **Zero-length live interval**: case where start_offset == end_offset

### 8. Optimization Considerations

**Performance optimization**:
- Use `BinaryHeap` or `BTreeSet` to optimize active interval management
- Precompute register pool size to avoid repeated calculations

**Code quality**:
- Add detailed comments explaining algorithm steps
- Use meaningful variable names
- Appropriate error handling and assertions

**Memory optimization**:
- Clean up data structures that are no longer needed in a timely manner
- Avoid unnecessary string clones

## Testing and Verification

**Test case design**:
1. Simple case: number of variables is less than the number of registers
2. Spill case: number of variables is greater than the number of registers
3. Complex live intervals: overlapping, nested, and adjacent live intervals
4. Edge cases: empty list, single variable, same interval

**Verification methods**:
- Check the correctness of allocation results: each variable has a unique location
- Verify register conflicts: two variables cannot use the same register at the same point in time
- Check stack space calculation: ensure the stack size is sufficient to accommodate all spilled variables

## Integration with the Project

**Integration points**:
- The register allocator is called at lines 324-328 in the `build_function` function
- Allocation results are recorded via `GenContext::record_alloca_vars`
- Variable locations are retrieved via `GenContext::get_location` during the second pass

**Output format**:
- Register allocation results are used to generate RISC-V assembly instructions
- Stack space size is used to generate function prologue and epilogue
- Location information is used for address calculation in load/store instructions

## Summary

The linear scan register allocation algorithm achieves good register utilization through a simple and efficient linear scanning process while ensuring correctness. This algorithm is particularly suitable for fast compilation scenarios in compilers, striking a good balance between code quality and compilation speed.

During implementation, special attention should be paid to RISC-V architecture register conventions, correct handling of spill cases, and ensuring good integration with the existing code generation framework.
