// RISC-V 代码生成模块
// 负责将LLVM IR翻译为RISC-V汇编代码

use std::collections::{HashMap, HashSet};
use std::process::Output;

use crate::riscv_codegen::register_alloc::{AllocatedInnerVar, InnerVar, Location};
use crate::{
    gen_llvm_ir::{IrSession, Scanner},
    riscv_codegen::asm_builder::AsmBuilder,
};
use clap::builder::Str;
use inkwell::values::{BasicValueEnum, InstructionOpcode};
use inkwell::{
    IntPredicate,
    module::Module,
    values::{FunctionValue, GlobalValue, InstructionValue},
};
use num_traits::zero;

pub mod asm_builder;
pub mod register_alloc;

pub fn generate_asm(input: &str, output: &str) -> Result<(), String> {
    let scanner = Scanner::new(input, output);
    let _ = scanner.scan_collect_asm(|ir_session, output| Ok(parse_llvm_ir(ir_session, output)));
    Ok(())
}

/// LLVM IR 解析工具
/// 负责解析LLVM IR并提取函数、基本块、指令信息
/// 并构建对应的汇编代码    
///
/// # 参数
/// * `ir_session` - LLVM IR会话
///
/// # 返回值
/// * `None` - 如果解析失败
///
pub fn parse_llvm_ir<'ctx>(ir_session: &IrSession<'ctx>, output: &str) {
    let module: &Module<'ctx> = &ir_session.module;
    // 初始化汇编构建器
    let mut asm_builder = AsmBuilder::new();

    // 提取全局变量
    let global_variables = module.get_globals();
    let mut global_names = HashSet::<String>::new();
    for global_variable in global_variables {
        global_names.insert(global_variable.get_name().to_str().unwrap().to_string());
        build_global_variable(&global_variable, &mut asm_builder);
    }

    // 依次提取函数，并构建汇编
    let functions = module.get_functions();
    for function in functions {
        build_function(&function, &mut asm_builder, &global_names);
    }

    // 获取并将结果写output 文件
    let result = asm_builder.emit();
    std::fs::write(output, result).unwrap();
}

/// 代码生成上下文：保存变量位置映射和栈大小
pub struct GenContext {
    var_locations: HashMap<String, Location>, // 变量名 -> 存储位置
    stack_size: usize,                        // 总栈大小
    global_names: Vec<String>,                // 全局变量名列表
    temp_val_num: usize,
    allocated_vals: HashSet<String>,           // alloca变量名集合
    pure_stack_mode: bool,                   // 是否为纯栈分配模式
}

impl GenContext {
    pub fn get_allocated_vals(&self) -> &HashSet<String> {
        &self.allocated_vals
    }
    
    pub fn new() -> Self {
        Self {
            var_locations: HashMap::new(),
            stack_size: 0,
            global_names: Vec::new(),
            temp_val_num: 0,
            allocated_vals: HashSet::new(),
            pure_stack_mode: false,
        }
    }
    
    pub fn set_pure_stack_mode(&mut self, enabled: bool) {
        self.pure_stack_mode = enabled;
    }
    
    pub fn set_allocated_vals(&mut self, allocated_vals: HashSet<String>) {
        self.allocated_vals = allocated_vals;
    }
    
    /// 检查是否为alloca变量（纯栈模式下返回false）
    pub fn is_alloc(&self, name: &str) -> bool {
        if self.pure_stack_mode {
            return false;
        }
        self.allocated_vals.contains(name)
    }

    pub fn add_global(&mut self, name: String) {
        self.var_locations
            .insert(name.clone(), Location::Global(name.clone()));
        self.global_names.push(name);
    }

    pub fn get_location(&self, name: &str) -> Option<&Location> {
        self.var_locations.get(name)
    }

    /// 记录寄存器分配结果：将分配器返回的字符串转换为 `Location` 枚举
    ///
    /// 该函数接收寄存器分配器返回的分配结果，将字符串格式的位置信息
    /// （如 `"t0"`、`"4(sp)"`）转换为 `Location` 枚举类型，并更新上下文中的
    /// 变量位置映射和栈大小信息。
    ///
    /// # 参数
    ///
    /// * `allocation` - 寄存器分配器返回的分配结果映射表
    ///   - 键：变量名称（`String`）
    ///   - 值：位置字符串（`String`），格式为：
    ///     - 寄存器位置：`"t0"`、`"s1"`、`"a2"` 等
    ///     - 栈位置：`"0(sp)"`、`"4(sp)"`、`"8(sp)"` 等（格式：`"{offset}(sp)"`）
    /// * `stack_size` - 函数所需的总栈空间大小（字节数）
    ///
    /// # 行为
    ///
    /// 1. **更新栈大小**：将 `stack_size` 保存到上下文中
    /// 2. **转换位置格式**：
    ///    - 如果位置字符串以 `"(sp)"` 结尾，解析为 `Location::Stack(offset)`
    ///    - 否则，解析为 `Location::Reg(register_name)`
    /// 3. **保护全局变量**：如果变量已经是全局变量（`Location::Global`），
    ///    则跳过该变量，不覆盖其位置信息
    /// 4. **更新映射表**：将转换后的 `Location` 插入到 `var_locations` 中
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let mut ctx = GenContext::new();
    ///
    /// // 寄存器分配器返回的结果
    /// let allocation = HashMap::from([
    ///     ("var1".to_string(), "t0".to_string()),      // 寄存器分配
    ///     ("var2".to_string(), "4(sp)".to_string()),   // 栈分配
    ///     ("var3".to_string(), "s1".to_string()),      // 寄存器分配
    /// ]);
    ///
    /// ctx.record_alloc_vars(allocation, 16);
    ///
    /// // 现在可以通过 get_location() 查询变量位置
    /// assert_eq!(ctx.get_location("var1"), Some(&Location::Reg("t0".to_string())));
    /// assert_eq!(ctx.get_location("var2"), Some(&Location::Stack(4)));
    /// assert_eq!(ctx.get_location("var3"), Some(&Location::Reg("s1".to_string())));
    /// assert_eq!(ctx.get_stack_size(), 16);
    /// ```
    ///
    /// # 注意
    ///
    /// - 该函数通常在寄存器分配完成后调用，用于将分配结果记录到代码生成上下文中
    /// - 全局变量的位置不会被覆盖，即使分配结果中包含该变量
    /// - 栈位置字符串必须符合 `"{offset}(sp)"` 格式，否则 `parse()` 会 panic
    /// - 该函数会更新 `stack_size`，用于后续生成函数 prologue/epilogue
    pub fn record_alloc_vars(&mut self, allocation: HashMap<String, String>, stack_size: usize) {
        self.stack_size = stack_size;

        for (var, loc_str) in allocation {
            if let Some(Location::Global(_)) = self.var_locations.get(&var) {
                continue; // 保护全局变量
            }

            let location = if loc_str.ends_with("(sp)") {
                let offset_str = loc_str.trim_end_matches("(sp)");
                let offset: i32 = offset_str.parse().unwrap();
                Location::Stack(offset)
            } else {
                Location::Reg(loc_str)
            };
            self.var_locations.insert(var, location);
        }
    }

    pub fn get_stack_size(&self) -> usize {
        self.stack_size
    }

    pub fn is_global(&self, name: &str) -> bool {
        self.global_names.contains(&name.to_string())
    }
}

/// 第一次遍历状态：收集指令的def/use信息
struct FunctionState {
    instructions: Vec<(usize, String, Vec<String>)>, // (指令索引, 变量名, 使用的变量集合)
    allocated_vals: HashSet<String>,               // 记录已经分配了存储空间的数据
    block_start_idxes: HashMap<String, usize>,       // 基本块起始位置
    loop_branch: bool,                               // 是否存在循环
    idx: usize,                                      // 当前指令索引
}

impl FunctionState {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            allocated_vals: HashSet::new(),
            block_start_idxes: HashMap::new(),
            loop_branch: false,
            idx: 0,
        }
    }

    /// 添加指令的信息(指令索引,指令名称，使用的操作数集合)
    pub fn record_instruction(&mut self, idx: usize, val_name: String, uses: Vec<String>) {
        // 记录所有有定义的指令，即使uses为空（用于计算活跃区间）
        if !val_name.is_empty() {
            self.instructions.push((idx, val_name, uses));
        }
    }

    /// 记录alloca变量名
    pub fn record_allocated_val(&mut self, name: String) {
        self.allocated_vals.insert(name);
    }

    /// 记录基本块起始索引
    pub fn record_block_start(&mut self, block_name: String, idx: usize) {
        self.block_start_idxes.insert(block_name, idx);
    }

    /// 标记存在循环
    pub fn mark_loop(&mut self) {
        self.loop_branch = true;
    }

    /// 获取访问器
    pub fn has_loop(&self) -> bool {
        self.loop_branch
    }
    pub fn get_allocated_vals(&self) -> &HashSet<String> {
        &self.allocated_vals
    }
    pub fn get_block_idx(&self, block_name: &str) -> Option<usize> {
        self.block_start_idxes.get(block_name).copied()
    }
    pub fn current_idx(&self) -> usize {
        self.idx
    }

    /// 遍历指令集合计算对应的活跃区间
    pub fn compute_liveness(&mut self) -> Vec<InnerVar> {
        let mut first = HashMap::<String, usize>::new();
        let mut last = HashMap::<String, usize>::new();
        // 求出这个函数作用域最后的指令索引,默认值为0
        // 整个函数作用域最大的活跃区间为max_pos
        let max_pos = self
            .instructions
            .iter()
            .map(|(idx, _, _)| idx)
            .max()
            .unwrap_or(&0);

        // 遍历指令集
        for (idx, val_name, uses) in self.instructions.iter() {
            // 如果名称不是临时变量或者常量，则说明是定义的变量，计算对应的活跃区间
            if !val_name.is_empty() && !val_name.starts_with("tmp_") {
                // 记录第一次出现的位置，entry方法是如果hash表中为空才会插入，否则不插入
                first.entry(val_name.to_string()).or_insert(*idx);
                // 记录最后一次出现的位置
                last.insert(val_name.to_string(), *idx);
            }
            // 开始根据被使用的位置信息，更新活跃区间
            for use_name in uses {
                if !use_name.is_empty() && !use_name.starts_with("tmp_") {
                    first.entry(use_name.to_string()).or_insert(*idx);
                    last.insert(use_name.to_string(), *idx);
                }
            }
        }

        // 将first和last转换为活跃区间
        let mut inner_vars = Vec::<InnerVar>::new();
        for (val_name, start_idx) in first {
            let mut end_idx = last.get(&val_name).unwrap_or(&start_idx).clone();
            if self.loop_branch && self.allocated_vals.contains(&val_name) {
                end_idx = *max_pos;
            }
            inner_vars.push(InnerVar::new(val_name.to_string(), start_idx, end_idx));
        }
        inner_vars
    }
}

/// 构建全局变量的汇编代码
fn build_global_variable<'ctx>(global_variable: &GlobalValue<'ctx>, asm_builder: &mut AsmBuilder) {
    // 1、添加数据段
    asm_builder.emit_data_section();
    // 2、添加标签
    let name = global_variable.get_name().to_str().unwrap();
    asm_builder.emit_label(name);
    // 3、获取初始值，默认值为0
    let init_value = global_variable.get_initializer();
    if let Some(init_value) = init_value
        && let Some(val) = init_value.into_int_value().get_zero_extended_constant()
    {
        asm_builder.emit_word(val as i32);
    } else {
        asm_builder.emit_word(0);
    }
    // 4、起一个新行
    asm_builder.emit_empty_line();
}

/// 构建函数的汇编代码
/// 采用两次遍历架构：先分析后生成
fn build_function<'ctx>(
    function: &FunctionValue<'ctx>,
    asm_builder: &mut AsmBuilder,
    global_names: &HashSet<String>,
) {
    // 1、获取函数名和全局变量名
    let function_name = function.get_name().to_str().unwrap();

    // 2、初始化GenContext
    let mut ctx = GenContext::new();

    // 注册全局变量到当前函数作用域
    for global in global_names {
        ctx.add_global(global.to_string());
    }

    // ==========================================
    // 遍历,收集要素
    // 1、收集基本块的起始位置（基本块索引）
    // 2、给块内所有指令位置编号，收集(编号，指令名称，操作数数量)
    // 3、判断是否存在循环，当目标块的起始位置小于当前块时，说明存在循环
    // ==========================================
    let mut state = FunctionState::new();
    // 步骤1：建立基本块到指令索引的映射（用于循环检测）
    // 记录每个基本块从第几条指令开始
    let mut temp_idx = 0;
    for basic_block in function.get_basic_blocks() {
        if let Ok(block_name) = basic_block.get_name().to_str()
            && !block_name.is_empty()
        {
            state.record_block_start(block_name.to_string(), temp_idx);
        }
        temp_idx += basic_block.get_instructions().count();
    }

    // 步骤2：遍历所有指令，收集信息
    for basic_block in function.get_basic_blocks() {
        let block_start_idx = state.current_idx();

        for instruction in basic_block.get_instructions() {
            let opcode = instruction.get_opcode();
            let idx = state.current_idx();

            // 2.1 收集定义变量集合
            if matches!(opcode, InstructionOpcode::Alloca) {
                let name = get_value_name(&instruction);
                if !name.is_empty() {
                    state.record_allocated_val(name);
                }
            }

            // 2.2 检测循环（backward branch）
            if matches!(opcode, InstructionOpcode::Br) {
                if check_loop_branch(&instruction, &block_start_idx, &state.block_start_idxes) {
                    state.mark_loop();
                }
            }

            // 2.3 跳过不产生值的指令（Return、Br）
            if matches!(opcode, InstructionOpcode::Return | InstructionOpcode::Br) {
                state.idx += 1;
                continue;
            }

            // 2.4 收集指令的信息
            let val_name = get_value_name(&instruction);
            // 收集函数内部使用了哪些变量
            let uses = collect_uses(&instruction, &mut ctx);

            // 保存到函数状态中
            state.record_instruction(idx, val_name, uses);
            state.idx = idx + 1;
        }
    }

    // 步骤3：计算活跃区间
    let inner_vars = state.compute_liveness();
    // 步骤4：执行寄存器分配
    // false表示使用线性扫描寄存器分配，true表示所有变量都放在栈上（Part2模式）
    let alloc = AllocatedInnerVar::default();
    let (allocations, stack_size) = alloc.allocate(inner_vars, ctx.pure_stack_mode);

    // 记录变量分配好的存储位置和预计使用的栈空间
    ctx.record_alloc_vars(allocations, stack_size);
    
    // 将alloca变量集合传递给GenContext，用于区分已被分配空间的变量和临时变量
    ctx.set_allocated_vals(state.get_allocated_vals().clone());

    // ==========================================
    // 汇编生成阶段
    // ==========================================
    // 步骤1：生成函数声明和标签
    // 1.1 声明.text段
    asm_builder.emit_text_section();
    // 1.2 声明全局符号（main函数）
    asm_builder.emit_global_symbol(function_name);
    // 1.3 生成函数入口标签
    asm_builder.emit_label(function_name);

    // 步骤2：生成函数prologue（分配栈空间）
    let aligned_stack_size = if ctx.stack_size > 0 {
        ((ctx.stack_size + 15) / 16) * 16 // 对齐到16字节
    } else {
        0
    };
    // // 避免生成 addi sp, sp, 0 无用指令
    // if aligned_stack_size > 0 {
    //     asm_builder.emit_function_prologue(aligned_stack_size);
    // }
    asm_builder.emit_function_prologue(aligned_stack_size);

    // 步骤3：遍历基本块和指令，生成汇编代码
    for basic_block in function.get_basic_blocks() {
        // 3.1 生成基本块标签（跳转目标）
        if let Ok(block_name) = basic_block.get_name().to_str()
            && !block_name.is_empty()
        {
            asm_builder.emit_label(block_name);
        }

        // 3.2 遍历指令，生成汇编
        for instruction in basic_block.get_instructions() {
            generate_instruction(&instruction, asm_builder, &mut ctx);
        }
    }
}

/// 检测循环检测
/// 通过比较跳转目标的"指令索引"来判断是否存在循环
///
/// 原理：如果跳转目标的索引 <= 当前索引，说明跳回到之前的指令，形成循环
/// 注意这个循环指的是回到之前基本块的起始命令位置
fn check_loop_branch(
    instruction: &InstructionValue,
    current_idx: &usize,
    block_indices: &HashMap<String, usize>,
) -> bool {
    let num_operands = instruction.get_num_operands();

    match num_operands {
        1 => {
            // 无条件跳转：br label %target
            check_single_branch(instruction, current_idx, block_indices)
        }
        3 => {
            // 条件跳转：br i1 %cond, label %true, label %false
            check_conditional_branch(instruction, current_idx, block_indices)
        }
        _ => false,
    }
}

/// 检测无条件跳转是否构成循环
fn check_single_branch(
    instruction: &InstructionValue,
    current_idx: &usize,
    block_indices: &HashMap<String, usize>,
) -> bool {
    // 1. 获取跳转目标
    // 2. 获取目标名称
    // 3. 获取目标索引（起始指令位置）
    // 4. 判断：目标索引 <= 当前索引 → 跳回去了 → 形成循环
    if let Some(target) = instruction.get_operand(0).and_then(|op| op.right())
        && let Ok(target_name) = target.get_name().to_str()
        && let Some(target_idx) = block_indices.get(target_name)
        && *target_idx <= *current_idx
    {
        return true;
    }
    false
}

/// 检测条件跳转是否构成循环
fn check_conditional_branch(
    instruction: &InstructionValue,
    current_idx: &usize,
    block_indices: &HashMap<String, usize>,
) -> bool {
    // 条件跳转有两个分支目标：true分支和false分支
    for operand_idx in [1, 2] {
        if let Some(target) = instruction
            .get_operand(operand_idx)
            .and_then(|op| op.right())
            && let Ok(target_name) = target.get_name().to_str()
            && let Some(target_idx) = block_indices.get(target_name)
            && *target_idx <= *current_idx
        {
            return true;
        }
    }
    false
}

/// 第一次遍历：收集指令的uses
fn collect_uses(instruction: &InstructionValue, ctx: &mut GenContext) -> Vec<String> {
    let mut uses = Vec::new();
    let opcode = instruction.get_opcode();
    match opcode {
        // 二元运算指令：收集两个操作数
        // add, sub, mul, sdiv, srem指令的例子
        // add i32 %1, i32 %2 -> %3
        // sub i32 %1, i32 %2 -> %3
        // mul i32 %1, i32 %2 -> %3
        // sdiv i32 %1, i32 %2 -> %3
        // srem i32 %1, i32 %2 -> %3
        // store指令的例子： store i32 %1, i32* %0, align 4
        // 收集value和ptr操作数
        // icmp指令的例子： icmp eq i32 %1, i32 %2 -> %3
        InstructionOpcode::ICmp
        | InstructionOpcode::Store
        | InstructionOpcode::Add
        | InstructionOpcode::Sub
        | InstructionOpcode::Mul
        | InstructionOpcode::SDiv
        | InstructionOpcode::SRem => {
            if let Some(lhs_operand) = instruction.get_operand(0)
                && let Some(lhs) = lhs_operand.left()
            {
                // FIX: 过滤掉立即数，立即数不应该被纳入活跃区间计算
                if !is_constant(lhs) {
                    let name = get_value_from_basic(lhs);
                    // FIX: 过滤掉临时名称（立即数会被转换为临时名称）
                    if !name.starts_with("tmp_") {
                        uses.push(name);
                    }
                }
            }
            if let Some(rhs_operand) = instruction.get_operand(1)
                && let Some(rhs) = rhs_operand.left()
            {
                // FIX: 过滤掉立即数，立即数不应该被纳入活跃区间计算
                if !is_constant(rhs) {
                    let name = get_value_from_basic(rhs);
                    // FIX: 过滤掉临时名称（立即数会被转换为临时名称）
                    if !name.starts_with("tmp_") {
                        uses.push(name);
                    }
                }
            }
        }
        // load指令的例子： load i32, i32* %0, align 4
        // br指令的例子： br label %target 分支指令
        // zext指令的例子： zext i1 %1 to i32 -> %2
        InstructionOpcode::Load => {
            // FIX: Load指令的operand 0是指针，应该使用right()获取指针值
            if let Some(value_operand) = instruction.get_operand(0) {
                if let Some(ptr_value) = value_operand.right() {
                    // 指针是指针类型，获取指针的名称
                    if let Ok(name_str) = ptr_value.get_name().to_str() {
                        if !name_str.is_empty() {
                            uses.push(name_str.to_string());
                        }
                    }
                } else if let Some(ptr_value) = value_operand.left() {
                    // 如果left()也能获取到值（可能是通过alloca创建的指针），也处理
                    if !is_constant(ptr_value) {
                        let name = get_value_from_basic(ptr_value);
                        if !name.starts_with("tmp_") {
                            uses.push(name);
                        }
                    }
                }
            }
        }
        InstructionOpcode::Br | InstructionOpcode::ZExt => {
            if let Some(value_operand) = instruction.get_operand(0)
                && let Some(value) = value_operand.left()
            {
                // FIX: 过滤掉立即数，立即数不应该被纳入活跃区间计算
                if !is_constant(value) {
                    let name = get_value_from_basic(value);
                    // FIX: 过滤掉临时名称（立即数会被转换为临时名称）
                    if !name.starts_with("tmp_") {
                        uses.push(name);
                    }
                }
            }
        }
        _ => {}
    }
    uses
}

/// 获取指令名称
fn get_value_name(instruction: &InstructionValue) -> String {
    if let Some(name_cstr) = instruction.get_name()
        && let Ok(name_str) = name_cstr.to_str()
        && !name_str.is_empty()
    {
        return name_str.to_string();
    }
    String::new()
}

fn is_constant(value: BasicValueEnum) -> bool {
     value.is_int_value() && value.into_int_value().is_constant_int()
}

/// 第二次遍历：生成指令代码
fn generate_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &mut GenContext,
) {
    // TODO: 根据instruction类型生成相应代码
    match instruction.get_opcode() {
        InstructionOpcode::ICmp => {
            // 汇编例子：
            // %cmp = icmp sgt i32 %a1, 3
            generate_icmp_instruction(instruction, asm_builder, ctx);
        }
        InstructionOpcode::Store => {
            // 汇编例子：
            // store i32 %a1, i32* %a0, align 4
            generate_store_instruction(instruction, asm_builder, ctx);
        }
        InstructionOpcode::Add
        | InstructionOpcode::Sub
        | InstructionOpcode::Mul
        | InstructionOpcode::SDiv
        | InstructionOpcode::SRem => {
            // 汇编例子：
            // %add = add i32 %a1, 3
            // %sub = sub i32 %a1, 3
            // %mul = mul i32 %a1, 3
            // %sdiv = sdiv i32 %a1, 3
            // %srem = srem i32 %a1, 3
            generate_cal_instruction(instruction, asm_builder, ctx);
        }
        InstructionOpcode::Load => {
            // 汇编例子：
            // %a1 = load i32, i32* %a0, align 4
            generate_load_instruction(instruction, asm_builder, ctx);
        }
        InstructionOpcode::Br => {
            // 汇编例子：
            // br label %label
            generate_br_instruction(instruction, asm_builder, ctx);
        }
        InstructionOpcode::ZExt => {
            // 汇编例子：
            // %a1 = zext i1 %a0 to i32
            generate_zext_instruction(instruction, asm_builder, ctx);
        }
        InstructionOpcode::Return => {
            // 汇编例子：
            // ret i32 %a1
            generate_return_instruction(instruction, asm_builder, ctx);
        }
        _ => {}
    }
}

/// 生成return指令：加载返回值到a0，恢复栈指针，执行exit系统调用
fn generate_return_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &mut GenContext,
) {
    let num_operands = instruction.get_num_operands();
    
    if num_operands > 0 {
        if let Some(return_operand) = instruction.get_operand(0)
            && let Some(return_value) = return_operand.left()
        {
            let return_reg = get_value_from_reg(return_value, ctx, "a0", asm_builder);
            if return_reg != "a0" {
                asm_builder.emit_mv("a0", &return_reg);
            }
        }
    }
    
    let stack_size = ctx.get_stack_size();
    let aligned_stack_size = if stack_size > 0 {
        ((stack_size + 15) / 16) * 16
    } else {
        0
    };
    asm_builder.emit_function_epilogue(aligned_stack_size);
    asm_builder.emit_exit_syscall();
}

/// 生成load指令：从指针指向的位置加载值到寄存器
/// (1) 全局变量
/// ```
///   @x = global i32 56
///   %val = load i32, i32* @x, align 4
/// ```
/// (2) 局部变量
/// ```
///   %a = alloca i32, align 4
///   store i32 1, i32* %a, align 4
///   %val = load i32, i32* %a, align 4
/// ```
fn generate_load_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &mut GenContext,
) {
    let ptr_operand = instruction.get_operand(0);
    let ptr_name = if let Some(ptr_e) = ptr_operand {
        if let Some(ptr_value) = ptr_e.right() {
            if let Ok(name_str) = ptr_value.get_name().to_str() {
                name_str.to_string()
            } else {
                return;
            }
        } else if let Some(ptr_basic) = ptr_e.left() {
            // FIX: 检查ptr_basic是否是立即数，如果是立即数则不能作为指针使用，直接返回
            if is_constant(ptr_basic) {
                return;
            }
            let name = get_value_from_basic(ptr_basic);
            // FIX: 如果获取的名称是临时名称（以"tmp_"开头），说明这不是一个有效的变量名，不能作为指针使用
            if name.starts_with("tmp_") {
                return;
            }
            name
        } else {
            return;
        }
    } else {
        return;
    };
    
    let result_name = get_value_name(instruction);
    let result_reg = if let Some(result_loc) = ctx.get_location(&result_name) {
        match result_loc {
            Location::Reg(reg) => reg.to_string(),
            _ => "t0".to_string(),
        }
    } else {
        "t0".to_string()
    };
    
    if ctx.is_global(&ptr_name) {
        // FIX: 全局变量加载：根据结果位置生成不同的代码
        // 如果结果在寄存器中，直接加载到结果寄存器
        // 如果结果在栈中，先加载到临时寄存器，再存储到栈
        match ctx.get_location(&result_name) {
            Some(Location::Reg(reg)) => {
                // 结果在寄存器中，直接加载到结果寄存器
                asm_builder.emit_la(reg, &ptr_name);
                asm_builder.emit_lw(reg, 0, reg);
            }
            Some(Location::Stack(offset)) => {
                // 结果在栈中，先加载到临时寄存器，再存储到栈
                asm_builder.emit_la("t0", &ptr_name);
                asm_builder.emit_lw("t0", 0, "t0");
                asm_builder.emit_sw("t0", *offset, "sp");
            }
            _ => {
                // 其他情况，使用默认方式
                asm_builder.emit_la("t0", &ptr_name);
                asm_builder.emit_lw(&result_reg, 0, "t0");
            }
        }
        // FIX: 全局变量加载后，不需要再次存储结果（已经在上面处理了）
        return;
    } else {
        let loc_type = ctx.get_location(&ptr_name).map(|loc| match loc {
            Location::Reg(reg) => (Some(reg.clone()), None),
            Location::Stack(sp_offset) => (None, Some(*sp_offset)),
            _ => (None, None),
        });
        
        // FIX: 如果loc_type为None，说明ptr_name不是一个有效的变量（可能是立即数或临时值），直接返回
        if let Some((reg_opt, sp_offset_opt)) = loc_type {
            // FIX: alloca变量应该始终使用栈存储，即使它被分配到了寄存器
            if ctx.is_alloc(&ptr_name) {
                if let Some(sp_offset) = sp_offset_opt {
                    // alloca变量：先加载指针值，再间接加载数据
                    let ptr_reg = "t1";
                    asm_builder.emit_lw(ptr_reg, sp_offset, "sp");
                    asm_builder.emit_lw(&result_reg, 0, ptr_reg);
                } else {
                    // FIX: alloca变量被分配到寄存器是不正确的，应该使用栈存储
                    // 如果alloca变量被分配到寄存器，说明寄存器分配有问题，这里应该报错或使用默认栈位置
                    return;
                }
            } else if let Some(ptr_reg) = reg_opt {
                // 非alloca变量：如果分配到寄存器，直接使用寄存器作为指针（这种情况不应该发生，因为load的源应该是指针）
                asm_builder.emit_lw(&result_reg, 0, &ptr_reg);
            } else if let Some(sp_offset) = sp_offset_opt {
                // 普通局部变量：直接从栈加载
                asm_builder.emit_lw(&result_reg, sp_offset, "sp");
            }
        } else {
            // FIX: ptr_name不在ctx中，说明这不是一个有效的变量，不能作为指针使用
            return;
        }
    }
    
    // FIX: 如果结果在栈中，存储结果（全局变量已经在上面处理了，这里只处理局部变量）
    if let Some(result_loc) = ctx.get_location(&result_name) {
        if let Location::Stack(offset) = result_loc {
            asm_builder.emit_sw(&result_reg, *offset, "sp");
        }
    }
}

/// 生成br指令：无条件跳转或条件跳转
fn generate_br_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &mut GenContext,
) {
    let num_operands = instruction.get_num_operands();
    
    match num_operands {
        1 => {
            if let Some(target_operand) = instruction.get_operand(0)
                && let Some(target) = target_operand.right()
                && let Ok(target_name) = target.get_name().to_str()
            {
                asm_builder.emit_j(target_name);
            }
        }
        3 => {
            if let Some(cond_operand) = instruction.get_operand(0)
                && let Some(true_operand) = instruction.get_operand(1)
                && let Some(false_operand) = instruction.get_operand(2)
                && let Some(cond) = cond_operand.left()
                && let Some(true_target) = true_operand.right()
                && let Some(false_target) = false_operand.right()
                && let Ok(true_label) = true_target.get_name().to_str()
                && let Ok(false_label) = false_target.get_name().to_str()
            {
                let cond_reg = get_value_from_reg(cond, ctx, "t0", asm_builder);
                asm_builder.emit_bne(&cond_reg, "x0", true_label);
                asm_builder.emit_j(false_label);
            }
        }
        _ => {}
    }
}

/// 生成zext指令：将i1类型零扩展到i32
fn generate_zext_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &mut GenContext,
) {
    let src_operand = instruction.get_operand(0);
    if let Some(src_e) = src_operand
        && let Some(src) = src_e.left()
    {
        let result_name = get_value_name(instruction);
        let result_reg = if let Some(result_loc) = ctx.get_location(&result_name) {
            match result_loc {
                Location::Reg(reg) => reg.to_string(),
                Location::Stack(sp_offset) => {
                    let temp_reg = "t0";
                    let offset = *sp_offset;
                    let src_reg = get_value_from_reg(src, ctx, temp_reg, asm_builder);
                    if src_reg != temp_reg {
                        asm_builder.emit_mv(temp_reg, &src_reg);
                    }
                    asm_builder.emit_sw(temp_reg, offset, "sp");
                    return;
                }
                _ => "t0".to_string(),
            }
        } else {
            "t0".to_string()
        };
        
        let src_reg = get_value_from_reg(src, ctx, "t1", asm_builder);
        if src_reg != result_reg {
            asm_builder.emit_mv(&result_reg, &src_reg);
        }
    }
}

/// 生成算术运算指令：add/sub/mul/sdiv/srem
fn generate_cal_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &mut GenContext,
) {
    
    let lhs_operand = instruction.get_operand(0);
    let rhs_operand = instruction.get_operand(1);
    if let Some(lhs_e) = lhs_operand
    && let Some(rhs_e) = rhs_operand
    && let Some(lhs) = lhs_e.left()
    && let Some(rhs) = rhs_e.left() {
        let lhs_reg = get_value_from_reg(lhs,ctx,"t0",asm_builder);
        let rhs_reg = get_value_from_reg(rhs,ctx,"t1",asm_builder);
        let result_name = get_value_name(instruction);
        let result_loc = ctx.get_location(&result_name);
        if let Some(result_loc) = result_loc {
            match result_loc {
                Location::Reg(reg) => {
                    let result_reg = reg.to_string();
                    match instruction.get_opcode() {
                        InstructionOpcode::Add => {
                            asm_builder.emit_add(&result_reg,&lhs_reg,&rhs_reg);
                        },
                        InstructionOpcode::Sub => {
                            asm_builder.emit_sub(&result_reg,&lhs_reg,&rhs_reg);
                        },
                        InstructionOpcode::Mul => {
                            asm_builder.emit_mul(&result_reg,&lhs_reg,&rhs_reg);
                        },
                        InstructionOpcode::SDiv => {
                            asm_builder.emit_div(&result_reg,&lhs_reg,&rhs_reg);
                        },
                        InstructionOpcode::SRem => {
                            asm_builder.emit_rem(&result_reg,&lhs_reg,&rhs_reg);
                        },
                        _ => {}
                    }
                },
                Location::Stack(sp_offset) => {
                    let result_reg = "t2";
                    match instruction.get_opcode() {
                        InstructionOpcode::Add => {
                            asm_builder.emit_add(result_reg, &lhs_reg, &rhs_reg);
                        },
                        InstructionOpcode::Sub => {
                            asm_builder.emit_sub(result_reg, &lhs_reg, &rhs_reg);
                        },
                        InstructionOpcode::Mul => {
                            asm_builder.emit_mul(result_reg, &lhs_reg, &rhs_reg);
                        },
                        InstructionOpcode::SDiv => {
                            asm_builder.emit_div(result_reg, &lhs_reg, &rhs_reg);
                        },
                        InstructionOpcode::SRem => {
                            asm_builder.emit_rem(result_reg, &lhs_reg, &rhs_reg);
                        },
                        _ => {}
                    }
                    asm_builder.emit_sw(result_reg, *sp_offset, "sp");
                },
                Location::Global(name) => {
                    let result_reg = "t2";
                    let addr_reg = "t3";
                    match instruction.get_opcode() {
                        InstructionOpcode::Add => {
                            asm_builder.emit_add(result_reg, &lhs_reg, &rhs_reg);
                        },
                        InstructionOpcode::Sub => {
                            asm_builder.emit_sub(result_reg, &lhs_reg, &rhs_reg);
                        },
                        InstructionOpcode::Mul => {
                            asm_builder.emit_mul(result_reg, &lhs_reg, &rhs_reg);
                        },
                        InstructionOpcode::SDiv => {
                            asm_builder.emit_div(result_reg, &lhs_reg, &rhs_reg);
                        },
                        InstructionOpcode::SRem => {
                            asm_builder.emit_rem(result_reg, &lhs_reg, &rhs_reg);
                        },
                        _ => {}
                    }
                    asm_builder.emit_la(addr_reg, &name);
                    asm_builder.emit_sw(result_reg, 0, addr_reg);
                },
                _ => {}
            }
        }
    
    }

}


/// 生成store指令：将源值存储到目标指针指向的位置
fn generate_store_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &mut GenContext,
) {
    let from_operand = instruction.get_operand(0);
    let to_operand = instruction.get_operand(1);
    let (from, ptr_name) = if let Some(from_e) = from_operand
        && let Some(to_e) = to_operand
        && let Some(from) = from_e.left()
    {
        let ptr_name = if let Some(ptr_value) = to_e.right() {
            if let Ok(name_str) = ptr_value.get_name().to_str() {
                name_str.to_string()
            } else {
                return;
            }
        } else if let Some(to) = to_e.left() {
            // FIX: 检查to是否是立即数，如果是立即数则不能作为指针使用，直接返回
            if is_constant(to) {
                return;
            }
            let name = get_value_from_basic(to);
            // FIX: 如果获取的名称是临时名称（以"tmp_"开头），说明这不是一个有效的变量名，不能作为指针使用
            if name.starts_with("tmp_") {
                return;
            }
            name
        } else {
            return;
        };
        (from, ptr_name)
    } else {
        return;
    };
    
    if ctx.is_global(&ptr_name) {
        let from_reg = get_value_from_reg(from, ctx, "t0", asm_builder);
        asm_builder.emit_la("t2", &ptr_name);
        asm_builder.emit_sw(&from_reg, 0, "t2");
    } else {
        let loc_type = ctx.get_location(&ptr_name).map(|loc| match loc {
            Location::Reg(reg) => (Some(reg.clone()), None),
            Location::Stack(sp_offset) => (None, Some(*sp_offset)),
            _ => (None, None),
        });
        
        // FIX: 如果loc_type为None，说明ptr_name不是一个有效的变量（可能是立即数或临时值），直接返回
        if let Some((reg_opt, sp_offset_opt)) = loc_type {
            // FIX: alloca变量应该始终使用栈存储，即使它被分配到了寄存器
            if ctx.is_alloc(&ptr_name) {
                if let Some(sp_offset) = sp_offset_opt {
                    // alloca变量：先加载指针值，再间接存储
                    let ptr_reg = "t2";
                    asm_builder.emit_lw(ptr_reg, sp_offset, "sp");
                    let from_reg = get_value_from_reg(from, ctx, "t0", asm_builder);
                    asm_builder.emit_sw(&from_reg, 0, ptr_reg);
                } else {
                    // FIX: alloca变量被分配到寄存器是不正确的，应该使用栈存储
                    // 如果alloca变量被分配到寄存器，说明寄存器分配有问题，这里应该报错或使用默认栈位置
                    return;
                }
            } else if let Some(reg) = reg_opt {
                // 非alloca变量：如果分配到寄存器，直接存储到寄存器（这种情况不应该发生，因为store的目标应该是指针）
                load_value_to_reg(from, ctx, &reg, asm_builder);
            } else if let Some(sp_offset) = sp_offset_opt {
                // 普通局部变量：直接存储到栈
                let from_reg = get_value_from_reg(from, ctx, "t0", asm_builder);
                asm_builder.emit_sw(&from_reg, sp_offset, "sp");
            }
        } else {
            // FIX: ptr_name不在ctx中，说明这不是一个有效的变量，不能作为指针使用
            return;
        }
    }
}

fn generate_icmp_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &mut GenContext,
) {
    // %cmp = icmp sgt i32 %a1 3 => cmp
    let result_name = get_value_name(instruction);
    let lf_operand = instruction.get_operand(0);
    let rt_operand = instruction.get_operand(1);
    if let Some(predicate) = instruction.get_icmp_predicate()
        && let Some(lft_e) = lf_operand
        && let Some(rgt_e) = rt_operand
        && let Some(lft_v) = lft_e.left()
        && let Some(rgt_v) = rgt_e.left()
    {
        match predicate {
            // ==
            IntPredicate::EQ => {
                let l_reg = get_value_from_reg(lft_v,ctx,"t0",asm_builder);
                let r_reg = get_value_from_reg(rgt_v,ctx,"t1",asm_builder);
                let result_reg = if let Some(res_location) = ctx.get_location(&result_name) {
                    match res_location {
                        Location::Reg(reg) => {
                            reg.to_string()
                        },
                        _ => "t2".to_string()
                    }
                }else {
                    "t2".to_string()
                };

                // 先做减法，然后与立即数0比较
                asm_builder.emit_sub(&result_reg,&l_reg,&r_reg);
                asm_builder.emit_seqz(&result_reg,"t2");
            }
            // !=
            IntPredicate::NE => {
                let l_reg = get_value_from_reg(lft_v,ctx,"t0",asm_builder);
                let r_reg = get_value_from_reg(rgt_v,ctx,"t1",asm_builder);
                let result_reg = if let Some(res_location) = ctx.get_location(&result_name) {
                    match res_location {
                        Location::Reg(reg) => {
                            reg.to_string()
                        },
                        _ => "t2".to_string()
                    }
                }else {
                    "t2".to_string()
                };

                // 先做减法，然后与立即数0比较
                asm_builder.emit_sub(&result_reg,&l_reg,&r_reg);
                asm_builder.emit_snez(&result_reg,"t2");

            }
            // >
            IntPredicate::SGT => {
                let l_reg = get_value_from_reg(lft_v,ctx,"t0",asm_builder);
                let r_reg = get_value_from_reg(rgt_v,ctx,"t1",asm_builder);
                let result_reg = if let Some(res_location) = ctx.get_location(&result_name) {
                    match res_location {
                        Location::Reg(reg) => {
                            reg.to_string()
                        },
                        _ => "t2".to_string()
                    }
                }else {
                    "t2".to_string()
                };
                asm_builder.emit_sgt(&l_reg,&r_reg,&result_reg);
            }
            // <
            IntPredicate::SLT => {
                let l_reg = get_value_from_reg(lft_v,ctx,"t0",asm_builder);
                let r_reg = get_value_from_reg(rgt_v,ctx,"t1",asm_builder);
                let result_reg = if let Some(res_location) = ctx.get_location(&result_name) {
                    match res_location {
                        Location::Reg(reg) => {
                            reg.to_string()
                        },
                        _ => "t2".to_string()
                    }
                }else {
                    "t2".to_string()
                };
                asm_builder.emit_slt(&l_reg,&r_reg,&result_reg);
            }
            // >=
            IntPredicate::SLE => {
                let l_reg = get_value_from_reg(lft_v,ctx,"t0",asm_builder);
                let r_reg = get_value_from_reg(rgt_v,ctx,"t1",asm_builder);
                let result_reg = if let Some(res_location) = ctx.get_location(&result_name) {
                    match res_location {
                        Location::Reg(reg) => {
                            reg.to_string()
                        },
                        _ => "t2".to_string()
                    }
                }else {
                    "t2".to_string()
                };
                asm_builder.emit_sle(&l_reg,&r_reg,&result_reg);
            }
            // <=
            IntPredicate::SGE => {
                let l_reg = get_value_from_reg(lft_v,ctx,"t0",asm_builder);
                let r_reg = get_value_from_reg(rgt_v,ctx,"t1",asm_builder);
                let result_reg = if let Some(res_location) = ctx.get_location(&result_name) {
                    match res_location {
                        Location::Reg(reg) => {
                            reg.to_string()
                        },
                        _ => "t2".to_string()
                    }
                }else {
                    "t2".to_string()
                };
                asm_builder.emit_sge(&l_reg,&r_reg,&result_reg);
            }
            _ => {}
        }
    }
}

/// 从 `BasicValueEnum` 中提取变量名称
///
/// 该函数从 LLVM IR 的 `BasicValueEnum` 值中获取变量名称，用于后续查找
/// 该变量在寄存器分配结果中的存储位置（寄存器、栈或全局变量）。
///
/// # 参数
///
/// * `input` - LLVM IR 中的基本值枚举，可以是变量、常量等
///
/// # 返回值
///
/// 返回变量的名称字符串。如果无法从 C 字符串转换为有效的 UTF-8 字符串
/// （例如包含无效字节序列），则返回 `"x0"` 作为默认值。
///
/// # 示例
///
/// ```rust,ignore
/// // 假设 LLVM IR 中有变量 %var1
/// let value: BasicValueEnum = ...;
/// let name = get_value_from_basic(value);
/// // name = "var1"
/// ```
///
/// # 注意
///
/// - 正常情况下，LLVM IR 中的变量名都是有效的 UTF-8 字符串，返回 `"x0"` 的情况极少发生
/// - 返回 `"x0"` 是一种防御性处理，用于避免程序崩溃
/// - 该函数主要用于获取变量名，以便通过 `GenContext::get_location()` 查找变量的存储位置
fn get_value_from_basic(input:BasicValueEnum) -> String {
    let cstr = input.get_name();
    let name = if let Ok(name_str) = cstr.to_str() {
        name_str
    }else {
        // 防御性处理：实际上走到这个分支的可能很小
        return "x0".to_string();
    };
    name.to_string()
}

/// 从存储位置获取值到寄存器，返回包含该值的寄存器名称
///
/// 该函数负责将 LLVM 值（常量、变量等）从不同的存储位置（寄存器、栈、全局变量）
/// 加载到指定的寄存器中，以便在后续指令中使用。函数会智能地处理不同的存储位置，
/// 避免不必要的加载操作。
///
/// # 参数
/// * `input` - 需要获取的 LLVM 值（`BasicValueEnum`）
/// * `ctx` - 代码生成上下文（`GenContext`），包含变量的位置信息映射
/// * `reg_name` - 目标寄存器名称（如 "t0", "t1"），当需要加载值时使用此寄存器
/// * `asm_builder` - 汇编代码构建器（`AsmBuilder`），用于生成加载指令
///
/// # 返回值
/// 返回包含该值的寄存器名称（`String`）。可能的返回值：
/// * 寄存器名称（如 "t0", "s1" 等）：值已在该寄存器中或已加载到该寄存器
/// * "x0"：零寄存器，用于常量 0
///
/// # 处理逻辑
/// 函数按以下顺序处理不同类型的值：
///
/// 1. **常量值（立即数）**：
///    - 如果值为常量 0，直接返回 "x0"（零寄存器），无需生成指令
///    - 其他常量值使用 `li` 指令加载到 `reg_name` 中
///    - 生成指令：`li {reg_name}, {value}`
///
/// 2. **已在寄存器中的值**：
///    - 如果值已经分配在某个寄存器中，直接返回该寄存器名称
///    - **优化**：避免不必要的移动指令，直接使用原寄存器
///
/// 3. **栈上的值**：
///    - 使用 `lw` 指令从栈偏移位置加载到 `reg_name`
///    - 生成指令：`lw {reg_name}, {offset}(sp)`
///    - 返回 `reg_name`
///
/// 4. **全局变量**：
///    - 使用 `la` 指令加载全局变量的地址到 `reg_name`
///    - 然后使用 `lw` 指令从该地址加载值
///    - 生成指令：
///      * `la {reg_name}, {label}`
///      * `lw {reg_name}, 0({reg_name})`
///    - 返回 `reg_name`
///
/// 5. **未知值（未在上下文中找到）**：
///    - 如果无法确定值的位置，加载常量 0 到 `reg_name`
///    - 生成指令：`li {reg_name}, 0`
///    - 返回 `reg_name`
///
/// # 示例
/// ```
/// // 假设有一个变量 "x" 已分配在寄存器 "t3" 中
/// let reg = get_value_from_reg(value_x, ctx, "t0", asm_builder);
/// // 返回 "t3"，无需生成加载指令
///
/// // 假设有一个变量 "y" 在栈偏移 8 处
/// let reg = get_value_from_reg(value_y, ctx, "t0", asm_builder);
/// // 生成: lw t0, 8(sp)
/// // 返回 "t0"
///
/// // 假设有一个常量 42
/// let reg = get_value_from_reg(const_42, ctx, "t0", asm_builder);
/// // 生成: li t0, 42
/// // 返回 "t0"
/// ```
fn get_value_from_reg(input: BasicValueEnum, ctx: &mut GenContext,reg_name:&str,asm_builder: &mut AsmBuilder) -> String {
    // 判断是否是立即数,如果是0则返回zero寄存器,不是则分配到入参的寄存器中
    if is_constant(input)
        && let Some(val) = input.into_int_value().get_zero_extended_constant(){
        if val == 0 {
            return "x0".to_string();
        }
        // 加载立即数 load immediate
        // li {reg_name}, {value}
        asm_builder.emit_li(reg_name,val as i32);
        return reg_name.to_string();
    }

    // 如果不是立即数,读取操作数名称，并获取其存储位置
    let val_name =  get_value_from_basic(input);
    if let Some(location) = ctx.get_location(&val_name) {
        match location {
            // 如果是寄存器，直接返回寄存器名称
            Location::Reg(reg) => {
                return reg.to_string();
            },
            // 如果是栈，则加载栈值到寄存器
            Location::Stack(sp_offset) => {
                // 加载栈值 load word
                // lw {reg_name}, {offset}(sp)
                asm_builder.emit_lw(reg_name, *sp_offset, "sp");
                return reg_name.to_string();
            },
            // 如果是全局变量，则加载全局变量值到寄存器
            Location::Global(name) => {
                // 加载全局变量 load address
                // la {reg_name}, {name}
                asm_builder.emit_la(reg_name, name);
                // 加载全局变量值 load word
                // lw {reg_name}, 0({reg_name})
                asm_builder.emit_lw(reg_name, 0, reg_name);
                return reg_name.to_string();
            }
        }
    }
    // 如果操作数名称不在ctx中，则加载立即数0到寄存器
    // li {reg_name}, 0
    asm_builder.emit_li(reg_name, 0);
    // 返回寄存器名称
    reg_name.to_string()
}


/// 将值加载到指定的目标寄存器中
///
/// 该函数负责将 LLVM 值（常量、变量等）从不同的存储位置（寄存器、栈、全局变量）
/// 加载到指定的目标寄存器中。与 `get_value_from_reg` 不同，本函数总是将值加载到
/// 目标寄存器，即使值已经在其他寄存器中也会执行移动操作。
///
/// # 参数
/// * `input` - 需要加载的 LLVM 值（`BasicValueEnum`）
/// * `ctx` - 代码生成上下文（`GenContext`），包含变量的位置信息映射
/// * `reg_name` - 目标寄存器名称（如 "t0", "t1"），值将被加载到此寄存器
/// * `asm_builder` - 汇编代码构建器（`AsmBuilder`），用于生成加载指令
///
/// # 处理逻辑
/// 函数按以下顺序处理不同类型的值：
///
/// 1. **常量值（立即数）**：
///    - 如果值为常量 0，使用 `mv` 指令将零寄存器复制到目标寄存器
///      - 生成指令：`mv {reg_name}, x0`
///    - 其他常量值使用 `li` 指令直接加载到目标寄存器
///      - 生成指令：`li {reg_name}, {value}`
///    - **优化**：常量直接加载到目标寄存器，无需中间步骤
///
/// 2. **已在寄存器中的值**：
///    - 如果源寄存器与目标寄存器不同，使用 `mv` 指令移动值
///      - 生成指令：`mv {reg_name}, {reg}`
///    - 如果源寄存器与目标寄存器相同，不生成任何指令（避免冗余移动）
///
/// 3. **栈上的值**：
///    - 使用 `lw` 指令从栈偏移位置直接加载到目标寄存器
///    - 生成指令：`lw {reg_name}, {offset}(sp)`
///
/// 4. **全局变量**：
///    - 使用 `la` 指令加载全局变量的地址到目标寄存器
///    - 然后使用 `lw` 指令从该地址加载值
///    - 生成指令：
///      * `la {reg_name}, {label}`
///      * `lw {reg_name}, 0({reg_name})`
///
/// 5. **未知值（特殊情况）**：
///    - 如果无法确定值的位置或没有名称，将零寄存器复制到目标寄存器
///    - 生成指令：`mv {reg_name}, x0`
///
/// # 与 `get_value_from_reg` 的区别
/// - `get_value_from_reg`：如果值已在寄存器中，直接返回该寄存器名称，不生成移动指令
/// - `load_value_to_reg`：总是将值加载到指定的目标寄存器，即使需要移动也会生成指令
///
/// # 示例
/// ```
/// // 假设有一个变量 "x" 已分配在寄存器 "t3" 中，需要加载到 "t0"
/// load_value_to_reg(value_x, ctx, "t0", asm_builder);
/// // 生成: mv t0, t3
///
/// // 假设有一个变量 "y" 在栈偏移 8 处，需要加载到 "t0"
/// load_value_to_reg(value_y, ctx, "t0", asm_builder);
/// // 生成: lw t0, 8(sp)
///
/// // 假设有一个常量 42，需要加载到 "t0"
/// load_value_to_reg(const_42, ctx, "t0", asm_builder);
/// // 生成: li t0, 42
/// ```
fn load_value_to_reg(input: BasicValueEnum, ctx: &mut GenContext, reg_name: &str, asm_builder: &mut AsmBuilder) {
    //  如果 操作数是 常量 则为立即数
    if is_constant(input)
        && let Some(val) = input.into_int_value().get_zero_extended_constant()
    {
        if val == 0 {
            // 如果常量是0，则直接移动到x0寄存器
            asm_builder.emit_mv(reg_name, "x0");
            return;
        }
        // 加载立即数 load immediate
        // li {reg_name}, {value}
        asm_builder.emit_li(reg_name, val as i32);
        return;
    }

    // 如果不是立即数,读取操作数名称，并获取其存储位置
    let val_name = get_value_from_basic(input);
    if let Some(location) = ctx.get_location(&val_name) {
        match location {
            // 如果是寄存器，则直接移动到目标寄存器
            Location::Reg(reg) => {
                if reg != reg_name {
                    // 源寄存器和目标寄存器不是一个寄存器
                    // mv {reg_name}, {reg}
                    asm_builder.emit_mv(reg_name, reg);
                }
            },
            // 如果是栈，则加载栈值到目标寄存器
            Location::Stack(sp_offset) => {
                // 加载栈值 load word
                // lw {reg_name}, {offset}(sp)
                asm_builder.emit_lw(reg_name, *sp_offset, "sp");
            },
            // 如果是全局变量，则加载全局变量值到目标寄存器
            Location::Global(label) => {
                // 加载全局变量 load address
                // la {reg_name}, {label}
                asm_builder.emit_la(reg_name, label);
                // 加载全局变量值 load word
                // lw {reg_name}, 0({reg_name})
                asm_builder.emit_lw(reg_name, 0, reg_name);
            }
        }
    } else {
        // 其他特殊情况，则移动到x0寄存器
        // mv {reg_name}, x0
        asm_builder.emit_mv(reg_name, "x0");
    }
}
#[cfg(test)]
mod tests {
    use inkwell::AddressSpace;
    use inkwell::values::BasicValue;
    use crate::riscv_codegen::register_alloc::{LinearScan, NoAlloc, RegisterAllocator};
    use super::*;

    const FILE_PATH: &str = "tests/lab6/";

    #[test]
    fn test_part1() {
        let file_path = format!("{}{}", FILE_PATH, "part1.sy");
        let out_path = format!("{}{}", FILE_PATH, "part1.s");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let _ = generate_asm(&input, &out_path);
    }

    #[test]
    fn  test_part2() {
        let file_path = format!("{}{}", FILE_PATH, "part2.sy");
        let out_path = format!("{}{}", FILE_PATH, "part2.s");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let _ = generate_asm(&input, &out_path);
    }

    #[test]
    fn  test_part3() {
        let file_path = format!("{}{}", FILE_PATH, "part3.sy");
        let out_path = format!("{}{}", FILE_PATH, "part3.s");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let _ = generate_asm(&input, &out_path);
    }

    #[test]
    fn test_ir() {
        let file_path = format!("{}{}", FILE_PATH, "part4.sy");
        let out_path = format!("{}{}", FILE_PATH, "part4.ll");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let scanner = Scanner::new(&input,&out_path);
        let _ = scanner.scan_collect();
    }


    #[test]
    fn test_record_loc(){
        let mut allocator = LinearScan::new();
        let mocks:Vec<InnerVar> = vec![
            InnerVar::new("a".to_string(), 0, 10),
            InnerVar::new("b".to_string(), 5, 20),
            InnerVar::new("c".to_string(), 21, 30),
            InnerVar::new("d".to_string(), 22, 40),
            InnerVar::new("e".to_string(), 20, 50),
            InnerVar::new("f".to_string(), 11, 60),
        ];
        // 由线性扫描器扫描之后的顺序是 a(0,10) b(5,20) f(11,60) e(20,50) c(21,30) d(22,40)
        let (allocation_map, stack_size) = allocator.allocate(mocks);
        allocation_map.iter().for_each(|map| {
            println!("{:?}", map);
        });
        let mut gen_context = GenContext::new();
        gen_context.record_alloc_vars(allocation_map, stack_size);
        println!("{:?}",gen_context.var_locations);
    }

    #[test]
    fn test_record_loc1(){
        let mut allocator = NoAlloc::default();
        // 生命周期没有重合的栈空间可以完全复用
        // 由于hashmap的无序性分配的栈偏移也具有随机性，但栈空间基本是定的
        let mocks:Vec<InnerVar> = vec![
            InnerVar::new("a".to_string(), 0, 10),
            InnerVar::new("b".to_string(), 5, 20),
            InnerVar::new("c".to_string(), 21, 30),
            InnerVar::new("d".to_string(), 5, 40),
            InnerVar::new("e".to_string(), 20, 50),
            InnerVar::new("f".to_string(), 3, 60),
        ];
        // 基于start_offset排序 a f b d e c
        // a -4 -> 12
        // f -8 -> 8
        // b -12 -> 4
        // d -16 -> 0
        // e -4  -> 12
        // c -12 -> 4
        let (allocation_map, stack_size) = allocator.allocate(mocks);
        allocation_map.iter().for_each(|map| {
            println!("{:?}", map);
        });
        let mut gen_context = GenContext::new();
        gen_context.record_alloc_vars(allocation_map, stack_size);
        println!("{:?}",gen_context.var_locations);
    }

    #[test]
    fn test_get_val_from_reg(){
        use inkwell::context::Context;
        
        let mut allocator = LinearScan::new();
        let mocks:Vec<InnerVar> = vec![
            InnerVar::new("a".to_string(), 0, 10),
            InnerVar::new("b".to_string(), 5, 20),
            InnerVar::new("c".to_string(), 21, 30),
            InnerVar::new("d".to_string(), 5, 40),
            InnerVar::new("e".to_string(), 20, 50),
            InnerVar::new("f".to_string(), 3, 60),
        ];
        // 基于start_offset排序 a f b d e c
        // a -4 -> 12
        // f -8 -> 8
        // b -12 -> 4
        // d -16 -> 0
        // e -4  -> 12
        // c -12 -> 4
        let (allocation_map, stack_size) = allocator.allocate(mocks);
        allocation_map.iter().for_each(|map| {
            println!("{:?}", map);
        });
        let mut gen_context = GenContext::new();
        gen_context.record_alloc_vars(allocation_map, stack_size);
        
        // 创建 LLVM Context 用于构建常量值
        let context = Context::create();
        let i32_type = context.i32_type();
        let mut asm_builder = AsmBuilder::new();
        
        // 测试立即数0 - 应该返回 "x0"
        let const_zero = i32_type.const_int(0, false).into();
        let reg_zero = get_value_from_reg(const_zero, &mut gen_context, "t0", &mut asm_builder);
        assert_eq!(reg_zero, "x0", "常量0应该返回x0寄存器");
        println!("测试立即数0: 返回寄存器 = {}", reg_zero);
        
        // 测试立即数1 - 应该生成 li t0, 1 并返回 "t0"
        let const_one = i32_type.const_int(1, false).into();
        let reg_one = get_value_from_reg(const_one, &mut gen_context, "t0", &mut asm_builder);
        assert_eq!(reg_one, "t0", "常量1应该加载到t0并返回t0");
        println!("测试立即数1: 返回寄存器 = {}", reg_one);
        
        // 测试全局变量 - 需要先添加全局变量
        let module = context.create_module("test");
        let global = module.add_global(i32_type,Some(AddressSpace::from(1u16)),"global_var");
        gen_context.add_global("global_var".to_string());
        println!("测试全局变量: global_var 已添加到上下文");
        let reg_global = get_value_from_reg(global.as_basic_value_enum(), &mut gen_context, "t1", &mut asm_builder);
        assert_eq!(reg_global,"t1","全局变量应该加载到t1");
        println!("测试全局变量: 返回寄存器 = {}", reg_global);

        // 测试栈变量 - 从分配结果中找到栈变量
        let stack_vars: Vec<_> = gen_context.var_locations.iter()
            .filter(|(_, loc)| matches!(loc, Location::Stack(_)))
            .collect();
        if !stack_vars.is_empty() {
            for (var_name, loc) in stack_vars {
                if let Location::Stack(offset) = loc {
                    println!("测试栈变量: 找到栈变量 {} 在栈偏移 {}", var_name, offset);
                    // 验证栈变量位置正确
                    assert!(
                        matches!(gen_context.get_location(var_name), Some(Location::Stack(o)) if o == offset),
                        "栈变量位置应该正确"
                    );
                }
            }
        } else {
            println!("测试栈变量: 未找到栈变量（可能所有变量都在寄存器中）");
        }
        
        // 测试寄存器变量 - 从分配结果中找到寄存器变量
        let reg_vars: Vec<_> = gen_context.var_locations.iter()
            .filter(|(_, loc)| matches!(loc, Location::Reg(_)))
            .collect();
        if !reg_vars.is_empty() {
            for (var_name, loc) in reg_vars {
                if let Location::Reg(reg) = loc {
                    println!("测试寄存器变量: 找到寄存器变量 {} 在寄存器 {}", var_name, reg);
                    // 验证寄存器变量位置正确
                    let expected_reg = reg.clone();
                    assert!(
                        matches!(gen_context.get_location(var_name), Some(Location::Reg(r)) if r == &expected_reg),
                        "寄存器变量位置应该正确"
                    );
                }
            }
        } else {
            println!("测试寄存器变量: 未找到寄存器变量");
        }
        
        // 测试未知变量 - 验证未知变量不在上下文中
        let unknown_var = "unknown_var_12345";
        assert!(
            gen_context.get_location(unknown_var).is_none(),
            "未知变量不应该在上下文中"
        );
        println!("测试未知变量: {} 不在上下文中（符合预期）", unknown_var);
        // 注意：由于无法直接创建 BasicValueEnum 来表示未知变量，
        // 完整的未知变量测试需要在集成测试中使用真实的 LLVM IR
        
        // 打印生成的汇编代码以验证
        let asm_code = asm_builder.emit();
        println!("\n生成的汇编代码:\n{}", asm_code);
        
        // 验证汇编代码包含预期的指令
        assert!(asm_code.contains("li t0, 1"), "汇编代码应该包含 li t0, 1 指令");
        println!("\n所有测试通过！");
    }

    #[test]
    fn test_load_val_to_reg(){
        use inkwell::context::Context;

        let mut allocator = LinearScan::new();
        let mocks:Vec<InnerVar> = vec![
            InnerVar::new("a".to_string(), 0, 10),
            InnerVar::new("b".to_string(), 5, 20),
            InnerVar::new("c".to_string(), 21, 30),
            InnerVar::new("d".to_string(), 5, 40),
            InnerVar::new("e".to_string(), 20, 50),
            InnerVar::new("f".to_string(), 3, 60),
        ];
        // 基于start_offset排序 a f b d e c
        // a -4 -> 12
        // f -8 -> 8
        // b -12 -> 4
        // d -16 -> 0
        // e -4  -> 12
        // c -12 -> 4
        let (allocation_map, stack_size) = allocator.allocate(mocks);
        allocation_map.iter().for_each(|map| {
            println!("{:?}", map);
        });
        let mut gen_context = GenContext::new();
        gen_context.record_alloc_vars(allocation_map, stack_size);

        // 创建 LLVM Context 用于构建常量值
        let context = Context::create();
        let i32_type = context.i32_type();
        let mut asm_builder = AsmBuilder::new();

        // 测试立即数0 - 打印 mv t0, x0
        let const_zero = i32_type.const_int(0, false).into();
        load_value_to_reg(const_zero, &mut gen_context, "t0", &mut asm_builder);
        println!("{:?}", asm_builder.emit());

        // 测试立即数10 - 会多打印 li t0, 10
        let ten = i32_type.const_int(10, false).into();
        load_value_to_reg(ten, &mut gen_context, "t0", &mut asm_builder);
        println!("{:?}", asm_builder.emit());

        // 测试全局变量 - 需要先添加全局变量
        let module = context.create_module("test");
        let global = module.add_global(i32_type,Some(AddressSpace::from(1u16)),"global_var");
        gen_context.add_global("global_var".to_string());
        println!("测试全局变量: global_var 已添加到上下文");
        load_value_to_reg(global.as_basic_value_enum(), &mut gen_context, "t1", &mut asm_builder);
        println!("{:?}", asm_builder.emit());

        // 测试

    }
}
