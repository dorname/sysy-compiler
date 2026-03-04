// RISC-V 代码生成模块
// 负责将LLVM IR翻译为RISC-V汇编代码

use std::collections::{HashMap, HashSet};
use std::process::Output;
use tklog::{trace,debug, error, fatal, info,warn};
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
    let _ = scanner.scan_collect_asm("temp.ll",|ir_session, output| Ok(parse_llvm_ir(ir_session, output)));
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
            if !val_name.is_empty()  {
                // 记录第一次出现的位置，entry方法是如果hash表中为空才会插入，否则不插入
                first.entry(val_name.to_string()).or_insert(*idx);
                // 记录最后一次出现的位置
                last.insert(val_name.to_string(), *idx);
            }
            // 开始根据被使用的位置信息，更新活跃区间
            for use_name in uses {
                if !use_name.is_empty(){
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
            let uses = collect_uses(&instruction);

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

    // allocations.iter().for_each(|map| {
    //     println!("{:?}", map);
    // });
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
fn collect_uses(instruction: &InstructionValue) -> Vec<String> {
    let mut uses = Vec::new();
    let opcode = instruction.get_opcode();
    match opcode {
        // 收集算数指令的使用地址
        InstructionOpcode::ICmp
        | InstructionOpcode::Store
        | InstructionOpcode::Add
        | InstructionOpcode::Sub
        | InstructionOpcode::Mul
        | InstructionOpcode::SDiv
        | InstructionOpcode::SRem => {
            // 收集<op1>
            if let Some(lhs_operand) = instruction.get_operand(0)
                && let Some(lhs) = lhs_operand.left()
            {
                if !is_constant(lhs) {
                    let name = get_value_from_basic(lhs);
                    uses.push(name);
                }
            }
            // 收集<op2>
            if let Some(rhs_operand) = instruction.get_operand(1)
                && let Some(rhs) = rhs_operand.left()
            {
                if !is_constant(rhs) {
                    let name = get_value_from_basic(rhs);
                    uses.push(name);

                }
            }
        }
        InstructionOpcode::Load => {
            // <pointer>
            if let Some(value_operand) = instruction.get_operand(0)
            && let Some(ptr_value) = value_operand.left() {
                if !is_constant(ptr_value) {
                    let name = get_value_from_basic(ptr_value);
                    uses.push(name);
                }
            }

        }
        InstructionOpcode::Br | InstructionOpcode::ZExt => {
            if let Some(value_operand) = instruction.get_operand(0)
                && let Some(value) = value_operand.left()
            {
                if !is_constant(value) {
                    let name = get_value_from_basic(value);
                    uses.push(name);
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
            generate_icmp_instruction(instruction, asm_builder, ctx);
        }
        InstructionOpcode::Store => {
            generate_store_instruction(instruction, asm_builder, ctx);
        }
        InstructionOpcode::Add
        | InstructionOpcode::Sub
        | InstructionOpcode::Mul
        | InstructionOpcode::SDiv
        | InstructionOpcode::SRem => {
            generate_cal_instruction(instruction, asm_builder, ctx);
        }
        InstructionOpcode::Load => {
            generate_load_instruction(instruction, asm_builder, ctx);
        }
        InstructionOpcode::Br => {
            generate_br_instruction(instruction, asm_builder, ctx);
        }
        InstructionOpcode::ZExt => {
            generate_zext_instruction(instruction, asm_builder, ctx);
        }
        InstructionOpcode::Return => {
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

/// 生成 load 指令的 RISC-V 汇编代码
///
/// 该函数负责将 LLVM IR 中的 load 指令转换为相应的 RISC-V 汇编指令。
/// load 指令用于从指针指向的内存位置读取值，并将其存储到目标位置（寄存器或栈）。
///
/// # 参数
///
/// * `instruction` - SSA 形式的 load 指令，包含：
///   - 操作数 0: 指针操作数，指向要读取的内存地址
/// * `asm_builder` - 汇编代码构建器，用于生成汇编指令
/// * `ctx` - 代码生成上下文，包含变量位置信息和全局变量表
///
/// # 支持的场景
///
/// ## 1. 从全局变量加载
///
/// 对于 LLVM IR：
/// ```llvm
/// @x = global i32 56
/// %val = load i32, i32* @x, align 4
/// ```
///
/// - **结果存储在寄存器**：使用 `la` 加载全局变量地址，再用 `lw` 读取值
/// - **结果存储在栈**：先加载到临时寄存器 `t0`，再用 `sw` 存入栈
///
/// ## 2. 从局部变量加载
///
/// 对于 LLVM IR：
/// ```llvm
/// %a = alloca i32, align 4
/// store i32 1, i32* %a, align 4
/// %val = load i32, i32* %a, align 4
/// ```
///
/// 支持四种位置组合：
///
/// | 来源位置 | 目标位置 | 操作 |
/// |----------|----------|------|
/// | 栈 (sp + offset) | 寄存器 | `lw reg, offset(sp)` |
/// | 栈 (sp + offset1) | 栈 (sp + offset2) | `lw t0, offset1(sp)` + `sw t0, offset2(sp)` |
/// | 寄存器 | 寄存器 | `mv dst, src` |
/// | 寄存器 | 栈 (sp + offset) | `sw reg, offset(sp)` |
///
/// # 实现细节
///
/// - **指针解析**：从 load 指令中提取指针操作数的名称
/// - **全局变量处理**：优先检查指针是否指向全局变量
/// - **局部变量处理**：根据来源和目标的位置类型生成相应的数据移动指令
/// - **优化**：当来源和目标位置相同时，跳过不必要的数据移动
/// - **临时寄存器**：使用 `t0` 作为临时寄存器进行中间值传递
///
/// # 语义说明
///
/// `%val = load i32, i32* %a` 表示：从 `%a` 指向的内存读取一个 32 位整数，
/// 并生成新的 SSA 值 `%val`。
fn generate_load_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &GenContext,
) {
    let ptr_operand = instruction.get_operand(0);
    let ptr_name = if let Some(ptr_e) = ptr_operand
    && let Some(ptr_basic) = ptr_e.left()
    && let Ok(p) = ptr_basic.get_name().to_str(){
        p.to_string()
    }else {
        return;
    };
    
    let result_name = get_value_name(instruction);

    // <pointer> 是全局变量
    if ctx.is_global(&ptr_name) {
        match ctx.get_location(&result_name) {
            Some(Location::Reg(reg)) => {
                // 把ptr_name的地址存进寄存器reg
                asm_builder.emit_la(reg, &ptr_name);
                // 然后根据存进reg的地址读取内容存回寄存器reg
                asm_builder.emit_lw(reg, 0, reg);
            }
            Some(Location::Stack(offset)) => {
                // 把ptr_name的地址存进寄存器t0
                asm_builder.emit_la("t0", &ptr_name);
                // 然后根据存进t0的地址读取内容存回寄存器t0
                asm_builder.emit_lw("t0", 0, "t0");
                // 将t0存的值存到栈偏移offset的位置
                asm_builder.emit_sw("t0", *offset, "sp");
            }
            _ => {
                return;
            }
        }
        return;
    } else {
        // 读取目标存储位置
        let result_loc = ctx.get_location(&result_name);
        // 读取来源存储位置
        let ptr_loc = ctx.get_location(&ptr_name);
        match (result_loc, ptr_loc) {
            // reg <- sp
            (Some(Location::Reg(to_reg)), Some(Location::Stack(from_offset))) => {
                asm_builder.emit_lw(to_reg,*from_offset,"sp")
            },
            // sp <- sp
            (Some(Location::Stack(to_offset)), Some(Location::Stack(from_offset))) => {
                if to_offset == from_offset {
                    return;
                }
                asm_builder.emit_lw("t0",*from_offset,"sp");
                asm_builder.emit_sw("t0",*to_offset,"sp");
            },
            // reg <- reg
            (Some(Location::Reg(to_reg)), Some(Location::Reg(from_reg))) => {
                if to_reg == from_reg {
                    return;
                }
                asm_builder.emit_mv(to_reg, from_reg);
            }
            // sp <- reg
            (Some(Location::Stack(to_offset)), Some(Location::Reg(from_reg))) => {
                asm_builder.emit_sw(from_reg,*to_offset,"sp");
            }
            _ => {}

        }
    }
}

/// 生成分支（br）指令的 RISC-V 汇编代码
///
/// 该函数负责将 LLVM IR 中的分支指令转换为相应的 RISC-V 跳转指令。
/// 支持无条件跳转和条件跳转两种形式，根据操作数数量自动识别分支类型。
///
/// # 参数
///
/// * `instruction` - SSA 形式的 br 指令，操作数数量决定分支类型：
///   - 1 个操作数：无条件分支，包含目标基本块
///   - 3 个操作数：条件分支，包含条件值、true 分支和 false 分支
/// * `asm_builder` - 汇编代码构建器，用于生成汇编指令
/// * `ctx` - 代码生成上下文，包含变量位置信息
///
/// # 支持的分支类型
///
/// ## 1. 无条件分支（1 个操作数）
///
/// 对于 LLVM IR：
/// ```llvm
/// br label %target
/// ```
///
/// 生成的 RISC-V 汇编：
/// ```asm
/// j target
/// ```
///
/// **行为**：直接跳转到目标基本块标签
///
/// ## 2. 条件分支（3 个操作数）
///
/// 对于 LLVM IR：
/// ```llvm
/// %cond = icmp eq i32 %a, %b
/// br i1 %cond, label %true_bb, label %false_bb
/// ```
///
/// 生成的 RISC-V 汇编：
/// ```asm
/// # 假设 %cond 的值在 t0 中
/// bne t0, x0, true_bb    # 如果条件为真（非零），跳转到 true_bb
/// j false_bb             # 否则跳转到 false_bb
/// ```
///
/// **行为**：根据条件值决定跳转目标
///
/// # 实现细节
///
/// - **条件值获取**：使用 `get_value_from_reg` 将条件值加载到临时寄存器 `t0`
/// - **条件判断逻辑**：
///   - 使用 `bne` (branch not equal) 指令与零寄存器 `x0` 比较
///   - 如果条件值非零（真），跳转到 true 分支
///   - 如果条件值为零（假），执行 `j` 指令跳转到 false 分支
/// - **分支优化**：先判断 true 分支，false 分支作为默认路径
///
/// # 注意事项
///
/// - 条件分支中的条件值在 LLVM IR 中为 `i1` 类型（布尔值）
/// - RISC-V 中用整数表示布尔值：`1` 表示真，`0` 表示假
/// - 使用 `x0`（零寄存器）作为比较基准，简化条件判断逻辑
fn generate_br_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &GenContext,
) {
    let num_operands = instruction.get_num_operands();
    match num_operands {
        // 无条件分支
        1 => {
            if let Some(target_operand) = instruction.get_operand(0)
                && let Some(target) = target_operand.right()
                && let Ok(target_name) = target.get_name().to_str()
            {
                // 跳转到目标分支
                asm_builder.emit_j(target_name);
            }
        }
        // 条件分支
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
                // 如果条件为1 jump 到 true 分支
                asm_builder.emit_bne(&cond_reg, "x0", true_label);
                // 否则 jump 到 false 分支
                asm_builder.emit_j(false_label);
            }
        }
        _ => {}
    }
}

/// 生成零扩展（zext）指令的 RISC-V 汇编代码
///
/// 该函数负责将 LLVM IR 中的零扩展指令转换为相应的 RISC-V 汇编指令。
/// 零扩展用于将较小位宽的整数类型转换为较大位宽的整数类型，扩展的高位填充为 0。
///
/// # 参数
///
/// * `instruction` - SSA 形式的 zext 指令，包含：
///   - 操作数 0: 要进行零扩展的源值
/// * `asm_builder` - 汇编代码构建器，用于生成汇编指令
/// * `ctx` - 可变的代码生成上下文，包含变量位置信息
///
/// # 零扩展说明
///
/// zext（Zero Extension）指令将较小的整数类型扩展为较大的整数类型，
/// 新增的高位全部填充为 0，常用于无符号整数的类型转换。
///
/// ## LLVM IR 示例
///
/// ```llvm
/// ; 将 i1 类型扩展为 i32 类型
/// %result = zext i1 %cond to i32
///
/// ; 将 i8 类型扩展为 i32 类型
/// %wide = zext i8 %byte to i32
/// ```
///
/// # 实现细节
///
/// ## RISC-V 中的零扩展
///
/// RISC-V 是 32 位架构，寄存器本身就是 32 位。由于 LLVM IR 中的小整数类型
/// （如 `i1`、`i8`、`i16`）在 RISC-V 中都以 32 位整数形式存储，因此零扩展
/// 操作在大多数情况下不需要额外的指令，只需要将值从源位置移动到目标位置。
///
/// ## 处理流程
///
/// 1. **获取源值**：使用 `get_value_from_reg` 将源值加载到寄存器（默认 `t0`）
/// 2. **确定目标位置**：查询结果值在上下文中的存储位置
/// 3. **生成移动指令**：
///    - **目标为寄存器**：使用 `mv` 指令将值移动到目标寄存器
///    - **目标为栈**：使用 `sw` 指令将值存储到栈的指定偏移位置
///
/// ## 优化
///
/// - **避免冗余移动**：如果源寄存器和目标寄存器相同，跳过 `mv` 指令
///
/// # 生成的汇编示例
///
/// 对于结果存储在寄存器 `a0` 的情况：
/// ```asm
/// mv a0, t0
/// ```
///
/// 对于结果存储在栈偏移 8 的情况：
/// ```asm
/// sw t0, 8(sp)
/// ```
fn generate_zext_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &mut GenContext,
) {
    let val_operand = instruction.get_operand(0);
    if let Some(value) = val_operand
        && let Some(val) = value.left()
    {
        // 获取val的存储位置
        let value_reg = get_value_from_reg(val, ctx, "t0", asm_builder);

        // 判断目标存储类型
        let result_name = get_value_name(instruction);
        let result_loc = ctx.get_location(&result_name);
        match result_loc {
            Some(Location::Reg(reg)) => {
                if &value_reg == reg {
                    return;
                }
                asm_builder.emit_mv(reg,&value_reg);
            },
            Some(Location::Stack(offset)) => {
                asm_builder.emit_sw(&value_reg,*offset,"sp");
            },
            _ => {}
        }
    }
}

/// 生成算术运算指令的 RISC-V 汇编代码
///
/// 该函数负责将 LLVM IR 中的二元算术运算指令转换为相应的 RISC-V 汇编指令。
/// 支持五种基本算术运算，并根据结果存储位置的不同（寄存器、栈或全局变量）
/// 生成相应的汇编代码。
///
/// # 参数
///
/// * `instruction` - SSA 形式的算术运算指令，包含：
///   - 操作数 0: 左操作数（可能是 SSA 值或常量）
///   - 操作数 1: 右操作数（可能是 SSA 值或常量）
///   - 操作码: 指定运算类型（Add、Sub、Mul、SDiv、SRem）
/// * `asm_builder` - 汇编代码构建器，用于生成汇编指令
/// * `ctx` - 代码生成上下文，包含变量位置信息
///
/// # 支持的算术操作
///
/// | 操作码 | 含义 | RISC-V 指令 |
/// |--------|------|-------------|
/// | `Add` | 加法 (+) | `add rd, rs1, rs2` |
/// | `Sub` | 减法 (-) | `sub rd, rs1, rs2` |
/// | `Mul` | 乘法 (*) | `mul rd, rs1, rs2` |
/// | `SDiv` | 有符号除法 (/) | `div rd, rs1, rs2` |
/// | `SRem` | 有符号取余 (%) | `rem rd, rs1, rs2` |
///
/// # 实现细节
///
/// ## 寄存器使用
///
/// - **临时寄存器**：
///   - `t0`: 存储左操作数
///   - `t1`: 存储右操作数
///   - `t2`: 当结果存储在栈或全局变量时使用
///   - `t3`: 当结果存储在全局变量时，用于存储全局变量地址
///
/// ## 结果存储策略
///
/// 根据结果在上下文中的位置类型，采用不同的存储策略：
///
/// 1. **存储在寄存器** (`Location::Reg`):
///    - 直接将运算结果存入分配的寄存器
///
/// 2. **存储在栈** (`Location::Stack`):
///    - 先将运算结果存入临时寄存器 `t2`
///    - 使用 `sw` 指令将结果写入栈的指定偏移位置
///
/// 3. **存储在全局变量** (`Location::Global`):
///    - 先将运算结果存入临时寄存器 `t2`
///    - 使用 `la` 指令将全局变量地址加载到 `t3`
///    - 使用 `sw` 指令将结果写入全局变量
///
/// # 示例
///
/// 对于 LLVM IR 指令：
/// ```llvm
/// %result = add i32 %a, %b
/// ```
///
/// 如果 `%result` 分配在寄存器 `a0` 中，生成的汇编代码可能为：
/// ```asm
/// add a0, t0, t1
/// ```
///
/// 如果 `%result` 存储在栈上偏移 8 的位置，生成的汇编代码可能为：
/// ```asm
/// add t2, t0, t1
/// sw t2, 8(sp)
/// ```
fn generate_cal_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &GenContext,
) {
    info!(instruction.to_string());
    let lhs_operand = instruction.get_operand(0);
    let rhs_operand = instruction.get_operand(1);
    if let Some(lhs_e) = lhs_operand
    && let Some(rhs_e) = rhs_operand
    && let Some(lhs) = lhs_e.left()
    && let Some(rhs) = rhs_e.left() {
        let lhs_reg = get_value_from_reg(lhs,ctx,"t0",asm_builder);
        let rhs_reg = get_value_from_reg(rhs,ctx,"t1",asm_builder);
        info!("左侧：",lhs.to_string(),";右侧：",rhs.to_string());
        let result_name = get_value_name(instruction);
        info!("结果名称",result_name);
        // 获取结果存储位置
        let result_loc = ctx.get_location(&result_name);
        if let Some(result_loc) = result_loc {
            info!("结果存储位置",result_loc.get_name());
            match result_loc {
                // 存储在寄存器
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
                // 存储在栈
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
                //存储在全局变量
                Location::Global(name) => {
                    info!("全局变量",name);
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


/// 生成store指令：把 SSA value / 常量 写入某个内存地址
/// ```
/// LLVM STORE 指令语法
/// store [volatile] <ty> <value>, ptr <pointer>[, align <alignment>][, !nontemporal !<nontemp_node>][, !invariant.group !<empty_node>]        ; yields void
/// store atomic [volatile] <ty> <value>, ptr <pointer> [syncscope("<target-scope>")] <ordering>, align <alignment> [, !invariant.group !<empty_node>] ; yields void
/// !<nontemp_node> = !{ i32 1 }
/// !<empty_node> = !{}
/// 例如：store i32 %a1, i32* %a0, align 4
/// store value, ptr %p 是 把 value 存到 %p 指向的内存
/// ```
/// 生成 store 指令的 RISC-V 汇编代码
///
/// 该函数负责将一个值存储到指定的内存地址。根据目标地址的类型（全局变量或局部变量）
/// 以及存储位置（寄存器或栈）的不同，生成相应的汇编指令。
///
/// # 参数
///
/// * `instruction` - SSA 形式的 store 指令，包含两个操作数：
///   - 操作数 0: 要存储的值（可能是 SSA 值或常量）
///   - 操作数 1: 目标内存地址（指针）
/// * `asm_builder` - 汇编代码构建器，用于生成汇编指令
/// * `ctx` - 代码生成上下文，包含变量位置信息和全局变量表
///
/// # 行为
///
/// 函数根据目标地址的不同类型采取不同的处理策略：
///
/// 1. **全局变量存储**：
///    - 将值加载到临时寄存器 `t0`
///    - 使用 `la` 指令将全局变量地址加载到 `t2`
///    - 使用 `sw` 指令将值写入全局变量
///
/// 2. **局部变量存储**：
///    - **存储在寄存器中**: 直接将值加载到目标寄存器
///    - **存储在栈上**: 将值加载到临时寄存器，然后使用 `sw` 指令写入栈
///
/// # 注意
///
/// - 如果无法解析指令的操作数或变量名，函数会提前返回
/// - 使用临时寄存器 `t0` 和 `t2` 进行中间值的处理
fn generate_store_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &GenContext,
) {
    // 解析来源SSA
    let from_operand = instruction.get_operand(0);
    // 解析内存地址存放处
    let to_operand = instruction.get_operand(1);

    let (from, ptr_name) = if let Some(from_e) = from_operand
        && let Some(to_e) = to_operand
        && let Some(from) = from_e.left()
        && let Some(to) = to_e.left()
    {
        let ptr_name = if let Ok(p) = to.get_name().to_str(){
            p
        } else {
            return;
        };
        (from, ptr_name.to_string())
    } else {
        return;
    };

    // 如果 pointer 是全局变量的内存地址
    // 则把 value 从ssa/const 取出来
    if ctx.is_global(&ptr_name) {
        let value = get_value_from_reg(from,ctx,"t0",asm_builder);
        // 加载全局变量地址到t2寄存器
        asm_builder.emit_la("t2",&ptr_name);
        // 将value 写入 全局变量
        asm_builder.emit_sw(&value,0,"t2");
        return ;
    }

    let ptr_s = {
        ctx.get_location(&ptr_name)
    };

    // 如果pointer 是局部变量的内存地址（可能指向的是栈空间也可能是寄存器）
    if let Some(ptr) = ptr_s {
        match ptr {
            Location::Reg(reg) => {
                load_value_to_reg(from,ctx,reg,asm_builder);
            },
            Location::Stack(sp_offset) => {
                let value_reg = get_value_from_reg(from,ctx,"t0",asm_builder);
                asm_builder.emit_sw(&value_reg,*sp_offset,"sp")
            },
            _=>{},
        }
    }
}

/// 生成整数比较（icmp）指令的 RISC-V 汇编代码
///
/// 该函数负责将 LLVM IR 中的整数比较指令转换为相应的 RISC-V 汇编指令。
/// 支持六种基本的整数比较操作，并将比较结果（0 或 1）存储到目标寄存器中。
///
/// # 参数
///
/// * `instruction` - SSA 形式的 icmp 指令，包含：
///   - 操作数 0: 左操作数（可能是 SSA 值或常量）
///   - 操作数 1: 右操作数（可能是 SSA 值或常量）
///   - 比较谓词: 指定比较类型（EQ、NE、SGT、SLT、SLE、SGE）
/// * `asm_builder` - 汇编代码构建器，用于生成汇编指令
/// * `ctx` - 代码生成上下文，包含变量位置信息
///
/// # 支持的比较操作
///
/// | 谓词 | 含义 | 生成的指令 |
/// |------|------|------------|
/// | `EQ` | 等于 (==) | `sub` + `seqz` |
/// | `NE` | 不等于 (!=) | `sub` + `snez` |
/// | `SGT` | 有符号大于 (>) | `sgt` (伪指令) |
/// | `SLT` | 有符号小于 (<) | `slt` |
/// | `SLE` | 有符号小于等于 (<=) | `sle` (伪指令: `slt` + `xori`) |
/// | `SGE` | 有符号大于等于 (>=) | `sge` (伪指令) |
///
/// # 实现细节
///
/// - **临时寄存器使用**：
///   - `t0`: 存储左操作数
///   - `t1`: 存储右操作数
///   - `t2`: 作为默认结果寄存器（如果结果没有分配到其他寄存器）
///
/// - **结果存储**：
///   - 优先使用上下文中为结果分配的寄存器
///   - 如果结果位置不是寄存器或未分配，则使用 `t2` 作为默认寄存器
///
/// - **比较结果**：所有比较操作的结果为布尔值，以整数形式存储：
///   - `1`: 比较条件为真
///   - `0`: 比较条件为假
///
/// # 示例
///
/// 对于 LLVM IR 指令：
/// ```llvm
/// %cmp = icmp sgt i32 %a, 3
/// ```
///
/// 生成的 RISC-V 汇编代码可能为：
/// ```asm
/// li t1, 3
/// sgt t2, a0, t1
/// ```
fn generate_icmp_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &GenContext,
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

                // 先做减法，然后与立即数0比较 sub rd rs2
                asm_builder.emit_sub(&result_reg,&l_reg,&r_reg);
                // seqz(set if equal zero) 如果 t2 寄存器中的值为0 则往 result_reg 寄存器中写入1 否则 写入 0 seqz rd rs1
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
                // snez (set if not equal zero) 如果 t2 寄存器中的值 不为 0 则往result_reg 寄存器中写入1 否则 写入 0 snez rd rs1
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
                // asm 支持伪指令
                // sgt rd, rs1, rs2
                asm_builder.emit_sgt(&result_reg,&l_reg,&r_reg);
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
                // slt rd, rs1, rs2
                asm_builder.emit_slt(&result_reg,&l_reg,&r_reg);
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
                // 伪指令
                // sle rd,rs1,rs2
                // 实际：
                // slt rd, rs2, rs1   # rd = (rs2 < rs1) ? 1 : 0
                // xori rd, rd, 1     # rd = rd ^ 1  => 取反，得到 (rs1 <= rs2)
                asm_builder.emit_sle(&result_reg,&l_reg,&r_reg);
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
                // 伪指令
                // sgt rd, rs1, rs2
                asm_builder.emit_sge(&result_reg,&l_reg,&r_reg);
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
fn get_value_from_reg(input: BasicValueEnum, ctx: &GenContext,reg_name:&str,asm_builder: &mut AsmBuilder) -> String {
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
fn load_value_to_reg(input: BasicValueEnum, ctx: &GenContext, reg_name: &str, asm_builder: &mut AsmBuilder) {
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
    use std::arch::asm;
    use inkwell::AddressSpace;
    use inkwell::values::BasicValue;
    use crate::log_init;
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
        log_init();
        let file_path = format!("{}{}", FILE_PATH, "part3.sy");
        let out_path = format!("{}{}", FILE_PATH, "part3.s");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let _ = generate_asm(&input, &out_path);
    }

    #[test]
    fn  test_part4() {
        let file_path = format!("{}{}", FILE_PATH, "part4.sy");
        let out_path = format!("{}{}", FILE_PATH, "part4.s");
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
        // allocation_map.iter().for_each(|map| {
        //     println!("{:?}", map);
        // });
        let mut gen_context = GenContext::new();
        gen_context.record_alloc_vars(allocation_map, stack_size);
        // println!("{:?}",gen_context.var_locations);
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
        let mut only_stack_alloc = NoAlloc::default();
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
        let (allocation_map, stack_size) = allocator.allocate(mocks.clone());
        allocation_map.iter().for_each(|map| {
            println!("{:?}", map);
        });
        let mut gen_context = GenContext::new();
        gen_context.record_alloc_vars(allocation_map, stack_size);

        // 纯栈分配
        let (stack_allocation_map, stack_size) = only_stack_alloc.allocate(mocks);
        stack_allocation_map.iter().for_each(|map| {
            println!("{:?}", map);
        });
        let mut stack_gen_context = GenContext::new();
        stack_gen_context.record_alloc_vars(stack_allocation_map, stack_size);

        // 创建 LLVM Context 用于构建常量值
        let context = Context::create();
        let i32_type = context.i32_type();
        let mut asm_builder = AsmBuilder::new();

        // 纯栈分配类型
        let stack_ctx = Context::create();
        let i32_type_s = context.i32_type();

        // 测试立即数0 - 应该返回 "x0"
        let const_zero = i32_type.const_int(0, false).into();
        let reg_zero = get_value_from_reg(const_zero, &mut gen_context, "t0", &mut asm_builder);
        // 立即数0不会产生汇编
        assert_eq!(reg_zero, "x0");

        // 测试立即数1 - 应该生成 li t0, 1 并返回 "t0"
        let const_one = i32_type.const_int(1, false).into();
        let _ = get_value_from_reg(const_one, &mut gen_context, "t0", &mut asm_builder);
        
        // 测试全局变量 - 需要先添加全局变量
        let module = context.create_module("test");
        let global = module.add_global(i32_type,Some(AddressSpace::from(1u16)),"global_var");
        gen_context.add_global("global_var".to_string());
        let _ = get_value_from_reg(global.as_basic_value_enum(), &mut gen_context, "t1", &mut asm_builder);

        // 测试栈变量 - 从分配结果中找到栈变量
        let stack_builder = stack_ctx.create_builder();
        let stack_module = stack_ctx.create_module("stack_test");
        let void_type = stack_ctx.void_type().fn_type(&[],false);
        let func = stack_module.add_function("test_fn", void_type, None);
        let entry = stack_ctx.append_basic_block(func, "entry");
        stack_builder.position_at_end(entry);
        if let Ok(stack_test) =  stack_builder.build_alloca(i32_type_s,"f"){
            let _ = get_value_from_reg(stack_test.as_basic_value_enum(),&mut stack_gen_context,"t1",&mut asm_builder);
        }

        // 测试寄存器变量 - 从分配结果中找到寄存器变量
        let builder = context.create_builder();
        let void_type = context.void_type().fn_type(&[],false);
        let func = module.add_function("test_fn", void_type, None);
        let entry = context.append_basic_block(func, "entry");
        builder.position_at_end(entry);
        if let Ok(local_var_test) =  builder.build_alloca(i32_type,"f"){
            let var_reg = get_value_from_reg(local_var_test.as_basic_value_enum(),&mut gen_context,"t1",&mut asm_builder);
            // 线性扫描算法给f分配的的t4寄存器
            assert_eq!(var_reg, "t4");
        }
        // 测试未知变量 - 验证未知变量不在上下文中
        if let Ok(unknown_test) =  builder.build_alloca(i32_type,"fb"){
            let var_reg = get_value_from_reg(unknown_test.as_basic_value_enum(),&mut gen_context,"t1",&mut asm_builder);
            // fb 不存在 所以返回t1
            assert_eq!(var_reg, "t1");
        }
        // 打印生成的汇编代码以验证
        let asm_code = asm_builder.emit();
        println!("\n生成的汇编代码:\n{}", asm_code);
        // 验证汇编代码包含预期的指令
        // 验证立即数1 li t0 1
        assert!(asm_code.contains("li t0, 1"), "汇编代码应该包含 li t0, 1 指令");
        // 验证全局变量 la t1 global_var lw t1 0(t1)
        assert!(asm_code.contains("la t1, global_var"), "汇编代码应该包含 la t1 global_var 指令");
        assert!(asm_code.contains("lw t1, 0(t1)"), "汇编代码应该包含 lw t1 0(t1) 指令");

        // 验证从栈中取值到寄存器 lw t1, 8(sp)
        assert!(asm_code.contains("lw t1, 8(sp)"), "汇编代码应该包含 lw t1, 8(sp) 指令");

        // 验证未知变量存储寄存器 li t1, 0
        assert!(asm_code.contains("li t1, 0"),"汇编代码应该包含 lw t1, 0 指令");
        println!("\n所有测试通过！");
    }

    #[test]
    fn test_load_val_to_reg(){
        use inkwell::context::Context;

        let mut allocator = LinearScan::new();
        let mut only_stack_alloc = NoAlloc::default();

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
        let (allocation_map, stack_size) = allocator.allocate(mocks.clone());
        allocation_map.iter().for_each(|map| {
            println!("{:?}", map);
        });

        // 纯栈分配
        let (stack_allocation_map, stack_size) = only_stack_alloc.allocate(mocks);
        stack_allocation_map.iter().for_each(|map| {
            println!("{:?}", map);
        });

        let mut gen_context = GenContext::new();
        gen_context.record_alloc_vars(allocation_map, stack_size);

        let mut stack_gen_context = GenContext::new();
        stack_gen_context.record_alloc_vars(stack_allocation_map, stack_size);

        // 创建 LLVM Context 用于构建常量值
        let context = Context::create();
        let i32_type = context.i32_type();
        let mut asm_builder = AsmBuilder::new();

        // 纯栈分配类型
        let stack_ctx = Context::create();
        let i32_type_s = context.i32_type();


        // 测试立即数0 - 打印 mv t0, x0
        let const_zero = i32_type.const_int(0, false).into();
        load_value_to_reg(const_zero, &mut gen_context, "t0", &mut asm_builder);


        // 测试立即数10 - 会多打印 li t0, 10
        let ten = i32_type.const_int(10, false).into();
        load_value_to_reg(ten, &mut gen_context, "t0", &mut asm_builder);


        // 测试全局变量 - 需要先添加全局变量
        let module = context.create_module("test");
        let global = module.add_global(i32_type,Some(AddressSpace::from(1u16)),"global_var");
        gen_context.add_global("global_var".to_string());
        load_value_to_reg(global.as_basic_value_enum(), &mut gen_context, "t1", &mut asm_builder);

        // 测试栈变量 - 从分配结果中找到栈变量
        let stack_builder = stack_ctx.create_builder();
        let stack_module = stack_ctx.create_module("stack_test");
        let void_type = stack_ctx.void_type().fn_type(&[],false);
        let func = stack_module.add_function("test_fn", void_type, None);
        let entry = stack_ctx.append_basic_block(func, "entry");
        stack_builder.position_at_end(entry);
        if let Ok(stack_test) =  stack_builder.build_alloca(i32_type_s,"f"){
             load_value_to_reg(stack_test.as_basic_value_enum(),&mut stack_gen_context,"t1",&mut asm_builder);
        }

        // 测试寄存器变量 - 从分配结果中找到寄存器变量
        let builder = context.create_builder();
        let void_type = context.void_type().fn_type(&[],false);
        let func = module.add_function("test_fn", void_type, None);
        let entry = context.append_basic_block(func, "entry");
        builder.position_at_end(entry);
        if let Ok(local_var_test) =  builder.build_alloca(i32_type,"f"){
            load_value_to_reg(local_var_test.as_basic_value_enum(),&mut gen_context,"t1",&mut asm_builder);
        }

        // 测试未知变量 - 验证未知变量不在上下文中
        if let Ok(unknown_test) =  builder.build_alloca(i32_type,"fb"){
            load_value_to_reg(unknown_test.as_basic_value_enum(),&mut gen_context,"t1",&mut asm_builder);
        }

        let asm_code = asm_builder.emit();
        println!("\n生成的汇编代码:\n{}", asm_code);
        // 验证加载立即数0
        assert!(asm_code.contains("mv t0, x0"),"汇编需要包含 mv t0, x0指令");
        // 验证立即数10
        assert!(asm_code.contains("li t0, 10"),"汇编需要包含 li t0, 10指令");
        // 验证全局变量
        assert!(asm_code.contains("la t1, global_var"),"汇编需要包含 la t1, global_var指令");
        assert!(asm_code.contains("lw t1, 0(t1)"),"汇编需要包含 lw t1, 0(t1)指令");

        // 验证把值从栈加载到寄存器
        assert!(asm_code.contains("lw t1, 8(sp)"),"汇编需要包含 lw t1, 8(sp)指令");

        // 验证把值从寄存器加载到另一个寄存器
        assert!(asm_code.contains("mv t1, t4"),"汇编需要包含 mv t1, t4指令");

        // 验证未知变量加载到另一个寄存器
        assert!(asm_code.contains("mv t1, x0"),"汇编需要包含 mv t1, x0指令");

    }
}
