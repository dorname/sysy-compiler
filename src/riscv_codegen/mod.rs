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
    module::Module,
    values::{FunctionValue, GlobalValue, InstructionValue},
};

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
    stack_size: i32,                          // 总栈大小（从寄存器分配器获得）
    global_names: Vec<String>,                // 全局变量名列表（用于区分全局和局部）
    temp_val_num: usize
}

impl GenContext {
    pub fn new() -> Self {
        Self {
            var_locations: HashMap::new(),
            stack_size: 0,
            global_names: Vec::new(),
            temp_val_num: 0
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
    pub fn record_alloca_vars(&mut self, allocation: HashMap<String, String>, stack_size: i32) {
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
    pub fn get_stack_size(&self) -> i32 {
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
        if !val_name.is_empty() && !uses.is_empty() {
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
        let mut first = HashMap::<String,usize>::new();
        let mut last = HashMap::<String,usize>::new();
        // 求出这个函数作用域最后的指令索引,默认值为0
        // 整个函数作用域最大的活跃区间为max_pos
        let max_pos = self.instructions.iter().map(|(idx,_,_)| idx).max().unwrap_or(&0);

        // 遍历指令集
        for (idx,val_name,uses) in self.instructions.iter() {
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

            // 2.4 收集指令的def（定义）和uses（使用）
            let val_name = get_value_name(&instruction);
            // 收集函数内部使用了哪些变量
            let uses = collect_uses(&instruction,&mut ctx);
            // 保存到函数状态中
            state.record_instruction(idx, val_name, uses);
            state.idx = idx + 1;
        }
    }

    // 步骤3：计算活跃区间
    let inner_vars = state.compute_liveness();
    // 步骤4：执行寄存器分配
    let alloctor = AllocatedInnerVar::default();
    let (allocations, stack_size) = alloctor.allocate(inner_vars, true);
    // 记录变量分配好的存储位置和预计使用的栈空间
    ctx.record_alloca_vars(allocations, stack_size);
  
    

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
    if aligned_stack_size > 0 {
        asm_builder.emit_function_prologue(aligned_stack_size);
    }

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

    // 步骤4：输出汇编代码（调试用）
    println!("{}", asm_builder.emit());
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
fn collect_uses(instruction: &InstructionValue,ctx:&mut GenContext) -> Vec<String> {
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
                uses.push(get_basic_value_name(lhs,ctx));
            }
            if let Some(rhs_operand) = instruction.get_operand(1)
                && let Some(rhs) = rhs_operand.left()
            {
                uses.push(get_basic_value_name(rhs,ctx));
            }
        }
        // load指令的例子： load i32, i32* %0, align 4
        // br指令的例子： br label %target 分支指令
        // zext指令的例子： zext i1 %1 to i32 -> %2
        InstructionOpcode::Load | InstructionOpcode::Br | InstructionOpcode::ZExt => {
            if let Some(value_operand) = instruction.get_operand(0)
                && let Some(value) = value_operand.left()
            {
                uses.push(get_basic_value_name(value,ctx));
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

/// 获取操作数名字
fn get_basic_value_name(value: BasicValueEnum,ctx:&mut GenContext) -> String {
    if value.is_int_value() && value.into_int_value().is_constant_int() {
        return String::new();
    }
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
    _instruction: &InstructionValue,
    _asm_builder: &mut AsmBuilder,
    _ctx: &mut GenContext,
) {
    // TODO: 根据instruction类型生成相应代码
    todo!("Implement instruction code generation")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE_PATH: &str = "tests/lab5/";

    #[test]
    fn test_normaltest1() {
        let file_path = format!("{}{}", FILE_PATH, "normaltest1.sy");
        let out_path = format!("{}{}", FILE_PATH, "normaltest1.s");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let _ = generate_asm(&input, &out_path);
    }

    #[test]
    fn test_example1() {
        let file_path = format!("{}{}", FILE_PATH, "example01.sy");
        let out_path = format!("{}{}", FILE_PATH, "example01.s");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let _ = generate_asm(&input, &out_path);
    }
}
