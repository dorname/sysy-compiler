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

/// 代码生成上下文（两次遍历架构的核心）
/// 保存变量位置映射和总栈大小，在第一遍和第二遍之间传递
pub struct GenContext {
    var_locations: HashMap<String, Location>, // 变量名 -> 存储位置
    stack_size: usize,                        // 总栈大小（从寄存器分配器获得）
    global_names: Vec<String>,                // 全局变量名列表（用于区分全局和局部）
    temp_val_num: usize,
}

impl GenContext {
    pub fn new() -> Self {
        Self {
            var_locations: HashMap::new(),
            stack_size: 0,
            global_names: Vec::new(),
            temp_val_num: 0,
        }
    }

    /// 注册全局变量到上下文
    pub fn add_global(&mut self, name: String) {
        self.var_locations
            .insert(name.clone(), Location::Global(name.clone()));
        self.global_names.push(name);
    }

    /// 查询变量的存储位置
    pub fn get_location(&self, name: &str) -> Option<&Location> {
        self.var_locations.get(name)
    }

    /// 设置寄存器分配结果（从分配器）
    /// 将分配器返回的字符串结果转换为VarLocation
    pub fn record_alloca_vars(&mut self, allocation: HashMap<String, String>, stack_size: usize) {
        self.stack_size = stack_size; // 保存总栈大小

        for (var, loc_str) in allocation {
            // 保护全局变量，不被SSA值覆盖
            if let Some(Location::Global(_)) = self.var_locations.get(&var) {
                continue;
            }

            // 解析位置字符串
            let location = if loc_str.ends_with("(sp)") {
                // 栈位置：如 "16(sp)" -> Mem(16)
                let offset_str = loc_str.trim_end_matches("(sp)");
                let offset: i32 = offset_str.parse().unwrap();
                Location::Stack(offset)
            } else {
                // 寄存器位置：如 "t0" -> Reg("t0")
                Location::Reg(loc_str)
            };
            self.var_locations.insert(var, location);
        }
    }

    /// 获取总栈大小
    pub fn get_stack_size(&self) -> usize {
        self.stack_size
    }

    /// 检查是否为全局变量
    pub fn is_global(&self, name: &str) -> bool {
        self.global_names.contains(&name.to_string())
    }
}

/// 第一次遍历的状态
/// 用于收集指令的def/use信息
struct FunctionState {
    instructions: Vec<(usize, String, Vec<String>)>, // (指令索引,操作数名称，被当作操作数使用的集合)
    allocation_names: HashSet<String>,               // 函数作用域下本地变量集合
    block_start_idxes: HashMap<String, usize>,       // 基本块起始命令的位置
    loop_branch: bool,                               // 循环标志
    idx: usize,                                      // 当前指令索引（指令编号）
}

impl FunctionState {
    /// 创建新的函数状态
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
    let alloctor = AllocatedInnerVar::default();
    let (allocations, stack_size) = alloctor.allocate(inner_vars, false);
    // 记录变量分配好的存储位置和预计使用的栈空间
    ctx.record_alloca_vars(allocations, stack_size);
    
    // 步骤5：为alloca变量（指针）分配栈位置
    // alloca变量本身（指针）需要在栈上分配位置，即使它们没有在活跃区间中
    let mut alloca_stack_offset = ctx.stack_size as i32;
    for alloca_name in state.get_allocation_names() {
        // 如果alloca变量还没有被分配location，为它分配栈位置
        if ctx.get_location(&alloca_name).is_none() {
            ctx.var_locations.insert(alloca_name.clone(), Location::Stack(alloca_stack_offset));
            alloca_stack_offset += 4; // 每个alloca变量占用4字节（指针大小）
        }
    }
    // 更新栈大小以包含alloca变量
    if alloca_stack_offset > ctx.stack_size as i32 {
        ctx.stack_size = alloca_stack_offset as usize;
    }

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
                uses.push(get_basic_value_name(lhs, ctx));
            }
            if let Some(rhs_operand) = instruction.get_operand(1)
                && let Some(rhs) = rhs_operand.left()
            {
                uses.push(get_basic_value_name(rhs, ctx));
            }
        }
        // load指令的例子： load i32, i32* %0, align 4
        // br指令的例子： br label %target 分支指令
        // zext指令的例子： zext i1 %1 to i32 -> %2
        InstructionOpcode::Load | InstructionOpcode::Br | InstructionOpcode::ZExt => {
            if let Some(value_operand) = instruction.get_operand(0)
                && let Some(value) = value_operand.left()
            {
                uses.push(get_basic_value_name(value, ctx));
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

/// 生成 RISC-V return 指令的汇编代码
///
/// ## LLVM IR 指令格式
/// ```
/// ret i32 %value
/// ret void
/// ```
/// - 操作数0 (可选): 返回值（对于有返回值的函数）
/// - 无操作数: 对于 void 函数
///
/// ## 翻译示例
///
/// ### 示例1: 返回立即数
/// ```llvm
/// ; LLVM IR
/// ret i32 42
/// ```
/// 生成汇编：
/// ```asm
/// li   a0, 42          ; 加载返回值到 a0 寄存器
/// addi sp, sp, 16       ; 恢复栈指针（假设栈大小为 16）
/// li   a7, 93          ; 设置系统调用号为 exit
/// ecall                ; 执行系统调用
/// ```
///
/// ### 示例2: 返回寄存器变量
/// ```llvm
/// ; LLVM IR
/// ret i32 %1
/// ```
/// 假设 `%1` 在寄存器 `t3` 中：
/// ```asm
/// mv   a0, t3          ; 将返回值移动到 a0 寄存器
/// addi sp, sp, 16      ; 恢复栈指针
/// li   a7, 93          ; 设置系统调用号
/// ecall                ; 执行系统调用
/// ```
///
/// ### 示例3: 返回栈变量
/// ```llvm
/// ; LLVM IR
/// ret i32 %1
/// ```
/// 假设 `%1` 在栈偏移 `16` 处：
/// ```asm
/// lw   a0, 16(sp)      ; 从栈加载返回值到 a0
/// addi sp, sp, 32       ; 恢复栈指针（假设总栈大小为 32）
/// li   a7, 93          ; 设置系统调用号
/// ecall                ; 执行系统调用
/// ```
///
/// ### 示例4: void 返回
/// ```llvm
/// ; LLVM IR
/// ret void
/// ```
/// 生成汇编：
/// ```asm
/// addi sp, sp, 16      ; 恢复栈指针
/// li   a7, 93          ; 设置系统调用号
/// ecall                ; 执行系统调用
/// ```
///
/// ## 实现思路
///
/// ### 1. 提取返回值（如果有）
/// - 检查指令是否有操作数
/// - 如果有操作数，提取返回值（可能是立即数、寄存器变量或栈变量）
///
/// ### 2. 加载返回值到 a0 寄存器
/// - 使用 `get_value_from_reg()` 将返回值加载到 `a0` 寄存器
/// - 返回值通过 `a0` 寄存器传递给系统调用
///
/// ### 3. 恢复栈指针（函数 epilogue）
/// - 从 `ctx.get_stack_size()` 获取总栈大小
/// - 对齐栈大小到 16 字节
/// - 使用 `emit_function_epilogue()` 恢复栈指针
///
/// ### 4. 执行退出系统调用
/// - 使用 `emit_exit_syscall()` 生成 `li a7, 93; ecall`
/// - 系统调用号 93 对应 exit 系统调用
///
/// ## 寄存器使用
/// - `a0`: 返回值寄存器，传递给系统调用
/// - `a7`: 系统调用号寄存器，设置为 93（exit）
/// - `sp`: 栈指针，需要恢复到函数调用前的状态
///
/// ## 注意事项
/// - RISC-V 中，main 函数不能使用普通的 `ret` 指令
/// - 必须显式调用 exit 系统调用来结束程序
/// - 返回值必须通过 `a0` 寄存器传递
/// - 栈大小必须对齐到 16 字节边界
fn generate_return_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &mut GenContext,
) {
    // ret i32 %a1 或 ret void
    let num_operands = instruction.get_num_operands();
    
    // 处理返回值（如果有）
    if num_operands > 0 {
        if let Some(return_operand) = instruction.get_operand(0)
            && let Some(return_value) = return_operand.left()
        {
            // 将返回值加载到 a0 寄存器
            let return_reg = get_value_from_reg(return_value, ctx, "a0", asm_builder);
            if return_reg != "a0" {
                asm_builder.emit_mv("a0", &return_reg);
            }
        }
    }
    
    // 恢复栈指针（函数 epilogue）
    let stack_size = ctx.get_stack_size();
    let aligned_stack_size = if stack_size > 0 {
        ((stack_size + 15) / 16) * 16
    } else {
        0
    };
    // if aligned_stack_size > 0 {
    //     asm_builder.emit_function_epilogue(aligned_stack_size);
    // }
    asm_builder.emit_function_epilogue(aligned_stack_size);
    // 执行退出系统调用
    asm_builder.emit_exit_syscall();
}

/// 生成 RISC-V load 指令的汇编代码
///
/// ## LLVM IR 指令格式
/// ```
/// %result = load i32, i32* %ptr, align 4
/// ```
/// - 操作数0 (`ptr`): 源指针变量名（来自 alloca 或全局变量）
/// - 结果: 存储加载值的变量名（通过 `get_value_name()` 获取）
///
/// ## 翻译示例
///
/// ### 示例1: 从栈变量加载
/// ```llvm
/// ; LLVM IR
/// %result = load i32, i32* %local_var, align 4
/// ```
/// 假设 `%local_var` 在栈偏移 `16` 处，结果 `%result` 在寄存器 `t5` 中：
/// ```asm
/// lw   t5, 16(sp)       ; 从栈偏移 16 处加载值到 t5
/// ```
///
/// ### 示例2: 从全局变量加载
/// ```llvm
/// ; LLVM IR
/// %result = load i32, i32* @global_var, align 4
/// ```
/// 假设结果 `%result` 在寄存器 `t5` 中：
/// ```asm
/// la   t0, global_var   ; 加载全局变量地址到 t0
/// lw   t5, 0(t0)        ; 从全局变量地址加载值到 t5
/// ```
///
/// ### 示例3: 从寄存器中的指针加载
/// ```llvm
/// ; LLVM IR
/// %result = load i32, i32* %ptr_reg, align 4
/// ```
/// 假设 `%ptr_reg` 在寄存器 `t4` 中（存放内存地址），结果 `%result` 在寄存器 `t5` 中：
/// ```asm
/// lw   t5, 0(t4)        ; 从 t4 指向的地址加载值到 t5
/// ```
///
/// ## 实现思路
///
/// ### 1. 提取操作数
/// - 从指令中提取源指针 (`ptr`)
/// - 获取结果变量名 `result_name`（通过 `get_value_name()`）
///
/// ### 2. 处理源指针
/// 源指针可能有以下几种情况：
/// - **全局变量**: `ctx.is_global(&ptr)` → 使用 `la` + `lw` 加载
/// - **栈变量**: `Location::Stack(offset)` → 直接使用 `lw reg, offset(sp)`
/// - **寄存器中的指针**: `Location::Reg(reg)` → 使用 `lw reg, 0(ptr_reg)`
///
/// ### 3. 获取结果寄存器
/// - 查询 `ctx.get_location(&result_name)` 获取结果的存储位置
/// - 如果结果在寄存器中：`Location::Reg(reg)` → 直接加载到该寄存器
/// - 如果结果在栈中：`Location::Stack(offset)` → 先加载到临时寄存器，再存储到栈
/// - 如果没有分配位置：使用临时寄存器 `t0`
///
/// ### 4. 根据指针类型生成汇编
///
/// #### 情况A: 全局变量
/// 当 `ctx.is_global(&ptr)` 为 `true` 时：
/// 1. 使用 `la t0, ptr` 将全局变量地址加载到临时寄存器 `t0`
/// 2. 使用 `lw result_reg, 0(t0)` 从全局变量地址加载值
///
/// #### 情况B: 栈变量 (`Location::Stack(offset)`)
/// 当源指针是局部变量（alloca）时：
/// - `ptr` 在 `ctx.var_locations` 中的 `Location::Stack(offset)` 表示源在栈上的偏移
/// - 直接使用 `lw result_reg, offset(sp)` 从栈加载值
///
/// #### 情况C: 寄存器中的指针 (`Location::Reg(reg)`)
/// 当指针本身存储在寄存器中时：
/// - 寄存器中存放的是内存地址
/// - 使用 `lw result_reg, 0(ptr_reg)` 从该寄存器指向的地址加载值
///
/// ## 寄存器使用
/// - `t0`: 临时寄存器，用于存放全局变量地址或加载的值
/// - `result_reg`: 结果寄存器，可能是分配的寄存器或临时寄存器
///
/// ## 与 store 指令的对应关系
/// - `load`: 从内存读取 → 寄存器
/// - `store`: 从寄存器写入 → 内存
/// 两者在源/目标位置查询和处理逻辑上是对称的
fn generate_load_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &mut GenContext,
) {
    // %a1 = load i32, i32* %a0, align 4
    let ptr_operand = instruction.get_operand(0);
    let ptr_name = if let Some(ptr_e) = ptr_operand {
        // 尝试用right()获取PointerValue（load指令的指针操作数应该是指针类型）
        if let Some(ptr_value) = ptr_e.right() {
            // 指针类型，直接获取名称
            if let Ok(name_str) = ptr_value.get_name().to_str() {
                name_str.to_string()
            } else {
                return; // 无法获取指针名称，跳过
            }
        } else {
            return; // load指令的指针操作数必须是指针类型，无法获取则跳过
        }
    } else {
        return; // 没有操作数，跳过
    };
    
    let result_name = get_value_name(instruction);
        
    // 获取结果寄存器
    let result_reg = if let Some(result_loc) = ctx.get_location(&result_name) {
        match result_loc {
            Location::Reg(reg) => reg.to_string(),
            _ => "t0".to_string(),
        }
    } else {
        "t0".to_string()
    };
    
    // 根据指针类型处理
    if ctx.is_global(&ptr_name) {
        // 全局变量：la + lw
        asm_builder.emit_la("t0", &ptr_name);
        asm_builder.emit_lw(&result_reg, 0, "t0");
    } else {
        // 栈变量或寄存器中的指针
        let loc_type = ctx.get_location(&ptr_name).map(|loc| match loc {
            Location::Reg(reg) => (Some(reg.clone()), None),
            Location::Stack(sp_offset) => (None, Some(*sp_offset)),
            _ => (None, None),
        });
        
        if let Some((reg_opt, sp_offset_opt)) = loc_type {
            if let Some(ptr_reg) = reg_opt {
                // 从寄存器中的指针加载
                asm_builder.emit_lw(&result_reg, 0, &ptr_reg);
            } else if let Some(sp_offset) = sp_offset_opt {
                // 从栈变量加载（alloca变量）
                // alloca变量（指针）存储在栈偏移sp_offset处
                // 我们需要：
                // 1. 先从栈加载指针值：lw ptr_reg, sp_offset(sp)
                // 2. 然后使用指针值加载数据：lw result_reg, 0(ptr_reg)
                let ptr_reg = "t1";  // 临时寄存器，用于存放指针值
                asm_builder.emit_lw(ptr_reg, sp_offset, "sp");  // 加载指针值
                asm_builder.emit_lw(&result_reg, 0, ptr_reg);   // 使用指针值加载数据
            }
        } else {
            // 如果找不到location，说明该指针变量未被分配
            // 这可能是alloca变量没有在活跃区间中，应该已经在步骤5中分配了栈位置
            // 如果不是alloca变量，尝试从指针值计算地址
            // 这种情况不应该发生在正确的LLVM IR中，但为了健壮性，我们跳过这条指令
        }
    }
    
    // 如果结果在栈中，需要存储
    if let Some(result_loc) = ctx.get_location(&result_name) {
        if let Location::Stack(offset) = result_loc {
            asm_builder.emit_sw(&result_reg, *offset, "sp");
        }
    }
}

/// 生成 RISC-V 分支跳转指令的汇编代码
///
/// ## LLVM IR 指令格式
/// ```
/// br label %target                    ; 无条件跳转
/// br i1 %cond, label %true, label %false  ; 条件跳转
/// ```
/// - 操作数（无条件）: 跳转目标标签
/// - 操作数（条件）: 条件值、true 分支标签、false 分支标签
///
/// ## 翻译示例
///
/// ### 示例1: 无条件跳转
/// ```llvm
/// ; LLVM IR
/// br label %label1
/// ```
/// 生成汇编：
/// ```asm
/// j    label1          ; 无条件跳转到 label1
/// ```
///
/// ### 示例2: 条件跳转（相等）
/// ```llvm
/// ; LLVM IR
/// br i1 %cmp, label %true_block, label %false_block
/// ```
/// 假设 `%cmp` 在寄存器 `t3` 中：
/// ```asm
/// beq  t3, zero, false_block  ; 如果 t3 == 0，跳转到 false_block
/// j    true_block               ; 否则跳转到 true_block
/// ```
/// 或者使用 `bne`：
/// ```asm
/// bne  t3, zero, true_block     ; 如果 t3 != 0，跳转到 true_block
/// j    false_block              ; 否则跳转到 false_block
/// ```
///
/// ### 示例3: 条件跳转（寄存器值）
/// ```llvm
/// ; LLVM IR
/// br i1 %cond, label %if_true, label %if_false
/// ```
/// 假设 `%cond` 在寄存器 `t3` 中：
/// ```asm
/// bne  t3, zero, if_true        ; 如果 t3 != 0（非零即真），跳转到 if_true
/// j    if_false                  ; 否则跳转到 if_false
/// ```
///
/// ## 实现思路
///
/// ### 1. 判断跳转类型
/// - 检查指令操作数数量
/// - 1个操作数：无条件跳转
/// - 3个操作数：条件跳转
///
/// ### 2. 无条件跳转处理
/// - 提取跳转目标标签（基本块名称）
/// - 使用 `emit_j(label)` 生成无条件跳转指令
///
/// ### 3. 条件跳转处理
/// - 提取条件值（第0个操作数）
/// - 提取 true 分支标签（第1个操作数）
/// - 提取 false 分支标签（第2个操作数）
/// - 将条件值加载到寄存器（通过 `get_value_from_reg()`）
/// - 使用条件分支指令：
///   - 如果条件值在寄存器中，使用 `bne cond_reg, zero, true_label` 跳转到 true 分支
///   - 然后使用 `j false_label` 跳转到 false 分支
///   - 或者反过来：先跳转 false，再跳转 true
///
/// ## 寄存器使用
/// - `t0`: 临时寄存器，用于存放条件值（如果需要加载）
///
/// ## 注意事项
/// - 条件值：在 RISC-V 中，非零值表示 true，零值表示 false
/// - 条件分支通常使用 `bne` 或 `beq` 与 `zero` 寄存器比较
/// - 基本块标签已经在第一遍遍历中生成，直接使用即可
fn generate_br_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &mut GenContext,
) {
    // br label %label 或 br i1 %cond, label %true, label %false
    let num_operands = instruction.get_num_operands();
    
    match num_operands {
        1 => {
            // 无条件跳转：br label %target
            if let Some(target_operand) = instruction.get_operand(0)
                && let Some(target) = target_operand.right()
                && let Ok(target_name) = target.get_name().to_str()
            {
                asm_builder.emit_j(target_name);
            }
        }
        3 => {
            // 条件跳转：br i1 %cond, label %true, label %false
            if let Some(cond_operand) = instruction.get_operand(0)
                && let Some(true_operand) = instruction.get_operand(1)
                && let Some(false_operand) = instruction.get_operand(2)
                && let Some(cond) = cond_operand.left()
                && let Some(true_target) = true_operand.right()
                && let Some(false_target) = false_operand.right()
                && let Ok(true_label) = true_target.get_name().to_str()
                && let Ok(false_label) = false_target.get_name().to_str()
            {
                // 加载条件值到寄存器
                let cond_reg = get_value_from_reg(cond, ctx, "t0", asm_builder);
                
                // 在 RISC-V 中，非零值表示 true，零值表示 false
                // 使用 bne 检查条件是否为非零（true）
                asm_builder.emit_bne(&cond_reg, "x0", true_label);
                // 如果条件为 false（零），跳转到 false 分支
                asm_builder.emit_j(false_label);
            }
        }
        _ => {}
    }
}

/// 生成 RISC-V 零扩展指令的汇编代码
///
/// ## LLVM IR 指令格式
/// ```
/// %result = zext i1 %value to i32
/// ```
/// - 操作数0 (`value`): 源值（i1 类型，0 或 1）
/// - 结果: 扩展后的值（i32 类型）
///
/// ## 翻译示例
///
/// ### 示例1: 寄存器变量零扩展
/// ```llvm
/// ; LLVM IR
/// %result = zext i1 %1 to i32
/// ```
/// 假设 `%1` 在寄存器 `t3` 中，结果 `%result` 在寄存器 `t5` 中：
/// ```asm
/// mv   t5, t3          ; 直接移动，因为 i1 已经是 i32 的低位
/// ```
/// 或者如果需要清除高位：
/// ```asm
/// andi t5, t3, 1        ; t5 = t3 & 1（只保留最低位）
/// ```
///
/// ### 示例2: 立即数零扩展
/// ```llvm
/// ; LLVM IR
/// %result = zext i1 1 to i32
/// ```
/// 生成汇编：
/// ```asm
/// li   t5, 1            ; 直接加载立即数 1
/// ```
///
/// ### 示例3: 栈变量零扩展
/// ```llvm
/// ; LLVM IR
/// %result = zext i1 %1 to i32
/// ```
/// 假设 `%1` 在栈偏移 `16` 处，结果 `%result` 在寄存器 `t5` 中：
/// ```asm
/// lw   t0, 16(sp)       ; 从栈加载值到 t0
/// andi t5, t0, 1        ; 零扩展（保留最低位）
/// ```
///
/// ## 实现思路
///
/// ### 1. 提取操作数
/// - 从指令中提取源值 (`value`)
/// - 获取结果变量名 `result_name`（通过 `get_value_name()`）
///
/// ### 2. 获取结果寄存器
/// - 查询 `ctx.get_location(&result_name)` 获取结果的存储位置
/// - 如果结果在寄存器中：`Location::Reg(reg)` → 使用该寄存器
/// - 如果结果在栈中：`Location::Stack(offset)` → 使用临时寄存器 `t0`，然后存储到栈
/// - 如果没有分配位置：使用临时寄存器 `t0`
///
/// ### 3. 处理源值
/// 源值可能有以下几种情况：
/// - **立即数常量**: 直接加载到结果寄存器
/// - **寄存器变量**: `Location::Reg(reg)` → 使用 `mv` 或 `andi` 扩展
/// - **栈变量**: `Location::Stack(offset)` → 先加载到临时寄存器，再扩展
/// - **全局变量**: `Location::Global(name)` → 先加载到临时寄存器，再扩展
///
/// ### 4. 零扩展实现
/// - 在 RISC-V 中，零扩展通常不需要特殊处理，因为寄存器已经足够大
/// - 如果需要确保只保留最低位，可以使用 `andi result_reg, src_reg, 1`
/// - 对于立即数，直接使用 `li` 加载即可
///
/// ## 寄存器使用
/// - `t0`: 临时寄存器，用于存放源值（如果需要加载）
/// - `result_reg`: 结果寄存器，可能是分配的寄存器或临时寄存器
///
/// ## 注意事项
/// - i1 类型在 RISC-V 中通常已经是 i32 的低位
/// - 零扩展主要是为了类型转换，通常不需要额外的位操作
/// - 如果源值已经是 0 或 1，直接移动即可
fn generate_zext_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &mut GenContext,
) {
    // %a1 = zext i1 %a0 to i32
    let src_operand = instruction.get_operand(0);
    if let Some(src_e) = src_operand
        && let Some(src) = src_e.left()
    {
        let result_name = get_value_name(instruction);
        
        // 获取结果寄存器
        let result_reg = if let Some(result_loc) = ctx.get_location(&result_name) {
            match result_loc {
                Location::Reg(reg) => reg.to_string(),
                Location::Stack(sp_offset) => {
                    // 如果结果在栈中，先计算到临时寄存器，再存储
                    let temp_reg = "t0";
                    let offset = *sp_offset;
                    let src_reg = get_value_from_reg(src, ctx, temp_reg, asm_builder);
                    // 零扩展：在 RISC-V 中，i1 值已经是 0 或 1，直接移动即可
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
        
        // 加载源值
        let src_reg = get_value_from_reg(src, ctx, "t1", asm_builder);
        
        // 零扩展：在 RISC-V 中，i1 值已经是 0 或 1，直接移动即可
        if src_reg != result_reg {
            // 移动到结果寄存器
            asm_builder.emit_mv(&result_reg, &src_reg);
        }
    }
}

/// 生成 RISC-V 算术运算指令的汇编代码
///
/// ## LLVM IR 指令格式
/// ```
/// %result = add i32 %lhs, %rhs
/// %result = sub i32 %lhs, %rhs
/// %result = mul i32 %lhs, %rhs
/// %result = sdiv i32 %lhs, %rhs
/// %result = srem i32 %lhs, %rhs
/// ```
/// - 操作数0 (`lhs`): 左操作数（可以是立即数、变量或临时值）
/// - 操作数1 (`rhs`): 右操作数（可以是立即数、变量或临时值）
/// - 结果: 存储运算结果的变量名（通过 `get_value_name()` 获取）
///
/// ## 翻译示例
///
/// ### 示例1: 加法运算（两个寄存器变量）
/// ```llvm
/// ; LLVM IR
/// %add = add i32 %1, %2
/// ```
/// 假设 `%1` 在寄存器 `t3` 中，`%2` 在寄存器 `t4` 中，结果 `%add` 在寄存器 `t5` 中：
/// ```asm
/// add t5, t3, t4       ; t5 = t3 + t4
/// ```
///
/// ### 示例2: 减法运算（一个寄存器变量，一个立即数）
/// ```llvm
/// ; LLVM IR
/// %sub = sub i32 %1, 10
/// ```
/// 假设 `%1` 在寄存器 `t3` 中，结果 `%sub` 在寄存器 `t5` 中：
/// ```asm
/// li   t0, 10          ; 加载立即数 10 到 t0
/// sub  t5, t3, t0      ; t5 = t3 - 10
/// ```
/// 或者使用立即数减法指令（如果支持）：
/// ```asm
/// subi t5, t3, 10      ; t5 = t3 - 10
/// ```
///
/// ### 示例3: 乘法运算（栈变量）
/// ```llvm
/// ; LLVM IR
/// %mul = mul i32 %1, %2
/// ```
/// 假设 `%1` 在栈偏移 `16` 处，`%2` 在栈偏移 `20` 处，结果 `%mul` 在寄存器 `t5` 中：
/// ```asm
/// lw   t0, 16(sp)      ; 加载 %1 到 t0
/// lw   t1, 20(sp)       ; 加载 %2 到 t1
/// mul  t5, t0, t1       ; t5 = t0 * t1
/// ```
///
/// ### 示例4: 除法运算（有符号除法）
/// ```llvm
/// ; LLVM IR
/// %div = sdiv i32 %1, %2
/// ```
/// 假设 `%1` 在寄存器 `t3` 中，`%2` 在寄存器 `t4` 中，结果 `%div` 在寄存器 `t5` 中：
/// ```asm
/// div  t5, t3, t4       ; t5 = t3 / t4 (有符号除法)
/// ```
///
/// ### 示例5: 取余运算（有符号取余）
/// ```llvm
/// ; LLVM IR
/// %rem = srem i32 %1, %2
/// ```
/// 假设 `%1` 在寄存器 `t3` 中，`%2` 在寄存器 `t4` 中，结果 `%rem` 在寄存器 `t5` 中：
/// ```asm
/// rem  t5, t3, t4       ; t5 = t3 % t4 (有符号取余)
/// ```
///
/// ## 实现思路
///
/// ### 1. 提取操作数
/// - 从指令中提取左操作数 (`lhs`) 和右操作数 (`rhs`)
/// - 获取结果变量名 `result_name`（通过 `get_value_name()`）
///
/// ### 2. 处理操作数
/// 操作数可能有以下几种情况：
/// - **立即数常量**: 使用 `get_value_from_reg()` 加载到临时寄存器（如 `t0` 或 `t1`）
/// - **寄存器变量**: `Location::Reg(reg)` → 直接使用该寄存器
/// - **栈变量**: `Location::Stack(offset)` → 使用 `lw` 加载到寄存器
/// - **全局变量**: `Location::Global(name)` → 使用 `la` + `lw` 加载
///
/// ### 3. 获取结果寄存器
/// - 查询 `ctx.get_location(&result_name)` 获取结果的存储位置
/// - 如果结果在寄存器中：`Location::Reg(reg)` → 使用该寄存器
/// - 如果结果在栈中：`Location::Stack(offset)` → 使用临时寄存器 `t2`，然后存储到栈
/// - 如果没有分配位置（临时值）：使用临时寄存器 `t2`
///
/// ### 4. 根据指令类型生成汇编
///
/// #### 情况A: 加法 (Add)
/// 1. 将左操作数加载到临时寄存器 `t0`（通过 `get_value_from_reg()`）
/// 2. 将右操作数加载到临时寄存器 `t1`（通过 `get_value_from_reg()`）
/// 3. 使用 `add result_reg, t0, t1` 生成加法指令
/// 4. 如果结果在栈中，使用 `sw result_reg, offset(sp)` 存储结果
///
/// #### 情况B: 减法 (Sub)
/// 1. 将左操作数加载到临时寄存器 `t0`
/// 2. 将右操作数加载到临时寄存器 `t1`
/// 3. 使用 `sub result_reg, t0, t1` 生成减法指令
/// 4. 如果结果是立即数且支持 `subi`，可以直接使用 `subi result_reg, t0, imm`
///
/// #### 情况C: 乘法 (Mul)
/// 1. 将左操作数加载到临时寄存器 `t0`
/// 2. 将右操作数加载到临时寄存器 `t1`
/// 3. 使用 `mul result_reg, t0, t1` 生成乘法指令
///
/// #### 情况D: 有符号除法 (SDiv)
/// 1. 将左操作数加载到临时寄存器 `t0`
/// 2. 将右操作数加载到临时寄存器 `t1`
/// 3. 使用 `div result_reg, t0, t1` 生成除法指令（RISC-V 的 `div` 是有符号除法）
///
/// #### 情况E: 有符号取余 (SRem)
/// 1. 将左操作数加载到临时寄存器 `t0`
/// 2. 将右操作数加载到临时寄存器 `t1`
/// 3. 使用 `rem result_reg, t0, t1` 生成取余指令（RISC-V 的 `rem` 是有符号取余）
///
/// ## 寄存器使用
/// - `t0`: 临时寄存器，用于存放左操作数
/// - `t1`: 临时寄存器，用于存放右操作数
/// - `t2`: 临时寄存器，用于存放结果（当结果未分配到寄存器时）
///
/// ## 注意事项
/// - 所有操作数在处理前都需要加载到寄存器中
/// - 如果操作数是立即数，需要先通过 `li` 加载到寄存器
/// - 结果寄存器优先使用分配好的寄存器，其次使用临时寄存器 `t2`
/// - 如果结果在栈中，需要先将结果计算到寄存器，然后存储到栈上指定偏移位置
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
                    // 结果在栈上：先计算到临时寄存器 t2，再存储到栈
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
                    // 将计算结果存储到栈上
                    asm_builder.emit_sw(result_reg, *sp_offset, "sp");
                },
                Location::Global(name) => {
                    // 结果在全局变量：先计算到临时寄存器 t2，再存储到全局变量
                    let result_reg = "t2";
                    let addr_reg = "t3";  // 用于存放全局变量地址
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
                    // 加载全局变量地址到 addr_reg
                    asm_builder.emit_la(addr_reg, &name);
                    // 将计算结果存储到全局变量地址
                    asm_builder.emit_sw(result_reg, 0, addr_reg);
                },
                _ => {}
            }
        }
    
    }

}


/// 生成 RISC-V store 指令的汇编代码
///
/// ## LLVM IR 指令格式
/// ```
/// store i32 %source_value, i32* %target_ptr, align 4
/// ```
/// - 第0个操作数 (`from`): 要存储的源值（可以是立即数、变量或临时值）
/// - 第1个操作数 (`to`): 目标指针变量名（来自 alloca 或全局变量）
///
/// ## 翻译示例
///
/// ### 示例1: 存储到全局变量
/// ```llvm
/// ; LLVM IR
/// store i32 42, i32* @global_var, align 4
/// ```
/// 生成汇编：
/// ```asm
/// li   t0, 42          ; 加载立即数 42 到 t0
/// la   t2, global_var  ; 加载全局变量地址到 t2
/// sw   t0, 0(t2)       ; 将 t0 的值存储到 [t2 + 0]
/// ```
///
/// ### 示例2: 存储到栈变量（alloca）
/// ```llvm
/// ; LLVM IR
/// %local_var = alloca i32, align 4
/// store i32 %1, i32* %local_var, align 4
/// ```
/// 假设 `%1` 在寄存器 `t3` 中，`%local_var` 在栈偏移 `16` 处：
/// ```asm
/// mv   t0, t3          ; 将 %1 的值移动到 t0（如果源值已经在寄存器中）
/// sw   t0, 16(sp)      ; 将 t0 的值存储到栈偏移 16 处
/// ```
///
/// 如果 `%1` 是立即数：
/// ```asm
/// li   t0, 100         ; 加载立即数到 t0
/// sw   t0, 16(sp)      ; 存储到栈
/// ```
///
/// ### 示例3: 存储到寄存器中的指针
/// ```llvm
/// ; LLVM IR
/// store i32 %1, i32* %ptr_reg, align 4
/// ```
/// 假设 `%ptr_reg` 在寄存器 `t4` 中（存放内存地址），`%1` 的值在 `t3` 中：
/// 代码会调用 `load_value_to_reg()` 将源值加载到目标寄存器：
/// ```asm
/// mv   t4, t3           ; 将 %1 的值移动到指针寄存器 t4（如果源值在 t3 中）
/// ```
/// 或者如果 `%1` 是立即数：
/// ```asm
/// li   t4, 100          ; 将立即数 100 加载到指针寄存器 t4
/// ```
/// 注意：当前实现将值加载到指针寄存器本身，而非存储到该寄存器指向的内存地址
///
/// ## 实现思路
///
/// ### 1. 提取操作数
/// - 从指令中提取源值 (`from`) 和目标指针 (`to`)
/// - 获取目标指针的名称 `ptr`
///
/// ### 2. 处理源值
/// 源值可能有以下几种情况：
/// - **立即数常量**: 使用 `get_value_from_reg()` 加载到寄存器（如 `t0`）
/// - **寄存器变量**: `Location::Reg(reg)` → 直接使用该寄存器
/// - **栈变量**: `Location::Stack(offset)` → 使用 `lw` 加载到寄存器
/// - **全局变量**: `Location::Global(name)` → 使用 `la` + `lw` 加载
///
/// ### 3. 根据目标指针类型生成指令
///
/// #### 情况A: 全局变量
/// 当 `ctx.is_global(&ptr)` 为 `true` 时：
/// 1. 将源值加载到临时寄存器 `t0`（通过 `get_value_from_reg()`）
/// 2. 使用 `la t2, ptr` 将全局变量地址加载到寄存器 `t2`
/// 3. 使用 `sw t0, 0(t2)` 将源值存储到全局变量地址
///
/// #### 情况B: 栈变量 (`Location::Stack(offset)`)
/// 当目标指针是局部变量（alloca）时：
/// - `ptr` 在 `ctx.var_locations` 中的 `Location::Stack(offset)` 表示目标在栈上的偏移
/// - 实现步骤：
///   1. 将源值加载到临时寄存器 `t0`（通过 `get_value_from_reg()`）
///   2. 直接使用 `sw t0, offset(sp)` 存储到栈上指定偏移位置
///
/// #### 情况C: 寄存器中的指针 (`Location::Reg(reg)`)
/// 当指针本身存储在寄存器中时（较少见的情况）：
/// - 寄存器中存放的是内存地址
/// - 使用 `load_value_to_reg()` 将源值加载到该寄存器指向的地址
/// - 注意：这种情况下寄存器存储的是地址值，需要间接存储
///
/// ## 寄存器使用
/// - `t0`: 临时寄存器，用于存放源值
/// - `t2`: 临时寄存器，用于存放全局变量地址（仅在全局变量情况下使用）
///
/// ## 与 load 指令的对应关系
/// - `load`: 从内存读取 → 寄存器
/// - `store`: 从寄存器写入 → 内存
/// 两者在目标位置查询和处理逻辑上是对称的
fn generate_store_instruction(
    instruction: &InstructionValue,
    asm_builder: &mut AsmBuilder,
    ctx: &mut GenContext,
) {
    // store i32 %a1, i32* %a0, align 4
    let from_operand = instruction.get_operand(0);
    let to_operand = instruction.get_operand(1);
    let (from, ptr_name) = if let Some(from_e) = from_operand
        && let Some(to_e) = to_operand
        && let Some(from) = from_e.left()
    {
        // 获取指针操作数：尝试用right()获取PointerValue（store指令的指针操作数应该是指针类型）
        let ptr_name = if let Some(ptr_value) = to_e.right() {
            // 指针类型，直接获取名称
            if let Ok(name_str) = ptr_value.get_name().to_str() {
                name_str.to_string()
            } else {
                return; // 无法获取指针名称，跳过
            }
        } else if let Some(to) = to_e.left() {
            // 如果不是指针类型，尝试用left()获取BasicValueEnum
            get_basic_value_name(to, ctx)
        } else {
            return; // 无法获取操作数，跳过
        };
        (from, ptr_name)
    } else {
        return; // 没有操作数，跳过
    };
    
    if ctx.is_global(&ptr_name) {
        // lw t0 ()sp / la t0 a1 lw t0 0(t0)
        let from_reg = get_value_from_reg(from,ctx,"t0",asm_builder);
        // la t2 a0 把 a0 地址读到寄存器
        asm_builder.emit_la("t2",&ptr_name);
        // sw t0 0(t2) 把寄存器 t0 中的值，存入内存地址 [t2 + 0] 处。
        asm_builder.emit_sw(&from_reg,0,"t2");
    } else {
        // 存储到寄存器/或者栈中
        let loc_type = ctx.get_location(&ptr_name).map(|loc| match loc {
            Location::Reg(reg) => (Some(reg.clone()), None),
            Location::Stack(sp_offset) => (None, Some(*sp_offset)),
            _ => (None, None),
        });
        
        if let Some((reg_opt, sp_offset_opt)) = loc_type {
            if let Some(reg) = reg_opt {
                load_value_to_reg(from,ctx,&reg,asm_builder);
            } else if let Some(sp_offset) = sp_offset_opt {
                // 存储到栈变量（alloca变量）
                // alloca变量（指针）存储在栈偏移sp_offset处
                // 我们需要：
                // 1. 先从栈加载指针值：lw ptr_reg, sp_offset(sp)
                // 2. 然后将源值存储到指针指向的位置：sw from_reg, 0(ptr_reg)
                let ptr_reg = "t2";  // 临时寄存器，用于存放指针值
                asm_builder.emit_lw(ptr_reg, sp_offset, "sp");  // 加载指针值
                let from_reg = get_value_from_reg(from,ctx,"t0",asm_builder);
                asm_builder.emit_sw(&from_reg, 0, ptr_reg);      // 使用指针值存储数据
            }
        }
        // 如果找不到location，说明该指针变量未被分配，这不应该发生在正确的LLVM IR中
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
                // 将数据从栈内存中取出并加载到寄存器中
                asm_builder.emit_lw(reg_name,*sp_offset,&format!("{}(sp)",sp_offset));
                return reg_name.to_string();
            },
            Location::Global(name) => {
                // la 负责拿"地址"，lw 负责拿"内容"。
                // 获取全局变量地址
                // la t0 a
                asm_builder.emit_la(reg_name,name);
                // lw t0, 0(t0) - 使用寄存器作为基址，而不是符号名
                asm_builder.emit_lw(reg_name,0,reg_name);
                return reg_name.to_string();
            }
        }
    }
    "x0".to_string()
}


fn load_value_to_reg(input: BasicValueEnum, ctx: &mut GenContext,reg_name:&str,asm_builder: &mut AsmBuilder) {
    // 判断是否是立即数
    // 如果是0则返回zero寄存器
    // 不是则分配到入参的寄存器中
    if is_constant(input)
        && let Some(val) = input.into_int_value().get_zero_extended_constant(){
        if val == 0 {
            asm_builder.emit_mv(reg_name,"x0");
            return;
        }
        asm_builder.emit_li(reg_name,val as i32);
        return;
    }

    //不是立即数而
    let reg_key =  get_basic_value_name(input, ctx);
    if reg_key.starts_with("tmp_") {
        asm_builder.emit_mv(reg_name,"x0");
        return;
    }
    if let Some(location) = ctx.get_location(&reg_key) {
        match location {
            Location::Reg(reg) => {
                if reg != reg_name {
                    asm_builder.emit_mv(reg_name,reg);
                }
            },
            Location::Stack(sp_offset) => {
                // 将数据从栈内存中取出并加载到寄存器中
                asm_builder.emit_lw(reg_name,*sp_offset,"sp");
            },
            Location::Global(label) => {
                // 获取全局变量地址
                // la t0 a  把 a 的地址 存放到寄存器t0
                asm_builder.emit_la(reg_name,label);
                // lw t0 t0 把寄存器地址对应的值取出来放到t0
                asm_builder.emit_lw(reg_name,0,reg_name);
            }
        }
    }else {
        asm_builder.emit_mv(reg_name,"x0");
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
    fn test_part2() {
        let file_path = format!("{}{}", FILE_PATH, "part2.sy");
        let out_path = format!("{}{}", FILE_PATH, "part2.s");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let _ = generate_asm(&input, &out_path);
    }

}
