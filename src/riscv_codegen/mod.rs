// RISC-V 代码生成模块
// 负责将LLVM IR翻译为RISC-V汇编代码

use std::collections::{HashMap, HashSet};
use std::process::Output;

use inkwell::{module::Module, values::{FunctionValue, GlobalValue, InstructionValue}};
use inkwell::values::InstructionOpcode;
use crate::{gen_llvm_ir::{Scanner, IrSession}, riscv_codegen::asm_builder::AsmBuilder};
pub mod asm_builder;
pub mod register_alloc;

pub fn generate_asm(input: &str,output: &str) -> Result<(), String> {
    let scanner = Scanner::new(input, output);
    let _ = scanner.scan_collect_asm(|ir_session,output| {
        Ok(parse_llvm_ir(ir_session,output))
    });
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
pub fn parse_llvm_ir<'ctx>(ir_session: &IrSession<'ctx>,output: &str) {
    let module:&Module<'ctx> = &ir_session.module;
    // 初始化汇编构建器
    let mut asm_builder = AsmBuilder::new();

    // 提取全局变量
    let global_variables = module.get_globals();
    for global_variable in global_variables {
        build_global_variable(&global_variable, &mut asm_builder);
    }

    // 依次提取函数，并构建汇编
    let functions = module.get_functions();
    for function in functions {
        build_function(&function, &mut asm_builder);
    }

    // 获取并将结果写output 文件
    let result = asm_builder.emit();
    std::fs::write(output, result).unwrap();
}


/// 变量存储位置枚举
/// 统一表示变量在寄存器、栈或全局变量中的位置
#[derive(Debug, Clone)]
enum VarLocation {
    Reg(String),   // 寄存器位置（如 "t0", "s0"）
    Mem(i32),      // 栈偏移量
    Glb(String),   // 全局变量标签
}

/// 代码生成上下文（两次遍历架构的核心）
/// 保存变量位置映射和总栈大小，在第一遍和第二遍之间传递
struct GenContext {
    var_locations: HashMap<String, VarLocation>,  // 变量名 -> 存储位置
    stack_size: i32,                              // 总栈大小（从寄存器分配器获得）
    global_names: Vec<String>,                     // 全局变量名列表（用于区分全局和局部）
}

impl GenContext {
    pub fn new() -> Self {
        Self {
            var_locations: HashMap::new(),
            stack_size: 0,
            global_names: Vec::new(),
        }
    }
    
    /// 注册全局变量到上下文
    pub fn add_global(&mut self, name: String) {
        self.var_locations.insert(name.clone(), VarLocation::Glb(name.clone()));
        self.global_names.push(name);
    }
    
    /// 查询变量的存储位置
    pub fn get_location(&self, name: &str) -> Option<&VarLocation> {
        self.var_locations.get(name)
    }
    
    /// 设置寄存器分配结果（从分配器）
    /// 将分配器返回的字符串结果转换为VarLocation
    pub fn alloca_vars(&mut self, allocation: HashMap<String, String>, stack_size: i32) {
        self.stack_size = stack_size;  // 保存总栈大小
        
        for (var, loc_str) in allocation {
            // 保护全局变量，不被SSA值覆盖
            if let Some(VarLocation::Glb(_)) = self.var_locations.get(&var) {
                continue;
            }
            
            // 解析位置字符串
            let location = if loc_str.ends_with("(sp)") {
                // 栈位置：如 "16(sp)" -> Mem(16)
                let offset_str = loc_str.trim_end_matches("(sp)");
                let offset: i32 = offset_str.parse().unwrap();
                VarLocation::Mem(offset)
            } else {
                // 寄存器位置：如 "t0" -> Reg("t0")
                VarLocation::Reg(loc_str)
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
    instructions: Vec<(usize, String, Vec<String>)>,  // (idx, def, uses)
    alloca_names: HashSet<String>,                           // alloca变量集合
    block_start_idxs: HashMap<String, usize>,            // 基本块起始命令的位置
    loop_branch: bool,                              // 循环标志
    idx: usize,                                              // 当前指令索引
}

impl FunctionState {
    /// 创建新的函数状态
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            alloca_names: HashSet::new(),
            block_start_idxs: HashMap::new(),
            loop_branch: false,
            idx: 0,
        }
    }

    /// 添加指令的def/use信息 (idx, def, uses)
    pub fn record_instruction(&mut self, idx: usize, def: String, uses: Vec<String>) {
        self.instructions.push((idx, def, uses));
        self.idx = idx + 1;
    }

    /// 记录alloca变量名
    pub fn record_alloca(&mut self, name: String) {
        self.alloca_names.insert(name);
    }

    /// 记录基本块起始索引
    pub fn record_block_start(&mut self, block_name: String, idx: usize) {
        self.block_start_idxs.insert(block_name, idx);
    }

    /// 标记存在循环
    pub fn mark_loop(&mut self) {
        self.loop_branch = true;
    }

    /// 获取访问器
    pub fn has_loop(&self) -> bool { self.loop_branch }
    pub fn get_alloca_names(&self) -> &HashSet<String> { &self.alloca_names }
    pub fn get_block_idx(&self, block_name: &str) -> Option<usize> {
        self.block_start_idxs.get(block_name).copied()
    }
    pub fn current_idx(&self) -> usize { self.idx }
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
    && let Some(val) = init_value.into_int_value().get_zero_extended_constant(){
        asm_builder.emit_word(val as i32);
    } else {
        asm_builder.emit_word(0);
    }
    // 4、起一个新行
    asm_builder.emit_empty_line();
}

/// 构建函数的汇编代码
/// 采用两次遍历架构：先分析后生成
fn build_function<'ctx>(function: &FunctionValue<'ctx>, asm_builder: &mut AsmBuilder) {
    // 1、获取函数名和全局变量名
    let function_name = function.get_name().to_str().unwrap();
    
    // TODO: 从模块中收集全局变量名
    let _global_names: Vec<String> = Vec::new();  // 需要从module中获取
    
    // 2、初始化GenContext
    let mut ctx = GenContext::new();
    
    // ==========================================
    // 第一次遍历：分析阶段
    // ==========================================
    let mut state = FunctionState::new();
    
    // 步骤1：建立基本块到指令索引的映射（用于循环检测）
    // 记录每个基本块从第几条指令开始
    let mut temp_idx = 0;
    for basic_block in function.get_basic_blocks() {
        if let Ok(block_name) = basic_block.get_name().to_str() 
        && !block_name.is_empty() {
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
            
            // 2.1 收集alloca变量（局部变量）
            if matches!(opcode, InstructionOpcode::Alloca) {
                let name = get_value_name(&instruction);
                if !name.is_empty() {
                    state.record_alloca(name);
                }
            }
            
            // 2.2 检测循环（backward branch）
            if matches!(opcode, InstructionOpcode::Br) {
                if check_backward_branch(&instruction, &block_start_idx, &state.block_start_idxs) {
                    state.mark_loop();
                }
            }
            
            // 2.3 跳过不产生值的指令（Return、Br）
            if matches!(opcode, InstructionOpcode::Return | InstructionOpcode::Br) {
                state.idx += 1;
                continue;
            }
            
            // 2.4 收集指令的def（定义）和uses（使用）
            let def = get_value_name(&instruction);
            let uses = collect_uses(&instruction);
            state.record_instruction(idx, def, uses);
        }
    }
    
    // 步骤3：计算活跃区间（Part3需要）
    // TODO: 调用compute_live_intervals
    
    // 步骤4：执行寄存器分配（Part3需要）
    // TODO: 创建LinearScanAllocator并分配
    
    // Part2简化版本：直接分配栈位置
    let mut stack_offset = 0;
    for alloca_name in state.get_alloca_names() {
        ctx.var_locations.insert(alloca_name.clone(), VarLocation::Mem(stack_offset));
        stack_offset += 4;
    }
    ctx.stack_size = stack_offset;
    
    // ==========================================
    // 第二次遍历：生成阶段
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
        ((ctx.stack_size + 15) / 16) * 16  // 对齐到16字节
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
        && !block_name.is_empty() {
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

/// 检测是否为backward branch（循环检测）
/// 通过比较跳转目标的"指令索引"来判断是否存在循环
/// 
/// 原理：如果跳转目标的索引 <= 当前索引，说明跳回到之前的指令，形成循环
/// 注意这个循环指的是回到之前基本块的起始命令位置
fn check_backward_branch(
    instruction: &InstructionValue,
    current_idx: &usize,
    block_indices: &HashMap<String, usize>
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
    block_indices: &HashMap<String, usize>
) -> bool {
    // 1. 获取跳转目标
    // 2. 获取目标名称
    // 3. 获取目标索引（起始指令位置）
    // 4. 判断：目标索引 <= 当前索引 → 跳回去了 → 形成循环
    if let Some(target) = instruction.get_operand(0).and_then(|op| op.right()) 
    && let Ok(target_name) = target.get_name().to_str()
    && let Some(target_idx) = block_indices.get(target_name)
    && *target_idx <= *current_idx {
        return true;
    }
    false
}

/// 检测条件跳转是否构成循环
fn check_conditional_branch(
    instruction: &InstructionValue,
    current_idx: &usize,
    block_indices: &HashMap<String, usize>
) -> bool {
    // 条件跳转有两个分支目标：true分支和false分支
    for operand_idx in [1, 2] {
        if let Some(target) = instruction.get_operand(operand_idx).and_then(|op| op.right()) 
        && let Ok(target_name) = target.get_name().to_str()
        && let Some(target_idx) = block_indices.get(target_name)
        && *target_idx <= *current_idx {
            return true;
        }
    }
    false
}

/// 第一次遍历：收集指令的uses
fn collect_uses(_instruction: &InstructionValue) -> Vec<String> {
    let uses = Vec::new();
    // TODO: 根据指令类型收集operands
    // 例如：
    // - Add/Sub/Mul: 收集两个操作数
    // - Load: 收集指针操作数
    // - Store: 收集value和ptr操作数
    uses
}



/// 获取指令名称
fn get_value_name(instruction: &InstructionValue) -> String {
    if let Some(name_cstr) = instruction.get_name()
    && let Ok(name_str) = name_cstr.to_str()
    && !name_str.is_empty() {
        return name_str.to_string();
    }
    String::new()
}

/// 第二次遍历：生成指令代码
fn generate_instruction(_instruction: &InstructionValue, _asm_builder: &mut AsmBuilder, _ctx: &mut GenContext) {
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
        let _ = generate_asm(&input,&out_path);
    }

    #[test]
    fn test_example1() {
        let file_path = format!("{}{}", FILE_PATH, "example01.sy");
        let out_path = format!("{}{}", FILE_PATH, "example01.s");
        let input = std::fs::read_to_string(file_path).expect("Failed to read file");
        let _ = generate_asm(&input,&out_path);
    }

}