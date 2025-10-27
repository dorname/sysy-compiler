use std::collections::{HashMap, HashSet};
use crate::riscv_codegen::GenContext;

#[derive(Clone, Debug)]
pub struct InnerVar{
    name: String,
    start_offset: usize,
    end_offset: usize,
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
    fn allocate(&mut self,allocation_names:Vec<InnerVar>)->(HashMap<String, String>,i32);
}


// Part2：全部走栈
#[derive(Clone, Default,Debug)]
pub struct NoAlloc;

impl RegisterAllocator for NoAlloc {
    fn allocate(&mut self, allocation_names:Vec<InnerVar>)->(HashMap<String, String>,i32){
        let mut stack_offset:i32 = 0;
        let mut allocation_map = HashMap::<String, String>::new();
        for local_val in allocation_names {
            allocation_map.insert(local_val.get_name().clone(), stack_offset.to_string());
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
    // 被占用的寄存器集合
    used_regs: HashSet<String>,
    // 寄存器和变量映射表
    reg_to_var: HashMap<String, String>,
    // 栈空间
    stack_offset: usize,
}

impl LinearScan {
    fn new() -> Self {
        todo!()
    }
}

impl RegisterAllocator for LinearScan {
    fn allocate(&mut self,allocation_names:Vec<InnerVar>)->(HashMap<String, String>,i32) { 
        todo!()
    }
}


#[derive(Clone, Default,Debug)]
pub struct AllocatedInnerVar;

impl AllocatedInnerVar {
    pub fn allocate(&self,inner_vars:Vec<InnerVar>,only_stack:bool)->(HashMap<String, String>,i32) {
        if only_stack {
            let mut allocator = NoAlloc::default();
            allocator.allocate(inner_vars)
        } else {
            let mut allocator = LinearScan::default();
            allocator.allocate(inner_vars)
        }
    }
}