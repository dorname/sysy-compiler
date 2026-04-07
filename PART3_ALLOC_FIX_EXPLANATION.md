# Part3 寄存器分配修复说明

## 1. 问题现象

在 `tests/lab6/part3.sy` 上，修复前出现了明显的寄存器分配退化：

- 栈帧从目标基线的 `16` / `0` 级别退化到 `320`
- `lw/sw` 数量从目标基线 `63` 附近退化到 `214`
- `part3_verify` 中的两个检查都失败：
  - `part3_stack_frame_stays_within_target_budget`
  - `part3_load_store_count_stays_close_to_target_baseline`

这说明变量没有被稳定地保留在寄存器里，而是发生了大量不必要的 spill/reload。

---

## 2. 实际修改点

### 修改点 A：`src/riscv_codegen/mod.rs`

位置：`build_function()` 中，计算活跃区间后、进入分配器前。

### 修复前

```rust
let inner_vars = state.compute_liveness();
```

### 修复后

```rust
let inner_vars = state
    .compute_liveness()
    .into_iter()
    .filter(|var| !global_names.contains(var.get_name()))
    .collect::<Vec<_>>();
```

### 这个修改修了什么

**Bug：全局变量被错误地送进了寄存器分配器。**

本项目里，全局变量本来已经固定映射成 `Location::Global(...)`，访问方式应该是：

- `la reg, global`
- `lw/sw ..., 0(reg)`

它们**不应该再参与线性扫描寄存器分配**。

但原逻辑里，`compute_liveness()` 产出的活跃区间没有过滤全局变量，于是像下面这种 IR：

```llvm
%x = load i32, i32* @x, align 4
```

会让名字为 `x` 的实体进入活跃区间集合；而函数作用域里又已经提前注册了全局 `x -> Location::Global("x")`。这会导致：

- 全局相关名字被当成普通局部值参与分配
- 分配器错误估计活跃变量数量
- 过早耗尽寄存器
- 后续大量 spill 到栈

### Bug -> 现象 -> 修改 对应

- **Bug**：全局变量也参与活跃区间/寄存器分配
- **现象**：寄存器被无意义占满，后续中间值大量落栈，栈帧和 `lw/sw` 暴涨
- **修改**：在进入分配器前，用 `filter(|var| !global_names.contains(var.get_name()))` 去掉全局变量

---

### 修改点 B：`src/riscv_codegen/register_alloc.rs`

位置：`InnerVar` 的 `Ord` 实现。

### 修复前

```rust
impl Ord for InnerVar {
    fn cmp(&self, other: &Self) -> Ordering {
        self.end_offset.cmp(&other.end_offset)
    }
}
```

### 修复后

```rust
impl Ord for InnerVar {
    fn cmp(&self, other: &Self) -> Ordering {
        self.end_offset
            .cmp(&other.end_offset)
            .then_with(|| self.start_offset.cmp(&other.start_offset))
            .then_with(|| self.name.cmp(&other.name))
    }
}
```

### 这个修改修了什么

**Bug：`BTreeSet<InnerVar>` 中，多个 `end_offset` 相同的变量会被错误视为同一个键。**

线性扫描实现里，`active_vars` 使用的是：

```rust
BTreeSet<InnerVar>
```

`BTreeSet` 要求元素排序关系能唯一标识元素。原先只比较 `end_offset`，这意味着：

- 如果两个变量 `end_offset` 一样
- 即使它们名字不同、起点不同
- `cmp()` 也会返回 `Equal`
- `BTreeSet` 就会把它们当成同一个元素

后果是：

- active 集合丢元素
- 某些寄存器释放逻辑失真
- 某些变量没有正确从 active 中退出/复用
- 最终出现额外 spill 或复用异常

### Bug -> 现象 -> 修改 对应

- **Bug**：live interval 的排序键不唯一，`BTreeSet` 去重错误
- **现象**：active 集合状态不准，寄存器回收/复用异常，导致额外 spill
- **修改**：把排序键补全为 `end_offset -> start_offset -> name`

这样每个 live interval 都能稳定地作为独立元素存在于 `BTreeSet` 中。

---

### 修改点 C：`src/riscv_codegen/register_alloc.rs`

位置：测试 `test_linear_scan1`

### 实际改动

更新了该测试的期望寄存器分配结果。

### 为什么这算必要修改

不是单纯格式化，而是因为：

- 修复 `Ord` 后，active 集合的真实行为变了
- 某些变量的寄存器复用结果变成了新的正确值
- 原断言已经不再符合修复后的分配行为

### Bug -> 现象 -> 修改 对应

- **Bug**：测试建立在旧的错误排序行为之上
- **现象**：修复真实 bug 后，旧测试断言会失败
- **修改**：同步更新断言为修复后的正确分配结果

---

### 修改点 D：`src/riscv_codegen/register_alloc.rs`

位置：新增测试 `test_linear_scan_releases_intervals_with_same_end`

### 新增测试的目的

这个测试专门覆盖：

- 多个变量拥有相同 `end_offset`
- 之后又出现新变量，应该正确复用释放出来的寄存器

它验证的是：

- 不会因为 `BTreeSet` 错误去重导致 active 集合丢状态
- 不会错误 spill 到栈
- 寄存器能正常回收并复用

### Bug -> 现象 -> 修改 对应

- **Bug**：相同结束点区间可能导致集合判重错误
- **现象**：回归时再次出现寄存器泄漏/错误 spill
- **修改**：新增专门回归测试锁住这个行为

---

## 3. 哪些改动不算语义修改

这次提交里还包含了一些 `cargo fmt` 触发的重排，例如：

- `use` 顺序变化
- 缩进/换行变化
- `if let` 的换行重排
- 参数列表换行

这些都**不是实际修复点**，可以忽略。

真正有语义影响的，只有上面 4 项。

---

## 4. 修复结果

修复后，Part3 相关验证恢复正常：

- `cargo test --test part3_verify -- --nocapture` 通过
- `cargo test riscv_codegen::register_alloc::tests -- --nocapture` 通过
- `cargo test riscv_codegen::tests -- --nocapture` 通过
- 端到端编译 `tests/lab6/part3.sy` 得到：
  - `stack_frame = 0`
  - `load_store_count = 60`

说明这次修复确实消除了导致 Part3 退化的两个核心问题：

1. 全局变量误入分配器
2. 相同结束点区间导致的 active 集合判重错误
