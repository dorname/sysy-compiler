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
        self.end_offset.cmp(&other.end_offset)
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
    /// 纯栈分配（考虑生命周期优化）
    /// 基于变量生命周期，让不重叠的变量共享栈位置，节省栈空间
    /// 返回分配结果和栈大小
    fn allocate(&mut self, mut allocation_names: Vec<InnerVar>) -> (HashMap<String, String>, usize) {
        // 按起始时间排序
        allocation_names.sort_by_key(|v| v.get_start_offset());
        
        // 已分配的栈位置：每个位置记录其最后使用的结束时间
        let mut stack_slots: Vec<(i32, usize)> = Vec::new(); // (栈偏移, 最后使用结束时间)
        let mut temp = HashMap::<String, String>::new();
        
        for var in allocation_names {
            let var_start = var.get_start_offset();
            let var_end = var.get_end_offset();
            
            // 查找可重用的栈位置：当前变量的开始时间 >= 某个栈位置的最后使用结束时间
            let mut allocated = false;
            for (offset, last_end) in stack_slots.iter_mut() {
                if var_start > *last_end {
                    // 可以重用这个栈位置
                    temp.insert(var.get_name().clone(), format!("{}(sp)", *offset));
                    *last_end = var_end;
                    allocated = true;
                    break;
                }
            }
            
            // 如果没有可重用的位置，分配新的栈位置
            if !allocated {
                let  len = stack_slots.len()+1;
                let new_offset = -(len as i32 * 4);
                temp.insert(var.get_name().clone(), format!("{}(sp)", new_offset));
                stack_slots.push((new_offset, var_end));
            }
        }
        
        // 计算栈大小
        let stack_size = (stack_slots.len() * 4) as usize;
        
        // 将负数偏移转换为正数（从栈帧底部开始的偏移）
        let mut allocation_map = HashMap::new();
        for (name, location) in temp.iter() {
            if location.ends_with("(sp)") {
                let offset_str = location.trim_end_matches("(sp)");
                if let Ok(offset) = offset_str.parse::<i32>() {
                    if offset < 0 {
                        let positive_offset = stack_size as i32 + offset;
                        allocation_map.insert(name.clone(), format!("{}(sp)", positive_offset));
                    } else {
                        allocation_map.insert(name.clone(), location.clone());
                    }
                }
            } 
        }
        
        (allocation_map, stack_size)
    }
}

#[derive(Clone, Default)]
pub struct LinearScan {
    // 空闲的寄存器池
    regs: Vec<String>,
    // 活跃变量集合
    active_vars: BTreeSet<InnerVar>,
    // 寄存器和变量映射表
    var_reg_map: HashMap<String, String>,
    // 栈偏移改为i32类型，使用负数偏移（从0开始递减：0, -4, -8...），符合RISC-V栈向下增长的习惯
    stack_offset: i32,
    // 活跃栈空间
    stack_slots: Vec<(i32, usize)>,
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

        Self { regs, active_vars: BTreeSet::new(), var_reg_map: HashMap::new(), stack_offset: 0,stack_slots: Vec::new() }
    }

    fn auto_sort_regs(&mut self) {
        // 按照 t -> s -> a 排序
        // 对于临时寄存器 t 按照编号小到大排序
        // 对于寄存器 s 按照编号小到大排序
        // 对于寄存器 a 按照编号小到大排序
        self.regs.sort_by(|a, b| {
            // 获取寄存器类型优先级：t=0, s=1, a=2
            let get_type_priority = |reg: &str| -> u8 {
                if reg.starts_with('t') {
                    0
                } else if reg.starts_with('s') {
                    1
                } else if reg.starts_with('a') {
                    2
                } else {
                    3 // 未知类型，放在最后
                }
            };
            
            // 获取寄存器编号
            let get_number = |reg: &str| -> Option<u32> {
                reg.chars()
                    .skip(1)
                    .collect::<String>()
                    .parse::<u32>()
                    .ok()
            };
            
            let priority_a = get_type_priority(a);
            let priority_b = get_type_priority(b);
            
            // 首先按类型优先级排序
            match priority_a.cmp(&priority_b) {
                std::cmp::Ordering::Equal => {
                    // 同类型内按编号从小到大排序
                    let num_a = get_number(a).unwrap_or(0);
                    let num_b = get_number(b).unwrap_or(0);
                    num_a.cmp(&num_b)
                }
                other => other,
            }
        });
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


    /// 清理已结束的活跃变量，释放其占用的寄存器
    ///
    /// 在线性扫描寄存器分配算法中，当一个变量的生命周期结束时，
    /// 需要将其从活跃变量集合中移除，并将其占用的寄存器释放回寄存器池。
    ///
    /// # 参数
    /// * `var` - 当前正在处理的变量，用于判断哪些变量已经结束
    ///
    /// # 算法逻辑
    /// 1. 遍历所有活跃变量，找出结束时间小于当前变量开始时间的变量
    /// 2. 将这些不活跃的变量从 `active_vars` 集合中移除
    /// 3. 将它们占用的寄存器释放回 `regs` 寄存器池
    ///
    /// # 示例
    /// ```
    /// // 假设当前变量 var 的开始时间为 20
    /// // 活跃变量集合中有：
    /// //   - var_a: [0, 10]  -> 结束时间 10 < 20，需要清理
    /// //   - var_b: [5, 15]  -> 结束时间 15 < 20，需要清理
    /// //   - var_c: [12, 25] -> 结束时间 25 >= 20，保持活跃
    /// // 调用后，var_a 和 var_b 的寄存器被释放，var_c 保持活跃
    /// ```
    fn clear_inactive_vars(&mut self, var: &InnerVar) {
        // 收集所有结束时间小于当前变量的开始时间的活跃区间
        let mut inactive_vars = Vec::<InnerVar>::new();
        for active in self.active_vars.iter() {
            // 判断活跃区间的结束时间是否小于当前变量的开始时间
            if active.get_end_offset() < var.get_start_offset() {
                inactive_vars.push(active.clone());
            }
        }
        // 移除不活跃的变量并释放其寄存器
        for inactive in inactive_vars.iter() {
            self.active_vars.remove(inactive);
            // 使用inactive变量的名称来获取寄存器并释放
            if let Some(reg) = self.var_reg_map.get(inactive.get_name()) {
                self.regs.push(reg.clone());
                self.auto_sort_regs();
            }
        }
    }

    /// 处理寄存器溢出到栈的情况
    ///
    /// 当没有可用寄存器时，需要将某个变量溢出（spill）到栈上。
    /// 本函数实现了"spill farthest"策略：优先溢出结束时间更早的变量，
    /// 因为它们在将来更早结束，可以更快释放寄存器资源。
    ///
    /// # 参数
    /// * `var` - 当前需要分配寄存器的变量
    ///
    /// # 算法逻辑
    /// 1. 如果存在活跃变量，选择结束时间最晚的活跃变量（`active_vars.last()`）
    /// 2. 比较最晚活跃变量的结束时间和当前变量的结束时间：
    ///    - 如果最晚变量的结束时间 > 当前变量的结束时间：
    ///      * 将最晚变量溢出到栈（`put_in_stack`）
    ///      * 将最晚变量的寄存器分配给当前变量
    ///    - 否则（当前变量的结束时间更晚或相等）：
    ///      * 将当前变量溢出到栈
    /// 3. 如果没有活跃变量，直接将当前变量溢出到栈
    ///
    /// # 示例
    /// ```
    /// // 假设当前变量 var 的生命周期为 [20, 30]
    /// // 活跃变量集合中有：
    /// //   - var_a: [0, 25]  -> 结束时间 25
    /// //   - var_b: [10, 35] -> 结束时间 35（最晚）
    /// // 由于 var_b 的结束时间 35 > var 的结束时间 30，
    /// // 因此溢出 var_b 到栈，将其寄存器分配给 var
    /// ```
    fn overflow_to_stack(&mut self,var: &InnerVar) {
        if let Some(max_active_var) = self.active_vars.last().cloned(){
            let max_var = max_active_var;
            // 如果最晚变量的结束时间大于当前变量的结束时间，则spill最晚变量，否则spill当前变量
            if max_var.get_end_offset() > var.get_end_offset() {
                // max_var 溢出到栈，将其寄存器分配给当前变量
                self.put_in_stack(&max_var);
                self.active_vars.remove(&max_var);
                let reg = self.var_reg_map.get(max_var.get_name()).unwrap();
                // var 加入active集合
                self.active_vars.insert(var.clone());
                self.var_reg_map.insert(var.get_name().clone(),reg.clone());
            }else {
                // var 溢出到栈（因为var的结束时间更晚或相等）
                self.put_in_stack(var);
            }
        } else {
            // 如果没有active变量，直接spill当前变量到栈
            self.put_in_stack(var);
        }
    }

    /// 栈分配
    fn put_in_stack(&mut self,var: &InnerVar) {
        // 使用负数偏移，符合RISC-V栈向下增长的习惯
        self.stack_offset -= 4;
        self.var_reg_map.insert(
            var.get_name().clone(),
            format!("{}(sp)", self.stack_offset),
        );
    }
}

impl RegisterAllocator for LinearScan {
    /// 使用线性扫描算法进行寄存器分配
    ///
    /// 线性扫描是一种高效的寄存器分配算法，通过按变量生命周期的起始时间顺序扫描，
    /// 动态分配和释放寄存器，当寄存器不足时将变量溢出到栈上。
    ///
    /// # 参数
    /// * `allocation_names` - 需要分配寄存器的变量列表，每个变量包含名称和生命周期区间
    ///
    /// # 返回值
    /// 返回一个元组 `(allocation_map, stack_size)`：
    /// * `allocation_map` - `HashMap<String, String>`：变量名到寄存器/栈位置的映射
    ///   - 寄存器格式：`"t0"`, `"s1"`, `"a2"` 等
    ///   - 栈位置格式：`"8(sp)"`, `"12(sp)"` 等（正偏移，从栈帧底部开始）
    /// * `stack_size` - `usize`：所需栈空间大小（字节数）
    ///
    /// # 算法步骤
    /// 1. **排序**：按变量的起始时间（`start_offset`）对变量列表进行排序
    /// 2. **遍历分配**：对每个变量执行以下操作：
    ///    - 清理已结束的活跃变量（调用 `clear_inactive_vars`）
    ///    - 如果有空闲寄存器：
    ///      * 按优先级分配寄存器（t > s > a）
    ///      * 将变量名和寄存器映射关系存入 `var_reg_map`
    ///    - 如果没有空闲寄存器：
    ///      * 调用 `overflow_to_stack` 处理溢出
    ///      * 使用"spill farthest"策略选择要溢出的变量
    /// 3. **偏移转换**：将内部使用的负数栈偏移转换为正偏移
    ///    - 内部使用负数偏移（如 -4, -8），符合 RISC-V 栈向下增长的习惯
    ///    - 最终输出正偏移（如 8, 12），从栈帧底部开始计算
    /// 4. **计算栈大小**：根据使用的栈偏移计算所需栈空间
    ///
    /// # 寄存器优先级
    /// 寄存器分配按以下优先级顺序：
    /// 1. **临时寄存器** (`t3`-`t6`)：最优先，用于临时计算
    /// 2. **保存寄存器** (`s0`-`s11`)：次优先，需要保存和恢复
    /// 3. **参数寄存器** (`a2`-`a7`)：最后使用，保留 `a0`-`a1` 用于函数调用
    ///
    /// # 示例
    /// ```
    /// use crate::riscv_codegen::register_alloc::{InnerVar, LinearScan, RegisterAllocator};
    /// 
    /// let mut allocator = LinearScan::new();
    /// let vars = vec![
    ///     InnerVar::new("a".to_string(), 0, 10),
    ///     InnerVar::new("b".to_string(), 5, 20),
    ///     InnerVar::new("c".to_string(), 15, 30),
    /// ];
    /// let (allocation_map, stack_size) = allocator.allocate(vars);
    /// // allocation_map 包含变量到寄存器/栈位置的映射
    /// // stack_size 表示所需的栈空间大小
    /// ```
    fn allocate(&mut self,allocation_names:Vec<InnerVar>)->(HashMap<String, String>,usize) { 
        let mut allocation_names = allocation_names.clone();
        // 根据起始区间进行排序
        allocation_names.sort_by_key(|v| v.get_start_offset());
        for var in allocation_names {
            // 清理已经结束的活跃区间
            self.clear_inactive_vars(&var);
            // 如果现在空闲的寄存器没有了，则需要考虑是否溢出到栈上
            if self.regs.is_empty() {
                // 判断是否要溢出到栈上
                self.overflow_to_stack(&var);
            } else {
                // 分配寄存器（按优先级：t > s > a）
                if let Some(reg) = self.pop_reg() {
                    self.active_vars.insert(var.clone());
                    self.var_reg_map.insert(var.get_name().clone(),reg);
                }else {
                    println!("{:?}",self.regs);
                }
            }
        }
        
        // 栈大小 = -stack_offset（如果stack_offset是负数）
        // 使用i32进行计算，确保类型一致性
        let stack_size_i32 = if self.stack_offset < 0 {
            -self.stack_offset
        } else {
            0
        };
        
        // 将负数偏移转换为正偏移（从栈帧底部开始的偏移）
        let mut temp = HashMap::new();
        for (var, loc) in &self.var_reg_map {
            if loc.ends_with("(sp)") {
                let offset_str = loc.trim_end_matches("(sp)");
                if let Ok(offset) = offset_str.parse::<i32>()
                && offset < 0 {
                    let positive_offset = stack_size_i32 + offset;
                    temp.insert(var.clone(), format!("{}(sp)", positive_offset));
                } else {
                    temp.insert(var.clone(), loc.clone());
                }
            } else {
                temp.insert(var.clone(), loc.clone());
            }
        }
        
        // 将i32类型的stack_size转换为usize返回
        let stack_size = stack_size_i32 as usize;
        (temp, stack_size)
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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};

    use crate::riscv_codegen::register_alloc::{InnerVar, LinearScan, NoAlloc, RegisterAllocator};

    #[test]
    fn test_only_stack1(){
        let mut allocator = NoAlloc::default();
        // 由于生命周期没有重合所以栈空间可以完全复用
        // 栈空间4  0(sp)
        let mocks:Vec<InnerVar> = vec![
            InnerVar::new("a".to_string(), 0, 10),  
            InnerVar::new("b".to_string(), 11, 20), 
            InnerVar::new("c".to_string(), 21, 30), 
            InnerVar::new("d".to_string(), 31, 40), 
            InnerVar::new("e".to_string(), 41, 50), 
            InnerVar::new("f".to_string(), 51, 60), 
        ];
        let (allocation_map, stack_size) = allocator.allocate(mocks);
        assert_eq!(allocation_map, HashMap::<String, String>::from([
            ("a".to_string(), "0(sp)".to_string()),
            ("b".to_string(), "0(sp)".to_string()),
            ("c".to_string(), "0(sp)".to_string()),
            ("d".to_string(), "0(sp)".to_string()),
            ("e".to_string(), "0(sp)".to_string()),
            ("f".to_string(), "0(sp)".to_string()),
        ]));
        assert_eq!(stack_size, 4);
    }


    #[test]
    fn test_only_stack2(){
        let mut allocator = NoAlloc::default();
        // 由于生命周期没有重合所以栈空间可以完全复用
        // 由于hashmap的无序性分配的栈偏移也具有随机性，但栈空间基本是定的
        let mocks:Vec<InnerVar> = vec![
            InnerVar::new("a".to_string(), 0, 10), // -4  -> 12
            InnerVar::new("b".to_string(), 5, 20), // -8 -> 8
            InnerVar::new("c".to_string(), 21, 30), // -8 -> 8
            InnerVar::new("d".to_string(), 5, 40),  // -12 -> 4
            InnerVar::new("e".to_string(), 20, 50), // -4 -> 12
            InnerVar::new("f".to_string(), 3, 60), // -16 -> 0
        ];
        let (_, stack_size) = allocator.allocate(mocks);
        assert_eq!(stack_size, 16);
    }

    #[test]
    fn test_check_sort(){
        let active_vars = BTreeSet::<InnerVar>::from([
            InnerVar::new("a".to_string(), 0, 10),
            InnerVar::new("b".to_string(), 5, 20),
            InnerVar::new("c".to_string(), 21, 30),
            InnerVar::new("d".to_string(), 5, 40),
            InnerVar::new("e".to_string(), 20, 50),
            InnerVar::new("f".to_string(), 3, 60),
        ]);
        println!("{:?}", active_vars);
    }

    #[test]
    fn test_linear_scan1(){
        let mut allocator = LinearScan::new();
        let mocks:Vec<InnerVar> = vec![
            InnerVar::new("a".to_string(), 0, 10),
            InnerVar::new("b".to_string(), 5, 20),
            InnerVar::new("c".to_string(), 21, 30),
            InnerVar::new("d".to_string(), 5, 40),
            InnerVar::new("e".to_string(), 20, 50),
            InnerVar::new("f".to_string(), 3, 60),
        ];
        // 由线性扫描器扫描之后的顺序是 a(0,10) f(3,60) b(5,20) d(5,40) e(20,50) c(21,30)
        let (allocation_map, stack_size) = allocator.allocate(mocks);
        assert_eq!(allocation_map, HashMap::<String, String>::from([
            ("a".to_string(), "t3".to_string()),
            ("b".to_string(), "t5".to_string()),
            ("c".to_string(), "s1".to_string()),
            ("d".to_string(), "t6".to_string()),
            ("e".to_string(), "s0".to_string()),
            ("f".to_string(), "t4".to_string()),
        ]));
        assert_eq!(stack_size, 0);
    }


    #[test]
    fn test_linear_scan2(){
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
        assert_eq!(allocation_map, HashMap::<String, String>::from([
            ("a".to_string(), "t3".to_string()),
            ("b".to_string(), "t4".to_string()),
            ("c".to_string(), "t4".to_string()),
            ("d".to_string(), "t6".to_string()),
            ("e".to_string(), "t5".to_string()),
            ("f".to_string(), "t3".to_string()),
        ]));
        assert_eq!(stack_size, 0);
    }
}