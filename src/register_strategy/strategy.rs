#[derive(Clone,Copy,Debug)]
pub struct VarId(pub u32);

#[derive(Clone,Debug)]
pub enum Location {
    Reg(&'static str),
    Stack(i32),
    Global(&'static str),
}

pub trait RegisterAllocator {
    fn allocate(&mut self, v:VarId) -> Location;
    fn stack_size(&self) -> usize;
}

pub struct NoAlloc;
impl RegisterAllocator for NoAlloc {
    fn allocate(&mut self, _:VarId) -> Location {Location::Stack(0)}
    fn stack_size(&self) -> usize {0}
}

pub struct LinearScan {}
