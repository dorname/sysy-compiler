use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};
use crate::riscv_codegen::GenContext;

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

#[derive(Clone, Debug)]
pub enum Location { Reg(String), Stack(i32), Global(String) }

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

#[derive(Clone, Default,Debug)]
pub struct LinearScan {
    // 空闲的寄存器池
    regs: Vec<String>,
    // 被占用的寄存器集合 HashSet 没有循序 改成BTreeSet     
    active_vars: BTreeSet<InnerVar>,
    // 寄存器和变量映射表
    reg_to_var: HashMap<String, String>,
    // 栈空间
    stack_offset: usize,
}

impl LinearScan {
    fn new() -> Self {
        let mut regs = Vec::<String>::new();
            
        // 1. 首先添加临时寄存器（最优先）
        for i in 0..=6 {
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
        // 收集所有结束时间小于当前变量的开始时间的活跃区间
        let mut inactive_vars = Vec::<InnerVar>::new();
        for active in self.active_vars.iter() {
            if var.get_end_offset() < active.get_start_offset() {
                inactive_vars.push(active.clone());
            }
        }
        for inactive in inactive_vars.iter() {
            self.active_vars.remove(inactive);
            if let Some(reg) = self.reg_to_var.get(var.get_name()) {
                self.regs.push(reg.clone());
            }
        }
    }

    /// 没有寄存器可分配的溢出处理
    fn overflow_to_stack(&mut self,var: &InnerVar) {
        // 对比当前变量和活跃列表中结束时间最晚的变量，如果当前变量的结束时间大于最晚的变量，则溢出到栈上
        if let Some(max_var) = self.active_vars.last().cloned(){
            if max_var.get_end_offset() >= var.get_start_offset() {
                // max_var 溢出到栈
                self.put_in_stack(&max_var);
                self.active_vars.remove(&max_var);
                let reg = self.reg_to_var.get(max_var.get_name()).unwrap();
                // var 加入栈帧
                self.active_vars.insert(var.clone());
                self.reg_to_var.insert(var.get_name().clone(),reg.clone());
            }else {
                // var 溢出到栈
                self.put_in_stack(var);
            }
        }
    }

    /// 栈分配
    fn put_in_stack(&mut self,var: &InnerVar) {
        self.reg_to_var.insert(
            var.get_name().clone(),
            format!("{}(sp)", self.stack_offset),
        );
        self.stack_offset += 4;
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
                self.active_vars.insert(var.clone());
                self.reg_to_var.insert(var.get_name().clone(),reg);
            }
        }
        // 计算栈的总量
        (self.reg_to_var.clone(),self.stack_offset)
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
            let mut allocator = LinearScan::default();
            allocator.allocate(inner_vars)
        }
    }
}