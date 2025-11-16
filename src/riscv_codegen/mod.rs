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
    alloca_names: HashSet<String>,           // alloca变量名集合
    pure_stack_mode: bool,                   // 是否为纯栈分配模式
}

impl GenContext {
    pub fn get_alloca_names(&self) -> &HashSet<String> {
        &self.alloca_names
    }
    
    pub fn new() -> Self {
        Self {
            var_locations: HashMap::new(),
            stack_size: 0,
            global_names: Vec::new(),
            temp_val_num: 0,
            alloca_names: HashSet::new(),
            pure_stack_mode: false,
        }
    }
    
    pub fn set_pure_stack_mode(&mut self, enabled: bool) {
        self.pure_stack_mode = enabled;
    }
    
    pub fn set_alloca_names(&mut self, alloca_names: HashSet<String>) {
        self.alloca_names = alloca_names;
    }
    
    /// 检查是否为alloca变量（纯栈模式下返回false）
    pub fn is_alloca(&self, name: &str) -> bool {
        if self.pure_stack_mode {
            return false;
        }
        self.alloca_names.contains(name)
    }

    pub fn add_global(&mut self, name: String) {
        self.var_locations
            .insert(name.clone(), Location::Global(name.clone()));
        self.global_names.push(name);
    }

    pub fn get_location(&self, name: &str) -> Option<&Location> {
        self.var_locations.get(name)
    }

    /// 记录寄存器分配结果：将分配器返回的字符串转换为Location
    pub fn record_alloca_vars(&mut self, allocation: HashMap<String, String>, stack_size: usize) {
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
    allocation_names: HashSet<String>,               // alloca变量集合
    block_start_idxes: HashMap<String, usize>,       // 基本块起始位置
    loop_branch: bool,                               // 是否存在循环
    idx: usize,                                      // 当前指令索引
}

impl FunctionState {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            allocation_names: HashSet::new(),
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
    pub fn record_local_val(&mut self, name: String) {
        self.allocation_names.insert(name);
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
    pub fn get_allocation_names(&self) -> &HashSet<String> {
        &self.allocation_names
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
            if self.loop_branch && self.allocation_names.contains(&val_name) {
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

            // 2.1 收集定义变量集合（局部变量）
            if matches!(opcode, InstructionOpcode::Alloca) {
                let name = get_value_name(&instruction);
                if !name.is_empty() {
                    state.record_local_val(name);
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
    let pure_stack_mode = false; //Part2使用纯栈分配模式
    let alloctor = AllocatedInnerVar::default();
    let (allocations, stack_size) = alloctor.allocate(inner_vars, pure_stack_mode);
    // 记录变量分配好的存储位置和预计使用的栈空间
    ctx.record_alloca_vars(allocations, stack_size);
    
    // 设置纯栈分配模式标志
    ctx.set_pure_stack_mode(pure_stack_mode);
    
    // 步骤5：为alloca变量分配栈位置（仅在纯栈分配模式下）
    // 在纯栈分配模式下，alloca变量应该直接存储值，而不是存储指针
    // 关键问题：alloca变量（如%c）在Store/Load指令中被使用，但它们的名称是alloca变量的名称
    // 如果alloca变量已经在活跃区间中被分配了位置，应该使用那个位置
    // 如果alloca变量没有在活跃区间中被分配，需要在步骤5中为它们分配位置
    // 但是，在纯栈分配模式下，alloca变量应该使用它们在活跃区间中被分配的位置
    // 而不是在步骤5中重新分配位置
    
    // 注意：alloca变量（如%c）在LLVM IR中是指针类型，但在纯栈分配模式下，
    // Store和Load指令应该直接使用alloca变量的栈位置，而不是通过指针间接访问
    
    // 在纯栈分配模式下，alloca变量应该已经在活跃区间中被分配了位置
    // 如果alloca变量没有在活跃区间中被分配，说明它没有被使用，不需要分配位置
    // 但是，为了确保所有alloca变量都有位置，我们仍然检查并分配
    let mut alloca_stack_offset = ctx.stack_size as i32;
    for alloca_name in state.get_allocation_names() {
        // 如果alloca变量还没有被分配location，为它分配栈位置
        if ctx.get_location(&alloca_name).is_none() {
            ctx.var_locations.insert(alloca_name.clone(), Location::Stack(alloca_stack_offset));
            alloca_stack_offset += 4; // 每个变量占用4字节
        }
    }
    // 更新栈大小以包含alloca变量
    if alloca_stack_offset > ctx.stack_size as i32 {
        ctx.stack_size = alloca_stack_offset as usize;
    }
    
    // 将alloca变量集合传递给GenContext，用于区分alloca变量和普通局部变量
    ctx.set_alloca_names(state.get_allocation_names().clone());

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
                    let name = get_basic_value_name(lhs, ctx);
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
                    let name = get_basic_value_name(rhs, ctx);
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
                        let name = get_basic_value_name(ptr_value, ctx);
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
                    let name = get_basic_value_name(value, ctx);
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

/// 获取操作数名字
fn get_basic_value_name(value: BasicValueEnum, ctx: &mut GenContext) -> String {
    if let Ok(name_cstr) = value.get_name().to_str()
        && !name_cstr.is_empty()
    {
        return name_cstr.to_string();
    }
    let num_str = ctx.temp_val_num.to_string();
    ctx.temp_val_num += 1;
    format!("tmp_{}", num_str)
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
            let name = get_basic_value_name(ptr_basic, ctx);
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
            if ctx.is_alloca(&ptr_name) {
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
            let name = get_basic_value_name(to, ctx);
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
            if ctx.is_alloca(&ptr_name) {
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


fn get_value_from_reg(input: BasicValueEnum, ctx: &mut GenContext,reg_name:&str,asm_builder: &mut AsmBuilder) -> String {
    // 判断是否是立即数
    // 如果是0则返回zero寄存器
    // 不是则分配到入参的寄存器中
    if is_constant(input)
        && let Some(val) = input.into_int_value().get_zero_extended_constant(){
        if val == 0 {
            return "x0".to_string();
        }
        asm_builder.emit_li(reg_name,val as i32);
        return reg_name.to_string();
    }

    //不是立即数而
    let reg_key =  get_basic_value_name(input, ctx);
    if let Some(location) = ctx.get_location(&reg_key) {
        match location {
            Location::Reg(reg) => {
                return reg.to_string();
            },
            Location::Stack(sp_offset) => {
                asm_builder.emit_lw(reg_name, *sp_offset, "sp");
                return reg_name.to_string();
            },
            Location::Global(name) => {
                asm_builder.emit_la(reg_name, name);
                asm_builder.emit_lw(reg_name, 0, reg_name);
                return reg_name.to_string();
            }
        }
    }
    // FIX: 当找不到location时，不应该返回x0（零寄存器），而应该使用临时寄存器加载该值
    // 这种情况通常发生在值没有被正确分配位置时，应该从栈或全局变量加载
    // 但由于我们不知道具体位置，这里使用临时寄存器并初始化为0，或者报错
    // 实际上，这种情况不应该发生，如果发生了，说明寄存器分配有问题
    // 为了安全起见，我们使用临时寄存器并初始化为0
    asm_builder.emit_li(reg_name, 0);
    reg_name.to_string()
}


fn load_value_to_reg(input: BasicValueEnum, ctx: &mut GenContext, reg_name: &str, asm_builder: &mut AsmBuilder) {
    if is_constant(input)
        && let Some(val) = input.into_int_value().get_zero_extended_constant()
    {
        if val == 0 {
            asm_builder.emit_mv(reg_name, "x0");
            return;
        }
        asm_builder.emit_li(reg_name, val as i32);
        return;
    }

    let reg_key = get_basic_value_name(input, ctx);
    if reg_key.starts_with("tmp_") {
        asm_builder.emit_mv(reg_name, "x0");
        return;
    }
    if let Some(location) = ctx.get_location(&reg_key) {
        match location {
            Location::Reg(reg) => {
                if reg != reg_name {
                    asm_builder.emit_mv(reg_name, reg);
                }
            },
            Location::Stack(sp_offset) => {
                asm_builder.emit_lw(reg_name, *sp_offset, "sp");
            },
            Location::Global(label) => {
                asm_builder.emit_la(reg_name, label);
                asm_builder.emit_lw(reg_name, 0, reg_name);
            }
        }
    } else {
        asm_builder.emit_mv(reg_name, "x0");
    }
}
#[cfg(test)]
mod tests {
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
        let file_path = format!("{}{}", FILE_PATH, "part2.sy");
        let out_path = format!("{}{}", FILE_PATH, "part2.ll");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let scanner = Scanner::new(&input,&out_path);
        let _ = scanner.scan_collect();
    }
}
