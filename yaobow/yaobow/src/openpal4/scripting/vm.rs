use std::{cell::RefCell, rc::Rc};

use super::{
    global_context::ScriptGlobalContext,
    module::{ScriptFunction, ScriptModule},
};

pub struct ScriptVm {
    context: Rc<RefCell<ScriptGlobalContext>>,
    module: Option<Rc<RefCell<ScriptModule>>>,
    function_index: usize,
    pc: usize,
    stack: Vec<u8>,
    sp: usize,
    r1: u32,
    r2: u32,
}

impl ScriptVm {
    const DEFAULT_STACK_SIZE: usize = 4096;
    pub fn new(context: Rc<RefCell<ScriptGlobalContext>>) -> Self {
        Self {
            context,
            module: None,
            function_index: 0,
            pc: 0,
            stack: vec![0; Self::DEFAULT_STACK_SIZE],
            sp: Self::DEFAULT_STACK_SIZE,
            r1: 0,
            r2: 0,
        }
    }

    pub fn set_module(&mut self, module: Rc<RefCell<ScriptModule>>) {
        self.module = Some(module);
    }

    pub fn set_function(&mut self, index: usize) {
        self.function_index = index;
    }

    pub fn execute(&mut self) {
        if self.module.is_none() {
            return;
        }

        let module = self.module.clone().unwrap();
        let module_ref = module.borrow();
        let function = module_ref.functions[self.function_index].clone();
        let mut reg: u32 = 0;

        loop {
            let inst = self.read_inst(&function);
            macro_rules! command {
                ($cmd_name: ident $(, $param_name: ident : $param_type: ident)*) => {{
                    $(let $param_name = data_read::$param_type(&function.inst, &mut self.pc);)*
                    self.$cmd_name($($param_name ,)*);
                }};

                ($cmd_name: ident : $g_type: ident $(, $param_name: ident : $param_type: ident)*) => {{
                    $(let $param_name = data_read::$param_type(&function.inst, &mut self.pc);)*
                    self.$cmd_name::<$g_type>($($param_name)*);
                }};
            }

            match inst {
                0 => command!(add_sp, size: u16),
                1 => command!(sub_sp, size: u16),
                2 => command!(push, size: u32),
                3 => self.deref(),
                4 => command!(load, index: u16),
                5 => self.store(),
                6 => self.store_pop(),
                7 => command!(lea, index: u16),
                8 => command!(load_pop, index: u16),
                9 => self.swap::<u32>(),
                10 => self.store_reg(&mut reg),
                11 => self.load_reg(reg),
                12 => command!(call, function: u32),
                13 => {
                    command!(ret, param_size: u16);
                    return;
                }
                14 => command!(jmp, offset: i32),
                15 => command!(jz, offset: i32),
                16 => command!(jnz, offset: i32),
                17 => self.is_zero(),
                18 => self.not_zero(),
                19 => self.ltz(),
                20 => self.gez(),
                21 => self.gtz(),
                22 => self.lez(),
                23 => self.add::<i32>(),
                24 => self.sub::<i32>(),
                25 => self.mul::<i32>(),
                26 => self.div::<i32>(0),
                27 => self.xmod::<i32>(0),
                28 => self.neg::<i32>(),
                29 => self.cmp::<i32>(),
                30 => self.deref_inc::<i32>(1),
                31 => self.deref_dec::<i32>(1),
                32 => self.fild(),
                33 => self.add::<f32>(),
                34 => self.sub::<f32>(),
                35 => self.mul::<f32>(),
                36 => self.div::<f32>(0.),
                37 => self.xmod::<f32>(0.),
                38 => self.neg::<f32>(),
                39 => self.cmp::<f32>(),
                40 => self.deref_inc::<f32>(1.),
                41 => self.deref_dec::<f32>(1.),
                42 | 51 => self.fst(),
                43 => self.bnot(),
                44 => self.band(),
                45 => self.bor(),
                46 => self.bxor(),
                47 => self.shl(),
                48 => self.shr(),
                49 => self.sar(),
                50 => self.fuld(),
                52 => self.cmp::<i32>(),
                53 => self.truc_to_i8(),
                54 => self.truc_to_i16(),
                55 => self.truc_to_u8(),
                56 => self.truc_to_u16(),
                57 => self.assign_byte(),
                58 => self.assign_word(),
                59 => self.deref_inc::<u16>(1),
                60 => self.deref_inc::<u8>(1),
                61 => self.deref_dec::<u16>(1),
                62 => self.deref_dec::<u8>(1),
                63 => self.push_zero(),
                64 => command!(memcpy, count: u16),
                65 => command!(load_global, index: i32),
                66 => command!(push_u64, data: u64),
                67 => self.load_u64(),
                68 => self.store_u64(),
                69 => self.neg::<f64>(),
                70 => self.deref_inc::<f64>(1.),
                71 => self.deref_dec::<f64>(1.),
                72 => self.add::<f64>(),
                73 => self.sub::<f64>(),
                74 => self.mul::<f64>(),
                75 => self.div::<f64>(0.),
                76 => self.xmod::<f64>(0.),
                77 => self.swap::<f64>(),
                78 => self.cmp::<f64>(),
                79 | 80 => self.d2i(),
                81 => self.d2f(),
                82 => self.x2d::<i32>(),
                83 => self.x2d::<u32>(),
                84 => self.x2d::<f32>(),
                85 => self.jmps(),
                86 => self.store_dword(),
                87 => self.store_qword(),
                88 => self.restore_dword(),
                89 => self.restore_qword(),
                90 => command!(get_string, index: u16),
                91 => command!(jgez, offset: i32),
                92 => command!(jlz, offset: i32),
                93 => command!(jlez, offset: i32),
                94 => command!(jgz, offset: i32),
                95 | 96 => command!(cmp_imm: i32, rhs: i32),
                97 => command!(call_global, function_index: i32),
                98 => command!(call_external, function_index: u32),
                99 => command!(load_global, index: i32),
                100 => command!(store_global, index: i32),
                101 => command!(add_imm: i32, rhs: i32),
                102 => command!(sub_imm: i32, rhs: i32),
                103 => command!(cmp_imm: f32, rhs: f32),
                104 => command!(add_imm: f32, rhs: f32),
                105 => command!(sub_imm: f32, rhs: f32),
                106 => command!(mul_imm: i32, rhs: i32),
                107 => command!(mul_imm: f32, rhs: f32),
                108 => println!("unimplemented byte code 108"),
                109 => command!(call_global2, this: i32, index: i32),
                110 => unimplemented!("byte code 110"),
                111 => unimplemented!("byte code 111"),
                112 => unimplemented!("byte code 112"),
                113 => unimplemented!("byte code 113"),
                114 => unimplemented!("byte code 114"),
                115 => unimplemented!("byte code 115"),
                116 => unimplemented!("byte code 116"),
                117 => unimplemented!("byte code 117"),
                118 => unimplemented!("byte code 118"),
                119 => unimplemented!("byte code 119"),
                120 => unimplemented!("byte code 120"),
                121 => unimplemented!("byte code 121"),
                122 => unimplemented!("byte code 122"),
                i => unimplemented!("byte code {}", i),
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

    fn store(&mut self) {
        unsafe {
            let pos: u32 = self.read_stack(self.sp);
            self.sp += 4;
            let data: u32 = self.read_stack(self.sp);
            self.write_stack(pos as usize, data);
        }
    }

    fn store_pop(&mut self) {
        self.store();
        self.sp += 4;
    }

    fn lea(&mut self, index: u16) {
        unsafe {
            let pos = self.stack.len() - index as usize * 4;
            self.sp -= 4;
            self.write_stack(self.sp, pos);
        }
    }

    fn load_pop(&mut self, index: u16) {
        unsafe {
            let pos = self.stack.len() - index as usize * 4;
            let data: u32 = self.read_stack(pos);
            self.write_stack(pos, data);
            self.sp += 4;
        }
    }

    fn swap<T: Copy>(&mut self) {
        unsafe {
            let size = std::mem::size_of::<T>();
            let data: T = self.read_stack(self.sp);
            let data2: T = self.read_stack(self.sp + size);
            self.write_stack(self.sp, data2);
            self.write_stack(self.sp + size, data);
        }
    }

    fn store_reg(&mut self, reg: &mut u32) {
        unsafe {
            let data = self.read_stack(self.sp);
            self.sp += 4;
            *reg = data;
        }
    }

    fn load_reg(&mut self, reg: u32) {
        unsafe {
            self.sp -= 4;
            self.write_stack(self.sp, reg);
        }
    }

    fn call(&mut self, function: u32) {
        println!("Unimplemented: call: {}", function);
    }

    fn call_external(&mut self, function: u32) {
        println!("Unimplemented: call: {}", function);
    }

    fn call_global(&mut self, function: i32) {
        println!("Unimplemented: call global: {}", function);
    }

    fn call_global2(&mut self, this: i32, function: i32) {
        println!("Unimplemented: call global2: {} {}", this, function);
    }

    fn ret(&mut self, param_size: u16) {
        println!("Unimplemented: ret: {}", param_size);
    }

    fn jmp(&mut self, offset: i32) {
        self.pc += offset as usize;
    }

    fn jz(&mut self, offset: i32) {
        unsafe {
            let data: i32 = self.read_stack(self.sp);
            self.sp += 4;
            if data == 0 {
                self.jmp(offset);
            }
        }
    }

    fn jnz(&mut self, offset: i32) {
        unsafe {
            let data: i32 = self.read_stack(self.sp);
            self.sp += 4;
            if data != 0 {
                self.jmp(offset);
            }
        }
    }

    fn is_zero(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a == 0) as i32);
    }

    fn not_zero(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a != 0) as i32);
    }

    fn ltz(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a < 0) as i32);
    }

    fn gez(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a >= 0) as i32);
    }

    fn gtz(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a > 0) as i32);
    }

    fn lez(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a <= 0) as i32);
    }

    fn add<T: Copy + std::ops::Add>(&mut self) {
        self.binary_op::<T, _, _>(|a, b| b + a)
    }

    fn sub<T: Copy + std::ops::Sub>(&mut self) {
        self.binary_op::<T, _, _>(|a, b| b - a)
    }

    fn mul<T: Copy + std::ops::Mul>(&mut self) {
        self.binary_op::<T, _, _>(|a, b| b * a)
    }

    fn div<T: Copy + std::ops::Div + PartialEq>(&mut self, zero: T) {
        unsafe {
            let data1: T = self.read_stack(self.sp);
            if data1 == zero {
                panic!("divided by zero");
            }

            self.sp += 4;
            let data2: T = self.read_stack(self.sp);
            self.write_stack(self.sp, data2 / data1);
        }
    }

    fn xmod<T: Copy + std::ops::Rem + PartialEq>(&mut self, zero: T) {
        unsafe {
            let data1: T = self.read_stack(self.sp);
            if data1 == zero {
                panic!("divided by zero");
            }

            self.sp += 4;
            let data2: T = self.read_stack(self.sp);
            self.write_stack(self.sp, data2 % data1);
        }
    }

    fn neg<T: Copy + std::ops::Neg>(&mut self) {
        self.unary_op::<T, _, _>(|a| -a);
    }

    fn cmp<T: Copy + PartialOrd>(&mut self) {
        self.binary_op::<T, _, _>(|a, b| {
            if b.gt(&a) {
                1
            } else if a.gt(&b) {
                -1
            } else {
                0
            }
        })
    }

    fn deref_inc<T: Copy + std::ops::Add>(&mut self, one: T) {
        unsafe {
            let pos: u32 = self.read_stack(self.sp);
            let data: T = self.read_stack(pos as usize);
            self.write_stack(pos as usize, data + one);
        }
    }

    fn deref_dec<T: Copy + std::ops::Sub>(&mut self, one: T) {
        unsafe {
            let pos: u32 = self.read_stack(self.sp);
            let data: T = self.read_stack(pos as usize);
            self.write_stack(pos as usize, data - one);
        }
    }

    fn fild(&mut self) {
        self.unary_op::<i32, _, _>(|a| a as f32);
    }

    fn fst(&mut self) {
        self.unary_op::<f32, _, _>(|a| a as i32);
    }

    fn bnot(&mut self) {
        self.unary_op::<u32, _, _>(|a| !a);
    }

    fn band(&mut self) {
        self.binary_op::<u32, _, _>(|a, b| b & a)
    }

    fn bor(&mut self) {
        self.binary_op::<u32, _, _>(|a, b| b | a)
    }

    fn bxor(&mut self) {
        self.binary_op::<u32, _, _>(|a, b| b ^ a)
    }

    fn shl(&mut self) {
        self.binary_op::<u32, _, _>(|a, b| b << (a & 0xff))
    }

    fn shr(&mut self) {
        self.binary_op::<u32, _, _>(|a, b| b >> (a & 0xff))
    }

    fn sar(&mut self) {
        self.binary_op::<i32, _, _>(|a, b| b >> (a & 0xff))
    }

    fn fuld(&mut self) {
        self.unary_op::<u32, _, _>(|a| a as f32);
    }

    fn truc_to_i8(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a as i8) as i32);
    }

    fn truc_to_i16(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a as i16) as i32);
    }

    fn truc_to_u8(&mut self) {
        self.unary_op::<u32, _, _>(|a| (a as u8) as u32);
    }

    fn truc_to_u16(&mut self) {
        self.unary_op::<u32, _, _>(|a| (a as u16) as u32);
    }

    fn assign_byte(&mut self) {
        self.binary_op::<u32, _, _>(|a, b| (b & 0xFFFFFF00) + (a & 0xFF));
    }

    fn assign_word(&mut self) {
        self.binary_op::<u32, _, _>(|a, b| (b & 0xFFFF0000) + (a & 0xFFFF));
    }

    fn push_zero(&mut self) {
        self.sp -= 4;
        unsafe {
            self.write_stack(self.sp, 0);
        }
    }

    fn memcpy(&mut self, count: u16) {
        unsafe {
            let dst: u32 = self.read_stack(self.sp);
            self.sp += 4;
            let src: u32 = self.read_stack(self.sp);

            for i in 0..count {
                let data: u32 = self.read_stack(src as usize + i as usize);
                self.write_stack(dst as usize + i as usize, data);
            }
        }
    }

    fn push_u64(&mut self, data: u64) {
        unsafe {
            self.sp -= 8;
            self.write_stack(self.sp, data);
        }
    }

    fn store_u64(&mut self) {
        unsafe {
            let pos: u32 = self.read_stack(self.sp);
            self.sp += 4;
            let data: u64 = self.read_stack(self.sp);
            self.write_stack(pos as usize, data);
        }
    }

    fn load_u64(&mut self) {
        unsafe {
            let pos: u32 = self.read_stack(self.sp);
            self.sp -= 4;
            let data: u64 = self.read_stack(pos as usize);
            self.write_stack(self.sp, data);
        }
    }

    fn d2i(&mut self) {
        unsafe {
            let data: f64 = self.read_stack(self.sp);
            self.sp += 4;
            self.write_stack(self.sp, data as i32);
        }
    }

    fn d2f(&mut self) {
        unsafe {
            let data: f64 = self.read_stack(self.sp);
            self.sp += 4;
            self.write_stack(self.sp, data as f32);
        }
    }

    fn x2d<T: Copy + std::convert::Into<f64>>(&mut self) {
        unsafe {
            let data: i32 = self.read_stack(self.sp);
            self.sp += 8;
            self.sp -= std::mem::size_of::<T>();
            self.write_stack(self.sp, data as f64);
        }
    }

    fn jmps(&mut self) {
        unsafe {
            let data: i32 = self.read_stack(self.sp);
            self.sp += 4;
            self.pc += (8 * data) as usize;
        }
    }

    fn store_dword(&mut self) {
        unsafe {
            let data: u32 = self.read_stack(self.sp);
            self.sp += 4;
            self.r1 = data;
        }
    }

    fn store_qword(&mut self) {
        unsafe {
            self.r1 = self.read_stack(self.sp);
            self.sp += 4;
            self.r2 = self.read_stack(self.sp);
            self.sp += 4;
        }
    }

    fn restore_dword(&mut self) {
        unsafe {
            self.sp -= 4;
            self.write_stack(self.sp, self.r1);
        }
    }

    fn restore_qword(&mut self) {
        unsafe {
            self.sp -= 4;
            self.write_stack(self.sp, self.r2);
            self.sp -= 4;
            self.write_stack(self.sp, self.r1);
        }
    }

    fn jgez(&mut self, offset: i32) {
        self.j(offset, |data| data >= 0);
    }

    fn jlz(&mut self, offset: i32) {
        self.j(offset, |data| data < 0);
    }

    fn jlez(&mut self, offset: i32) {
        self.j(offset, |data| data <= 0);
    }

    fn jgz(&mut self, offset: i32) {
        self.j(offset, |data| data > 0);
    }

    fn cmp_imm<T: Copy + PartialOrd>(&mut self, rhs: T) {
        unsafe {
            let data: T = self.read_stack(self.sp);
            self.write_stack(
                self.sp,
                if rhs.gt(&data) {
                    1
                } else if data.gt(&rhs) {
                    -1
                } else {
                    0
                },
            );
        }
    }

    fn add_imm<T: Copy + std::ops::Add>(&mut self, rhs: T) {
        unsafe {
            let data: T = self.read_stack(self.sp);
            self.write_stack(self.sp, data + rhs);
        }
    }

    fn sub_imm<T: Copy + std::ops::Sub>(&mut self, rhs: T) {
        unsafe {
            let data: T = self.read_stack(self.sp);
            self.write_stack(self.sp, data - rhs);
        }
    }

    fn mul_imm<T: Copy + std::ops::Mul>(&mut self, rhs: T) {
        unsafe {
            let data: T = self.read_stack(self.sp);
            self.write_stack(self.sp, data * rhs);
        }
    }

    fn load_global(&mut self, index: i32) {
        let data = if index > 0 {
            let module = self.module.as_mut().unwrap().borrow();
            module.globals[index as usize]
        } else {
            let context = self.context.borrow();
            context.get_global((-index - 1) as usize)
        };

        self.sp -= 4;

        unsafe {
            self.write_stack(self.sp, data);
        }
    }

    fn store_global(&mut self, index: i32) {
        let data: u32 = unsafe { self.read_stack(self.sp) };

        if index > 0 {
            let mut module = self.module.as_mut().unwrap().borrow_mut();
            module.globals[index as usize] = data;
        } else {
            let mut context = self.context.borrow_mut();
            context.set_global((-index - 1) as usize, data);
        };

        self.sp += 4;
    }

    fn get_string(&mut self, index: u16) {
        let module = self.module.as_ref().unwrap().clone();
        let module_ref = module.borrow();
        let string = &module_ref.strings[index as usize];
        unsafe {
            self.sp -= 4;
            self.write_stack(self.sp, index as u32);
            self.sp -= 4;
            self.write_stack(self.sp, string.len());
        }
    }

    #[inline]
    fn j<F: Fn(i32) -> bool>(&mut self, offset: i32, f: F) {
        unsafe {
            let data: i32 = self.read_stack(self.sp);
            if f(data) {
                self.pc += offset as usize;
            }
        }
    }

    #[inline]
    fn unary_op<T: Copy, U, F: Fn(T) -> U>(&mut self, f: F) {
        unsafe {
            let data: T = self.read_stack(self.sp);
            self.write_stack(self.sp, f(data));
        }
    }

    #[inline]
    fn binary_op<T: Copy, U, F: Fn(T, T) -> U>(&mut self, f: F) {
        unsafe {
            let data: T = self.read_stack(self.sp);
            self.sp += std::mem::size_of::<T>();
            let data2: T = self.read_stack(self.sp);
            self.sp += std::mem::size_of::<T>();
            self.sp -= std::mem::size_of::<U>();
            self.write_stack(self.sp, f(data, data2));
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

    pub(super) fn u64(inst: &[u8], pc: &mut usize) -> u64 {
        *pc += 8;
        (&inst[*pc - 8..*pc]).read_u64::<LittleEndian>().unwrap()
    }
}
