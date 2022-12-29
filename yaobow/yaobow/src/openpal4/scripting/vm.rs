use std::rc::Rc;

use super::module::ScriptFunction;

pub struct ScriptVm {
    function: Option<Rc<ScriptFunction>>,
    pc: usize,
    stack: Vec<u8>,
    sp: usize,
}

impl ScriptVm {
    const DEFAULT_STACK_SIZE: usize = 4096;
    pub fn new() -> Self {
        Self {
            function: None,
            pc: 0,
            stack: vec![0; Self::DEFAULT_STACK_SIZE],
            sp: Self::DEFAULT_STACK_SIZE,
        }
    }

    pub fn set_function(&mut self, function: Rc<ScriptFunction>) {
        self.function = Some(function);
    }

    pub fn execute(&mut self) {
        if self.function.is_none() {
            return;
        }

        let f = self.function.clone().unwrap();
        let function = f.as_ref();
        let mut reg: u32 = 0;

        loop {
            let inst = self.read_inst(function);
            macro_rules! command {
                ($cmd_name: ident, $param_name: ident : $param_type: ident) => {{
                    let $param_name = data_read::$param_type(&function.inst, &mut self.pc);
                    self.$cmd_name($param_name);
                }};
            }

            match inst {
                0 => command!(add_sp, size: u16),
                1 => command!(sub_sp, size: u16),
                2 => command!(push, size: u32),
                3 => self.deref(),
                4 => command!(load, index: u16),
                5 => self.write(),
                6 => self.write2(),
                7 => command!(lea, index: u16),
                8 => command!(save, index: u16),
                9 => self.swap(),
                10 => self.save_reg(&mut reg),
                11 => self.load_reg(&reg),
                _ => unreachable!(),
            }
        }
    }

    fn read_inst(&mut self, function: &ScriptFunction) -> u8 {
        let inst = function.inst[self.pc];
        self.pc += 4;
        inst
    }

    fn add_sp(&mut self, size: u16) {
        self.sp += size as usize;
    }

    fn sub_sp(&mut self, size: u16) {
        self.sp -= size as usize;
    }

    fn deref(&mut self) {
        unsafe {
            let pos: u32 = self.read_stack(self.sp);
            let data: u32 = self.read_stack(pos as usize);
            self.write_stack(self.sp, data);
        }
    }

    fn push(&mut self, data: u32) {
        self.sp -= 4;
        unsafe {
            self.write_stack(self.sp, data);
        }
    }

    fn load(&mut self, index: u16) {
        unsafe {
            let data: u32 = self.read_stack(self.stack.len() - index as usize * 4);
            self.write_stack(self.sp, data);
        }
    }

    fn write(&mut self) {
        unsafe {
            let pos: u32 = self.read_stack(self.sp);
            self.sp += 4;
            let data: u32 = self.read_stack(self.sp);
            self.write_stack(pos as usize, data);
        }
    }

    fn write2(&mut self) {
        unsafe {
            self.write();
            self.sp += 4;
        }
    }

    fn lea(&mut self, index: u16) {
        unsafe {
            let pos = self.stack.len() - index as usize * 4;
            self.sp -= 4;
            self.write_stack(self.sp, pos);
        }
    }

    fn save(&mut self, index: u16) {
        unsafe {
            let pos = self.stack.len() - index as usize * 4;
            let data: u32 = self.read_stack(pos);
            self.write_stack(pos, data);
            self.sp += 4;
        }
    }

    fn swap(&mut self) {
        unsafe {
            let data: u32 = self.read_stack(self.sp);
            let data2: u32 = self.read_stack(self.sp + 4);
            self.write_stack(self.sp, data2);
            self.write_stack(self.sp + 4, data);
        }
    }

    fn save_reg(&mut self, reg: &mut u32) {
        unsafe {
            let data = self.read_stack(self.sp);
            self.sp += 4;
            *reg = data;
        }
    }

    fn load_reg(&mut self, reg: &u32) {
        unsafe {
            self.sp -= 4;
            self.write_stack(self.sp, *reg);
        }
    }

    #[inline]
    unsafe fn write_stack<T>(&mut self, pos: usize, data: T) {
        *(&mut self.stack[pos] as *mut u8 as *mut T) = data;
    }

    #[inline]
    unsafe fn read_stack<T: Copy>(&self, pos: usize) -> T {
        *(&self.stack[pos] as *const u8 as *const T)
    }
}

mod data_read {
    use byteorder::{LittleEndian, ReadBytesExt};

    pub(super) fn u16(inst: &[u8], pc: &mut usize) -> u16 {
        *pc += 2;
        (&inst[*pc - 2..*pc]).read_u16::<LittleEndian>().unwrap()
    }

    pub(super) fn i16(inst: &[u8], pc: &mut usize) -> i16 {
        *pc += 2;
        (&inst[*pc - 2..*pc]).read_i16::<LittleEndian>().unwrap()
    }

    pub(super) fn i32(inst: &[u8], pc: &mut usize) -> i32 {
        *pc += 4;
        (&inst[*pc - 4..*pc]).read_i32::<LittleEndian>().unwrap()
    }

    pub(super) fn u32(inst: &[u8], pc: &mut usize) -> u32 {
        *pc += 4;
        (&inst[*pc - 4..*pc]).read_u32::<LittleEndian>().unwrap()
    }

    pub(super) fn f32(inst: &[u8], pc: &mut usize) -> f32 {
        *pc += 4;
        (&inst[*pc - 4..*pc]).read_f32::<LittleEndian>().unwrap()
    }
}
