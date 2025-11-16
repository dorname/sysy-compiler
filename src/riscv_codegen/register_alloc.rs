use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};
use crate::riscv_codegen::GenContext;
use crate::riscv_codegen::register_alloc::Location::Reg;

#[derive(Clone, Debug,PartialEq,Eq)]
pub struct InnerVar{
    name: String,
    start_offset: usize,
    end_offset: usize,
}

impl PartialOrd<Self> for InnerVar {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for InnerVar {
    fn cmp(&self, other: &Self) -> Ordering {
        self.start_offset.cmp(&other.start_offset)
    }
}

impl InnerVar {
    pub fn new(name: String, start_offset: usize, end_offset: usize) -> Self {
        Self { name, start_offset, end_offset }
    }
    pub fn get_name(&self) -> &String {
        &self.name
    }
    pub fn get_start_offset(&self) -> usize {
        self.start_offset
    }
    pub fn get_end_offset(&self) -> usize {
        self.end_offset
    }
}

// FIX: ActiveVar用于active集合，按end_offset排序以便快速找到结束时间最晚的变量
#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveVar {
    inner: InnerVar,
}

impl ActiveVar {
    fn new(inner: InnerVar) -> Self {
        Self { inner }
    }
    
    fn as_inner(&self) -> &InnerVar {
        &self.inner
    }
    
    fn into_inner(self) -> InnerVar {
        self.inner
    }
}

impl PartialOrd for ActiveVar {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ActiveVar {
    fn cmp(&self, other: &Self) -> Ordering {
        // FIX: 按end_offset排序，如果相同则按变量名排序（保证唯一性）
        match self.inner.end_offset.cmp(&other.inner.end_offset) {
            Ordering::Equal => self.inner.name.cmp(&other.inner.name),
            other => other,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Location { Reg(String), Stack(i32), Global(String) }

impl Location {
    pub fn get_name(&self) -> String {
        match self {
            Reg(name) => name.to_string(),
            Location::Stack(stack_offset) => stack_offset.to_string(),
            Location::Global(name) => name.to_string(),
        }
    }
}

pub trait RegisterAllocator {
    fn allocate(&mut self,allocation_names:Vec<InnerVar>)->(HashMap<String, String>,usize);
}


// Part2：全部走栈
#[derive(Clone, Default,Debug)]
pub struct NoAlloc;

impl RegisterAllocator for NoAlloc {
    fn allocate(&mut self, allocation_names:Vec<InnerVar>)->(HashMap<String, String>,usize){
        let mut stack_offset:usize = 0;
        let mut allocation_map = HashMap::<String, String>::new();
        for local_val in allocation_names {
            allocation_map.insert(local_val.get_name().clone(), format!("{}(sp)", stack_offset));
            // 因为当前SysY只考虑int类型，一个int是4个字节，所以每次加4
            stack_offset += 4;
        }
        (allocation_map, stack_offset)
    }
}

#[derive(Clone, Default)]
pub struct LinearScan {
    // 空闲的寄存器池
    regs: Vec<String>,
    // FIX: active_vars改为使用ActiveVar，按end_offset排序以便快速找到结束时间最晚的变量
    active_vars: BTreeSet<ActiveVar>,
    // 寄存器和变量映射表
    reg_to_var: HashMap<String, String>,
    // FIX: 栈偏移改为i32类型，使用负数偏移（从0开始递减：0, -4, -8...），符合RISC-V栈向下增长的习惯
    stack_offset: i32,
}

impl LinearScan {
    fn new() -> Self {
        let mut regs = Vec::<String>::new();
            
        // 1. 首先添加临时寄存器（最优先）
        // t0、t1、t2用来作为运算
        // t0、t1 用来加载操作数
        // t2 用来存储结果
        for i in 3..=6 {
            regs.push(format!("t{}", i));
        }
        
        // 2. 然后添加保存寄存器（次优先）
        for i in 0..=11 {
            regs.push(format!("s{}", i));
        }
        
        // 3. 最后添加参数寄存器（仅 a2-a7，保留 a0-a1）
        for i in 2..=7 {
            regs.push(format!("a{}", i));
        }

        Self { regs, active_vars: BTreeSet::new(), reg_to_var: HashMap::new(), stack_offset: 0 }
    }

    fn pop_reg(&mut self) -> Option<String> {
        
        // 优先级1：查找并弹出 t 类型寄存器
        if let Some(pos) = self.regs.iter().position(|r| r.starts_with('t')) {
            return Some(self.regs.remove(pos));
        }
        
        // 优先级2：查找并弹出 s 类型寄存器
        if let Some(pos) = self.regs.iter().position(|r| r.starts_with('s')) {
            return Some(self.regs.remove(pos));
        }
        
        // 优先级3：查找并弹出 a 类型寄存器
        if let Some(pos) = self.regs.iter().position(|r| r.starts_with('a')) {
            return Some(self.regs.remove(pos));
        }
        
        // 没有可用寄存器
        None
    }

    fn clear_inactive_vars(&mut self,var: &InnerVar) {
        // FIX: 修复判断条件和变量名使用错误
        // 收集所有结束时间小于当前变量的开始时间的活跃区间
        let mut inactive_vars = Vec::<ActiveVar>::new();
        for active in self.active_vars.iter() {
            // FIX: 应该判断活跃区间的结束时间是否小于当前变量的开始时间
            if active.as_inner().get_end_offset() < var.get_start_offset() {
                inactive_vars.push(active.clone());
            }
        }
        for inactive in inactive_vars.iter() {
            let inner = inactive.as_inner();
            self.active_vars.remove(inactive);
            // FIX: 应该使用inactive变量的名称来获取寄存器，而不是当前变量var的名称
            if let Some(reg) = self.reg_to_var.get(inner.get_name()) {
                self.regs.push(reg.clone());
            }
        }
    }

    /// 没有寄存器可分配的溢出处理
    fn overflow_to_stack(&mut self,var: &InnerVar) {
        // FIX: 使用ActiveVar后，last()现在能正确返回结束时间最晚的变量
        // FIX: 修正注释和添加else分支处理active_vars为空的情况
        if let Some(max_active_var) = self.active_vars.last().cloned(){
            let max_var = max_active_var.as_inner();
            // FIX: 如果最晚变量的结束时间大于当前变量的结束时间，则spill最晚变量，否则spill当前变量
            if max_var.get_end_offset() > var.get_end_offset() {
                // max_var 溢出到栈，将其寄存器分配给当前变量
                self.put_in_stack(max_var);
                self.active_vars.remove(&max_active_var);
                let reg = self.reg_to_var.get(max_var.get_name()).unwrap();
                // var 加入active集合
                self.active_vars.insert(ActiveVar::new(var.clone()));
                self.reg_to_var.insert(var.get_name().clone(),reg.clone());
            }else {
                // var 溢出到栈（因为var的结束时间更晚或相等）
                self.put_in_stack(var);
            }
        } else {
            // FIX: 如果没有active变量，直接spill当前变量到栈
            self.put_in_stack(var);
        }
    }

    /// 栈分配
    fn put_in_stack(&mut self,var: &InnerVar) {
        // FIX: 使用负数偏移，符合RISC-V栈向下增长的习惯
        self.stack_offset -= 4;
        self.reg_to_var.insert(
            var.get_name().clone(),
            format!("{}(sp)", self.stack_offset),
        );
    }
}

impl RegisterAllocator for LinearScan {
    fn allocate(&mut self,allocation_names:Vec<InnerVar>)->(HashMap<String, String>,usize) { 
        let mut allocation_names = allocation_names.clone();
        // 根据起始区间进行排序
        allocation_names.sort_by_key(|v| v.get_start_offset());
        
        for var in allocation_names {
            // 清理已经结束的活跃区间
            // 判断依据，活跃区间中的变量结束时间小于当前变量的开始时间
            self.clear_inactive_vars(&var);
            // 如果现在空闲的寄存器没有了，则需要考虑是否溢出到栈上
            if self.regs.is_empty() {
                // 判断是否要溢出到栈上
                self.overflow_to_stack(&var);
            }else {
                // 分配寄存器（按优先级：t > s > a）
                let reg = self.pop_reg().unwrap();
                // FIX: 插入ActiveVar而不是InnerVar
                self.active_vars.insert(ActiveVar::new(var.clone()));
                self.reg_to_var.insert(var.get_name().clone(),reg);
            }
        }
        
        // FIX: 计算栈大小并转换负数偏移为正偏移
        // 栈大小 = -stack_offset（如果stack_offset是负数）
        // 使用i32进行计算，确保类型一致性
        let stack_size_i32 = if self.stack_offset < 0 {
            -self.stack_offset
        } else {
            0
        };
        
        // FIX: 将负数偏移转换为正偏移（从栈帧底部开始的偏移）
        let mut fixed_allocation = HashMap::new();
        for (var, loc) in &self.reg_to_var {
            if loc.ends_with("(sp)") {
                let offset_str = loc.trim_end_matches("(sp)");
                if let Ok(offset) = offset_str.parse::<i32>() {
                    if offset < 0 {
                        // FIX: 转换负数偏移为正偏移：positive_offset = stack_size_i32 + offset
                        // 例如：stack_size_i32=12, offset=-4 -> positive_offset=12+(-4)=8
                        // 注意：stack_size_i32 + offset 的结果可能为0或正数
                        let positive_offset = stack_size_i32 + offset;
                        fixed_allocation.insert(var.clone(), format!("{}(sp)", positive_offset));
                    } else {
                        // FIX: 如果已经是正偏移，直接使用（这种情况不应该发生，但为了安全保留）
                        fixed_allocation.insert(var.clone(), loc.clone());
                    }
                } else {
                    fixed_allocation.insert(var.clone(), loc.clone());
                }
            } else {
                fixed_allocation.insert(var.clone(), loc.clone());
            }
        }
        
        // FIX: 将i32类型的stack_size转换为usize返回
        let stack_size = stack_size_i32 as usize;
        (fixed_allocation, stack_size)
    }
}


#[derive(Clone, Default,Debug)]
pub struct AllocatedInnerVar;

impl AllocatedInnerVar {
    pub fn allocate(&self,inner_vars:Vec<InnerVar>,only_stack:bool)->(HashMap<String, String>,usize) {
        if only_stack {
            let mut allocator = NoAlloc::default();
            allocator.allocate(inner_vars)
        } else {
            let mut allocator = LinearScan::new();
            allocator.allocate(inner_vars)
        }
    }
}