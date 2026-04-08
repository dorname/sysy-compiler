# part4 修复说明

## 1. 问题背景

`tests/lab6/part4.sy` 对应的是一个用减法实现的欧几里得算法。目标行为是：编译器生成的 RISC-V 汇编能够在 RARS 中正常结束，并返回与 `tests/lab6/target_part4.s` 一致的结果。

在修复前，`part4` 生成结果会长时间运行而无法结束，而目标基线 `tests/lab6/target_part4.s` 可以正常退出并返回 `a0 = 14`。

## 2. 错误现象

修复前的具体现象有两层：

1. 运行层面，`timeout 5s java -jar tests/lab6/rars.jar tests/lab6/part4.s a0` 无法在超时前退出。
2. 控制流层面，生成汇编中的条件跳转方向与 LLVM IR 不一致：
   - `whileCond` 的真分支本应跳到 `whileBody`，实际却跳到了 `whileNext`
   - `whileBody` 的真分支本应跳到 `if_true`，实际却跳到了 `if_false`

这两个现象叠加后，导致循环条件和分支逻辑被翻转，程序无法按预期收敛到退出路径。

## 3. 错误原因分析

这次问题的根因不是一个点，而是两个独立错误叠加造成的。

### 3.1 条件分支目标映射错误

在 `src/riscv_codegen/mod.rs` 的 `generate_br_instruction` 中，条件分支的 block operand 顺序处理错了。

修复前的实现默认把：

- `instruction.get_operand(1)` 当成 true 分支
- `instruction.get_operand(2)` 当成 false 分支

但结合 `tests/lab6/part4.ll` 和实际生成结果可以确认，这里的顺序应当按 `false`、`true` 解释。这个错误会直接把 `bne` 和后续 `j` 的落点对调，导致：

- 本来应该“条件成立就继续循环体”，变成了“条件成立就提前退出”
- 本来应该“`a > b` 时更新 `a`”，变成了“`a > b` 时更新 `b`”

### 3.2 `icmp eq/ne` 结果归一化使用了错误寄存器

在 `src/riscv_codegen/mod.rs` 附近的 `generate_icmp_instruction` 中，`eq` 和 `ne` 的 lowering 先把 `sub` 的结果写入 `result_reg`，随后却固定从 `t2` 读取，再执行 `seqz/snez`。

这意味着当结果寄存器不是 `t2` 时，后续布尔归一化实际上读取的是错误来源，比较结果就可能被污染。虽然这个问题在所有样例里不一定都会立即暴露，但在 `part4` 这种“比较结果再参与条件分支”的链路里，会放大为实际控制流错误。

## 4. 修复思路

修复思路遵循两个原则：

1. 只修正当前 lowering 逻辑，不重写寄存器分配或整体代码生成流程。
2. 不追求生成汇编和目标文件逐行一致，只追求运行语义一致。

基于这个原则，修复被拆成两个最小闭环：

### 4.1 先修正分支语义

先把 `generate_br_instruction` 中的 true/false 目标映射纠正，使生成汇编的跳转方向与 `tests/lab6/part4.ll` 一致。

### 4.2 再修正布尔归一化

把 `generate_icmp_instruction` 中 `eq/ne` 的 `seqz/snez` 改成基于 `result_reg` 自身继续归一化，保证“减法结果”和“布尔结果”处在同一条数据流上，不再依赖错误的固定寄存器。

### 4.3 用回归测试锁定行为

除了原有的“能生成汇编”的测试，这次还补了针对 `part4` 的运行语义回归测试，确保：

- 生成程序不会超时
- 生成程序能正常退出
- 返回值与 `target_part4.s` 一致
- 关键真分支确实跳向正确标签

## 5. 实际实现

### 5.1 修改 `generate_br_instruction`

在 `src/riscv_codegen/mod.rs` 中，调整了条件分支操作数的解释顺序：

- 将 `operand(1)` 解释为 `false_operand`
- 将 `operand(2)` 解释为 `true_operand`

同时增加了一条注释，明确这里的 block operand 顺序是 `false`、`true`，避免后续再次改反。

### 5.2 修改 `generate_icmp_instruction`

在 `src/riscv_codegen/mod.rs` 中，调整了 `eq/ne` 的归一化逻辑：

- `seqz result_reg, result_reg`
- `snez result_reg, result_reg`

也就是让 `seqz/snez` 直接基于刚刚写入的 `result_reg` 继续计算，而不是固定读取 `t2`。

### 5.3 新增 `part4` 回归测试

新增了 `tests/part4_verify.rs`，主要包含三类检查：

1. `part4_generated_program_terminates_with_target_result`
   - 编译 `part4.sy`
   - 在 RARS 中执行生成的汇编
   - 验证正常退出，且返回值与目标基线一致

2. `part4_generated_program_does_not_timeout`
   - 验证生成程序不会在 5 秒超时窗口内卡住

3. `part4_truthy_branches_target_loop_body_and_if_true`
   - 直接检查生成汇编中关键真分支是否跳到了正确标签

## 6. 验证结果

修复完成后，已确认以下结果：

- `cargo build` 通过
- `cargo test --test part4_verify -- --nocapture` 通过，`3 passed`
- `cargo test test_part4 -- --nocapture` 通过
- `cargo test test_ir -- --nocapture` 通过
- `cargo run -- tests/lab6/part4.sy /tmp/ralph-part4.s && timeout 5s java -jar tests/lab6/rars.jar /tmp/ralph-part4.s a0` 正常退出，返回 `a0 = 0x0000000e`

这说明本次修复已经满足目标：生成汇编的执行结果与 `target_part4.s` 一致，虽然汇编文本本身不要求完全一致。

补充说明：仓库全量 `cargo test` 仍存在一批与 `gen_llvm_ir` fixture 相关的既有失败，这些失败与本次 `part4` 修复无关，因此本次验证聚焦在 `part4` 相关构建、测试和运行结果上。

## 7. 结论

`part4` 的问题本质上是“布尔值 lowering 错误”和“条件分支目标映射错误”共同导致的控制流语义偏差。修复没有扩大到整个代码生成器，而是通过两处定点修改和一组针对性回归测试，把问题稳定收敛到了正确行为。

本次修复的关键价值不只是让 `part4` 通过，还补上了原先缺失的运行语义回归测试，使后续再改 `icmp` 或 `br` lowering 时，能够更早暴露类似回归。
