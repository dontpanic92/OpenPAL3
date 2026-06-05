use std::{cell::RefCell, rc::Rc};

#[cfg(enable_debug)]
use super::debug::{DebugIpcClient, Notification, Request};

use super::{
    global_context::{GlobalFunctionContinuation, ScriptGlobalContext},
    module::{ScriptFunction, ScriptModule},
    trace::{BranchKind, GlobalScope, TraceEvent, TraceEventKind, TraceSink},
};

#[derive(Clone)]
pub(crate) struct ScriptFunctionContext {
    pub(crate) module: Rc<RefCell<ScriptModule>>,
    function_index: usize,
    pc: usize,
}

impl ScriptFunctionContext {
    pub fn new(module: Rc<RefCell<ScriptModule>>, function_index: usize) -> Self {
        Self {
            module,
            function_index,
            pc: 0,
        }
    }
}

pub struct ScriptVm<TAppContext: 'static> {
    pub(crate) app_context: TAppContext,
    pub(crate) g: Rc<RefCell<ScriptGlobalContext<TAppContext>>>,
    pub(crate) context: Option<ScriptFunctionContext>,

    #[cfg(enable_debug)]
    debug_client: DebugIpcClient,

    call_stack: Vec<ScriptFunctionContext>,

    pub(crate) heap: Vec<Option<String>>,
    pub(crate) robj: usize,

    stack: Vec<u8>,
    sp: usize,
    fp: usize,
    r1: u32,
    r2: u32,

    /// Set when a stack access goes out of bounds. The execution loop
    /// checks this and aborts the current script gracefully instead of
    /// letting an out-of-bounds index panic and crash the whole game.
    faulted: std::cell::Cell<bool>,

    yield_func: Vec<GlobalFunctionContinuation<TAppContext>>,
    pub(crate) imm: bool,

    /// Optional execution-trace sink. `None` is the unobserved (zero-
    /// overhead) default. When set, the VM emits a [`TraceEvent`] for
    /// every branch / sysfn call / global read or write — see
    /// `super::trace` for the event reference and the agent-server
    /// integration.
    trace_sink: Option<Rc<dyn TraceSink>>,
    /// Monotonic counter for [`TraceEvent::seq`]. Allocated only
    /// when a sink is installed; held in a `Cell` so the read-only
    /// emit path doesn't need `&mut self`.
    trace_seq: std::cell::Cell<u64>,
}

impl<TAppContext: 'static> ScriptVm<TAppContext> {
    /// Per-VM stack capacity in bytes. The legacy default was 1024,
    /// but PAL4 scripts genuinely allocate large local-variable
    /// frames in late-game cutscenes (Q04 onward) and the operand
    /// stack regularly approaches a few KB during deep call chains.
    /// Underrunning panics with `attempt to subtract with overflow`
    /// in `str()`/`set4()`/etc. when `sp` is already at 0.
    ///
    /// 64 KB matches upstream AngelScript's default initial stack
    /// per context and gives substantial headroom for the planner's
    /// long-running fire-many-triggers sessions. Memory cost is
    /// trivial (one Vec<u8> per VM, and PAL4 only ever has one VM
    /// alive at a time).
    const DEFAULT_STACK_SIZE: usize = 64 * 1024;

    pub fn new(
        g: Rc<RefCell<ScriptGlobalContext<TAppContext>>>,
        module: Rc<RefCell<ScriptModule>>,
        function_index: usize,
        app_context: TAppContext,
    ) -> Self {
        let mut vm = Self {
            app_context,
            g,
            context: Some(ScriptFunctionContext::new(module, function_index)),
            call_stack: vec![],
            heap: vec![],
            r1: 0,
            r2: 0,
            robj: 0,
            yield_func: vec![],

            #[cfg(enable_debug)]
            debug_client: DebugIpcClient::new(),

            stack: vec![0; Self::DEFAULT_STACK_SIZE],
            sp: Self::DEFAULT_STACK_SIZE,
            fp: Self::DEFAULT_STACK_SIZE,

            faulted: std::cell::Cell::new(false),

            imm: true,

            trace_sink: None,
            trace_seq: std::cell::Cell::new(0),
        };

        vm.debug_update_module();
        vm
    }

    pub fn app_context(&self) -> &TAppContext {
        &self.app_context
    }

    pub fn app_context_mut(&mut self) -> &mut TAppContext {
        &mut self.app_context
    }

    pub fn set_function(&mut self, module: Rc<RefCell<ScriptModule>>, index: usize) {
        if self.context.is_some() {
            self.call_stack.push(self.context.clone().unwrap());
        }

        self.context = Some(ScriptFunctionContext::new(module, index));

        self.debug_update_module();
    }

    pub fn set_function_by_name2(&mut self, module: Rc<RefCell<ScriptModule>>, name: &str) {
        for (i, f) in module.borrow().functions.iter().enumerate() {
            if f.name.as_str() == name {
                self.set_function(module.clone(), i)
            }
        }
    }

    /// Name of the function the VM is currently executing, if any.
    /// `None` when the VM is idle between scripts (e.g. waiting for
    /// the director to load the next entry).
    pub fn current_function_name(&self) -> Option<String> {
        let ctx = self.context.as_ref()?;
        let module = ctx.module.borrow();
        module
            .functions
            .get(ctx.function_index)
            .map(|f| f.name.clone())
    }

    /// Install (or replace) the execution-trace sink.
    ///
    /// The hot-path cost while a sink is active is one indirect call
    /// per recorded event plus one `Cell` increment for the
    /// monotonic sequence number. Pass `None` to disable tracing —
    /// the interpreter loop short-circuits on the [`Option`] check.
    pub fn set_trace_sink(&mut self, sink: Option<Rc<dyn TraceSink>>) {
        self.trace_sink = sink;
    }

    /// True once a sink has been installed via
    /// [`Self::set_trace_sink`]. Useful for assertions in tests.
    pub fn has_trace_sink(&self) -> bool {
        self.trace_sink.is_some()
    }

    /// Emit a trace event. Inlined and self-no-op when the sink is
    /// absent so the unobserved path is a single `Option::is_none`.
    #[inline]
    pub(crate) fn trace(&self, kind: TraceEventKind) {
        if let Some(sink) = self.trace_sink.as_ref() {
            let seq = self.trace_seq.get();
            self.trace_seq.set(seq.wrapping_add(1));
            sink.record(TraceEvent { seq, kind });
        }
    }

    /// True when a trace sink is active. Lets opcode handlers skip
    /// the auxiliary work needed to assemble an event (e.g. looking
    /// up sysfn names) when nobody will read it.
    #[inline]
    pub(crate) fn trace_enabled(&self) -> bool {
        self.trace_sink.is_some()
    }

    pub fn stack_peek<T: std::marker::Copy>(&mut self) -> Option<T> {
        if self.sp < self.stack.len() - std::mem::size_of::<T>() {
            let ret: T = unsafe { self.read_stack(self.sp) };
            Some(ret)
        } else {
            None
        }
    }

    pub fn stack_pop<T: std::marker::Copy>(&mut self) -> T {
        let ret: T = unsafe { self.read_stack(self.sp) };
        self.sp += std::mem::size_of::<T>();
        ret
    }

    pub fn stack_push<T: std::marker::Copy>(&mut self, ret: T) {
        self.sp -= std::mem::size_of::<T>();
        unsafe { self.write_stack(self.sp, ret) };
    }

    /// Set the AS "return register" (`r1`) to a value of any
    /// 4-byte-or-less `Copy` type (`i32`, `u32`, `f32`, `u8`, …).
    ///
    /// PAL4's bytecode emits sysfn returns as `CallSys ; Rret4`
    /// (or eventually `Rret8`, though no PAL4 module uses the
    /// 8-byte form). `Rret4` pushes `r1` onto the operand stack
    /// — so a sysfn that wants its return value to be readable
    /// by the script must write `r1` here. **Pushing onto the
    /// operand stack directly (`stack_push`) does not work for
    /// PAL4: the script never reads the stack after a sysfn
    /// call, and the stale push leaks the operand stack until
    /// it eventually underflows.** See AS calling-convention
    /// notes in CLAUDE.md.
    ///
    /// We tile the supplied bytes into the low end of `r1`
    /// (little-endian on every Rust target we support), zero-
    /// extending if `T` is smaller than 4 bytes. The `Sret8` /
    /// `Rret8` pair is unused by PAL4 so we don't provide an
    /// `r2` setter.
    pub fn set_ret_value<T: std::marker::Copy>(&mut self, value: T) {
        let size = std::mem::size_of::<T>();
        assert!(
            size <= std::mem::size_of::<u32>(),
            "set_ret_value: T must fit in the 4-byte AS r1 register"
        );
        let mut buf: u32 = 0;
        unsafe {
            std::ptr::copy_nonoverlapping(
                &value as *const T as *const u8,
                &mut buf as *mut u32 as *mut u8,
                size,
            );
        }
        self.r1 = buf;
    }

    /// Aborts the currently-executing script by clearing the
    /// execution context and call stack, the same way the stack-fault
    /// recovery path does in `execute()`. Use this from a sysfn
    /// handler when continuing would cascade into a panic — e.g.
    /// `giArenaLoad` failing means the surrounding cutscene's
    /// follow-up `giPlayer*` calls would dereference actors in a
    /// scene that was never loaded. Leaves the VM ready to accept a
    /// fresh `set_function*` call (the agent server / trigger
    /// dispatch / next `giArenaLoad` will re-enter naturally).
    pub fn abort_script(&mut self) {
        log::warn!(
            "AngelScript VM script aborted from sysfn handler (fn={} sp={} fp={})",
            self.current_fn_name(),
            self.sp,
            self.fp,
        );
        self.context = None;
        self.call_stack.clear();
        self.yield_func.clear();
    }

    pub fn push_object(&mut self, object: String) -> usize {
        for i in 0..self.heap.len() {
            if self.heap[i].is_none() {
                self.heap[i] = Some(object);
                return i;
            }
        }

        self.heap.push(Some(object));
        return self.heap.len() - 1;
    }

    pub fn execute(&mut self, delta_sec: f32) {
        loop {
            if self.context.is_none() {
                return;
            }

            if self.faulted.get() {
                log::error!(
                    "AngelScript VM aborting script after stack fault in fn={} (sp={} fp={}); \
                     clearing execution context to keep the game running.",
                    self.current_fn_name(),
                    self.sp,
                    self.fp,
                );
                self.faulted.set(false);
                self.context = None;
                self.call_stack.clear();
                self.yield_func.clear();
                return;
            }

            let module = self.context.as_ref().unwrap().module.clone();
            let module_ref = module.borrow();
            let function =
                module_ref.functions[self.context.as_ref().unwrap().function_index].clone();
            let mut reg: u32 = 0;

            self.debug_update_context();
            self.wait_for_action();

            let mut wait = false;
            let mut new_funcs = vec![];
            while let Some(mut cont) = self.yield_func.pop() {
                match cont(self, delta_sec) {
                    crate::scripting::angelscript::ContinuationState::Loop => {
                        new_funcs.push(cont);
                        wait = true;
                    }
                    crate::scripting::angelscript::ContinuationState::Concurrent => {
                        new_funcs.push(cont);
                    }
                    crate::scripting::angelscript::ContinuationState::Completed => {}
                }
            }

            self.yield_func = new_funcs;

            if wait {
                return;
            }

            let inst = self.read_inst(&function);
            macro_rules! command {
                ($cmd_name: ident $(, $param_name: ident : $param_type: ident)*) => {{
                    $(let $param_name = data_read::$param_type(&function.inst, &mut self.context.as_mut().unwrap().pc);)*
                    self.$cmd_name($($param_name ,)*);
                }};

                ($cmd_name: ident : $g_type: ident $(, $param_name: ident : $param_type: ident)*) => {{
                    $(let $param_name = data_read::$param_type(&function.inst, &mut self.context.as_mut().unwrap().pc);)*
                    self.$cmd_name::<$g_type>($($param_name)*);
                }};
            }

            match inst {
                0 => command!(pop, size: u16),
                1 => command!(push, size: u16),
                2 => command!(set4, size: u32),
                3 => self.rd4(),
                4 => command!(rdsf4, index: u16),
                5 => self.wrt4(),
                6 => self.mov4(),
                7 => command!(psf, index: u16),
                8 => command!(movsf4, index: u16),
                9 => self.swap::<u32>(),
                10 => self.store4(&mut reg),
                11 => self.recall4(reg),
                12 => command!(call, function: u32),
                13 => {
                    command!(ret, param_size: u16);
                    return;
                }
                14 => command!(jmp, offset: i32),
                15 => command!(jz, offset: i32),
                16 => command!(jnz, offset: i32),
                17 => self.tz(),
                18 => self.tnz(),
                19 => self.ts_ltz(),
                20 => self.tns_gez(),
                21 => self.tp_gtz(),
                22 => self.tnp_lez(),
                23 => self.add::<i32>(),
                24 => self.sub::<i32>(),
                25 => self.mul::<i32>(),
                26 => self.div::<i32>(0),
                27 => self.xmod::<i32>(0),
                28 => self.neg::<i32>(),
                29 => self.cmp::<i32>(),
                30 => self.inc::<i32>(1),
                31 => self.dec::<i32>(1),
                32 => self.i2f(),
                33 => self.add::<f32>(),
                34 => self.sub::<f32>(),
                35 => self.mul::<f32>(),
                36 => self.div::<f32>(0.),
                37 => self.xmod::<f32>(0.),
                38 => self.neg::<f32>(),
                39 => self.cmp::<f32>(),
                40 => self.inc::<f32>(1.),
                41 => self.dec::<f32>(1.),
                42 => self.f2i(),
                43 => self.bnot(),
                44 => self.band(),
                45 => self.bor(),
                46 => self.bxor(),
                47 => self.bsll(),
                48 => self.bsrl(),
                49 => self.bsra(),
                50 => self.ui2f(),
                51 => self.f2ui(),
                52 => self.cmp::<u32>(),
                53 => self.sb(),
                54 => self.sw(),
                55 => self.ub(),
                56 => self.uw(),
                57 => self.wrt1(),
                58 => self.wrt2(),
                59 => self.inc::<i16>(1),
                60 => self.inc::<i8>(1),
                61 => self.dec::<i16>(1),
                62 => self.dec::<i8>(1),
                63 => self.push_zero(),
                64 => command!(copy, count: u16),
                65 => command!(pga, index: i32),
                66 => command!(set8, data: u64),
                67 => self.wrt8(),
                68 => self.rd8(),
                69 => self.neg::<f64>(),
                70 => self.inc::<f64>(1.),
                71 => self.dec::<f64>(1.),
                72 => self.add::<f64>(),
                73 => self.sub::<f64>(),
                74 => self.mul::<f64>(),
                75 => self.div::<f64>(0.),
                76 => self.xmod::<f64>(0.),
                77 => self.swap::<f64>(),
                78 => self.cmp::<f64>(),
                79 => self.d2i(),
                80 => self.d2ui(),
                81 => self.d2f(),
                82 => self.x2d::<i32>(),
                83 => self.x2d::<u32>(),
                84 => self.x2d::<f32>(),
                85 => self.jmpp(),
                86 => self.sret4(),
                87 => self.sret8(),
                88 => self.rret4(),
                89 => self.rret8(),
                90 => command!(str, index: u16),
                91 => command!(js_jgez, offset: i32),
                92 => command!(jns_jlz, offset: i32),
                93 => command!(jp_jlez, offset: i32),
                94 => command!(jnp_jgz, offset: i32),
                95 => command!(cmpi: i32, rhs: i32),
                96 => command!(cmpi: u32, rhs: u32),
                97 => {
                    command!(callsys, function_index: i32);
                    /*if self.yield_func.is_some() {
                        return;
                    }*/
                    return;
                }
                98 => command!(callbnd, function_index: u32),
                99 => command!(rdga4, index: i32),
                100 => command!(movga4, index: i32),
                101 => command!(addi: i32, rhs: i32),
                102 => command!(subi: i32, rhs: i32),
                103 => command!(cmpi: f32, rhs: f32),
                104 => command!(addi: f32, rhs: f32),
                105 => command!(subi: f32, rhs: f32),
                106 => command!(muli: i32, rhs: i32),
                107 => command!(muli: f32, rhs: f32),
                108 => {
                    // Suspend
                    if self.trace_enabled() {
                        let fn_name = self.current_fn_name();
                        let pc = self.context.as_ref().map(|c| c.pc).unwrap_or(0);
                        self.trace(TraceEventKind::Suspend { fn_name, pc });
                    }
                    return;
                }
                109 => command!(alloc, this: i32, index: i32),
                110 => command!(free, obj_type: u32),
                111 => unimplemented!("byte code 111 - loadobj"),
                112 => command!(storeobj, param_index: i16),
                113 => unimplemented!("byte code 113 - getobj"),
                114 => unimplemented!("byte code 114 - refcpy"),
                115 => self.checkref(),
                116 => self.rd1(),
                117 => self.rd2(),
                118 => command!(getobjref, offset: i16),
                119 => unimplemented!("byte code 119 - getref"),
                120 => unimplemented!("byte code 120 - swap48"),
                121 => unimplemented!("byte code 121 - swap84"),
                122 => unimplemented!("byte code 122 - objtype"),
                i => unimplemented!("byte code {}", i),
            }
        }
    }

    fn as_trace_enabled() -> bool {
        static FLAG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *FLAG.get_or_init(|| {
            std::env::var("YAOBOW_AS_TRACE")
                .map(|v| !v.is_empty() && v != "0")
                .unwrap_or(false)
        })
    }

    fn current_fn_name(&self) -> String {
        match self.context.as_ref() {
            Some(ctx) => {
                let module = ctx.module.borrow();
                module
                    .functions
                    .get(ctx.function_index)
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| format!("#<{}>", ctx.function_index))
            }
            None => "<none>".to_string(),
        }
    }

    #[cold]
    fn dump_stack_context(&self, op: &str, pos: usize, size: usize) {
        log::error!(
            "AngelScript VM stack out-of-bounds during {op}: pos={pos} size={size} \
             stack_len={} sp={} fp={} fn={} pc={} call_depth={}",
            self.stack.len(),
            self.sp,
            self.fp,
            self.current_fn_name(),
            self.context.as_ref().map(|c| c.pc).unwrap_or(0),
            self.call_stack.len(),
        );
    }

    fn read_inst(&mut self, function: &ScriptFunction) -> u8 {
        let inst = function.inst[self.context.as_ref().unwrap().pc];
        self.context.as_mut().unwrap().pc += 4;
        inst
    }

    fn pop(&mut self, size: u16) {
        self.sp += size as usize * 4;
    }

    fn push(&mut self, size: u16) {
        self.sp -= size as usize * 4;
    }

    fn set4(&mut self, data: u32) {
        self.sp -= 4;
        unsafe {
            self.write_stack(self.sp, data);
        }
    }

    fn rd4(&mut self) {
        unsafe {
            let pos: u32 = self.read_stack(self.sp);
            let data: u32 = self.read_stack(pos as usize);
            self.write_stack(self.sp, data);
        }
    }

    /// `Rd1` / `Rd2` — variable-width reads matching `Rd4`'s pattern.
    /// Reads a 1/2-byte value from the address held in stack[sp]
    /// and overwrites that stack slot with the zero-extended u32.
    /// PAL4 emits `Rd1` after `giGetVisibleObject` and similar
    /// sysfns that return a single-byte bool / count; without an
    /// implementation the VM panics with "byte code 116 - rd1"
    /// the moment any script tries to inspect such a return value
    /// (first observed when the planner reached M07/1).
    fn rd1(&mut self) {
        unsafe {
            let pos: u32 = self.read_stack(self.sp);
            let data: u8 = self.read_stack(pos as usize);
            self.write_stack::<u32>(self.sp, data as u32);
        }
    }

    fn rd2(&mut self) {
        unsafe {
            let pos: u32 = self.read_stack(self.sp);
            let data: u16 = self.read_stack(pos as usize);
            self.write_stack::<u32>(self.sp, data as u32);
        }
    }

    fn rdsf4(&mut self, index: u16) {
        unsafe {
            let data: u32 = self.read_stack(self.stack.len() - index as usize * 4);
            self.write_stack(self.sp, data);
        }
    }

    fn wrt4(&mut self) {
        unsafe {
            let pos: u32 = self.read_stack(self.sp);
            self.sp += 4;
            let data: u32 = self.read_stack(self.sp);
            self.write_stack(pos as usize, data);
        }
    }

    fn mov4(&mut self) {
        self.wrt4();
        self.sp += 4;
    }

    fn psf(&mut self, index: u16) {
        unsafe {
            let pos = self.fp - index as usize * 4;
            self.sp -= 4;
            self.write_stack(self.sp, pos as u32);
        }
    }

    fn movsf4(&mut self, index: u16) {
        unsafe {
            let pos = self.fp - index as usize * 4;
            let data: u32 = self.read_stack(self.sp);
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

    fn store4(&mut self, reg: &mut u32) {
        unsafe {
            let data = self.read_stack(self.sp);
            *reg = data;
        }
    }

    fn recall4(&mut self, reg: u32) {
        unsafe {
            self.sp -= 4;
            self.write_stack(self.sp, reg);
        }
    }

    fn call(&mut self, function: u32) {
        if Self::as_trace_enabled() {
            log::info!(
                "[as] call fn#{} from {} sp={} fp={} depth={}",
                function,
                self.current_fn_name(),
                self.sp,
                self.fp,
                self.call_stack.len(),
            );
        }
        let module = self.context.as_ref().unwrap().module.clone();
        self.set_function(module, function as usize);
        if self.trace_enabled() {
            let name = self.current_fn_name();
            self.trace(TraceEventKind::FnEnter {
                name,
                function_index: function as usize,
                depth: self.call_stack.len(),
            });
        }
    }

    fn callbnd(&mut self, function: u32) {
        println!("Unimplemented: call: {}", function);
    }

    fn rdga4(&mut self, offset: i32) {
        // See `pga` for the index encoding. Tolerant fallback so that
        // unrecognised module globals evaluate to 0 instead of
        // crashing the engine.
        let (data, scope, slot) = if offset < 0 {
            let index = -offset - 1;
            let context = self.g.clone();
            let value = context
                .borrow()
                .vars
                .get(index as usize)
                .copied()
                .unwrap_or(0);
            (value, GlobalScope::Shared, index as u32)
        } else {
            let context = self.context.as_ref().unwrap();
            let module = context.module.borrow();
            let value = module.globals.get(offset as usize).copied().unwrap_or(0);
            (value, GlobalScope::Module, offset as u32)
        };
        if self.trace_enabled() {
            let fn_name = self.current_fn_name();
            let pc = self.context.as_ref().map(|c| c.pc).unwrap_or(0);
            self.trace(TraceEventKind::GlobalRead {
                fn_name,
                pc,
                scope,
                slot,
                value: data,
            });
        }
        self.set4(data);
    }

    fn callsys(&mut self, function: i32) {
        let index = -function - 1;
        let trace_log = Self::as_trace_enabled();
        let trace_sink = self.trace_enabled();
        let pc = self.context.as_ref().map(|c| c.pc).unwrap_or(0);
        let (name, sp_before) = if trace_log || trace_sink {
            let name = self
                .g
                .borrow()
                .functions()
                .get(index as usize)
                .map(|f| f.name.clone())
                .unwrap_or_else(|| format!("#<{}>", index));
            (name, self.sp)
        } else {
            (String::new(), 0)
        };
        let caller_fn = if trace_sink {
            self.current_fn_name()
        } else {
            String::new()
        };
        let context = self.g.clone();
        let context = context.borrow();
        match context.call_function(self, index as usize) {
            super::GlobalFunctionState::Yield(cont) => self.yield_func.push(cont),
            super::GlobalFunctionState::Completed => {}
        }
        drop(context);
        if trace_log {
            log::info!(
                "[as] callsys {} idx={} sp {}->{} delta={} fp={} robj={}",
                name,
                index,
                sp_before,
                self.sp,
                self.sp as i64 - sp_before as i64,
                self.fp,
                self.robj,
            );
        }
        if trace_sink {
            let sp_after = self.sp;
            let r1_after = self.r1;
            self.trace(TraceEventKind::CallSys {
                fn_name: caller_fn,
                pc,
                sysfn_index: index as usize,
                sysfn_name: name,
                sp_before,
                sp_after,
                r1_after,
            });
        }
    }

    fn alloc(&mut self, this: i32, function: i32) {
        println!("Unimplemented: call global2: {} {}", this, function);
    }

    fn storeobj(&mut self, param_index: i16) {
        unsafe {
            self.write_stack(
                (self.fp as isize - param_index as isize * 4) as usize,
                self.robj as u32,
            );
        }
    }

    fn free(&mut self, _obj_type: u32) {
        let obj_ref: u32 = unsafe { self.read_stack(self.sp) };
        let obj_index: u32 = unsafe { self.read_stack(obj_ref as usize) };
        self.sp += 4;
        self.heap[obj_index as usize] = None;
    }

    fn checkref(&mut self) {}

    fn getobjref(&mut self, offset: i16) {
        unsafe {
            let addr = (self.sp as isize + offset as isize * 4) as usize;
            let index: u32 = self.read_stack(addr);
            let objref: u32 = self.read_stack((self.fp as isize - index as isize * 4) as usize);
            self.write_stack(addr, objref);
        }
    }

    fn ret(&mut self, param_size: u16) {
        if Self::as_trace_enabled() {
            log::info!(
                "[as] ret from {} param_size={} sp={} fp={} depth={}",
                self.current_fn_name(),
                param_size,
                self.sp,
                self.fp,
                self.call_stack.len(),
            );
        }
        if self.trace_enabled() {
            let name = self.current_fn_name();
            // `call_stack` holds parent frames only; the active
            // frame about to be popped lives in `self.context`.
            // Reporting `call_stack.len()` therefore gives the
            // depth of the frame that's leaving.
            let depth = self.call_stack.len();
            self.trace(TraceEventKind::FnExit { name, depth });
        }
        let func = self.call_stack.pop();
        self.context = func;

        self.sp -= param_size as usize;
    }

    fn jz(&mut self, offset: i32) {
        unsafe {
            let data: i32 = self.read_stack(self.sp);
            self.sp += 4;
            let taken = data == 0;
            if self.trace_enabled() {
                let fn_name = self.current_fn_name();
                let pc = self.context.as_ref().map(|c| c.pc).unwrap_or(0);
                self.trace(TraceEventKind::Branch {
                    fn_name,
                    pc,
                    kind: BranchKind::Jz,
                    operand: data,
                    offset,
                    taken,
                });
            }
            if taken {
                self.jmp(offset);
            }
        }
    }

    fn jnz(&mut self, offset: i32) {
        unsafe {
            let data: i32 = self.read_stack(self.sp);
            self.sp += 4;
            let taken = data != 0;
            if self.trace_enabled() {
                let fn_name = self.current_fn_name();
                let pc = self.context.as_ref().map(|c| c.pc).unwrap_or(0);
                self.trace(TraceEventKind::Branch {
                    fn_name,
                    pc,
                    kind: BranchKind::Jnz,
                    operand: data,
                    offset,
                    taken,
                });
            }
            if taken {
                self.jmp(offset);
            }
        }
    }

    fn jmp(&mut self, offset: i32) {
        if offset < 0 && Self::as_trace_enabled() {
            let target = self
                .context
                .as_ref()
                .unwrap()
                .pc
                .wrapping_add(offset as usize);
            log::info!(
                "[as] backjmp fn={} target_pc={} sp={} fp={}",
                self.current_fn_name(),
                target,
                self.sp,
                self.fp,
            );
        }
        // `offset` is a signed byte-offset relative to the
        // post-fetch pc. Use `wrapping_add` after casting through
        // `usize` so backward jumps (negative offsets) don't
        // overflow the unsigned pc — sufficient because pc is
        // bounded by the instruction-buffer length and the AS
        // assembler emits in-bounds offsets.
        let ctx = self.context.as_mut().unwrap();
        ctx.pc = ctx.pc.wrapping_add(offset as usize);
    }

    fn tz(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a == 0) as i32);
    }

    fn tnz(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a != 0) as i32);
    }

    fn ts_ltz(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a < 0) as i32);
    }

    fn tns_gez(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a >= 0) as i32);
    }

    fn tp_gtz(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a > 0) as i32);
    }

    fn tnp_lez(&mut self) {
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

    fn inc<T: Copy + std::ops::Add>(&mut self, one: T) {
        unsafe {
            let pos: u32 = self.read_stack(self.sp);
            let data: T = self.read_stack(pos as usize);
            self.write_stack(pos as usize, data + one);
        }
    }

    fn dec<T: Copy + std::ops::Sub>(&mut self, one: T) {
        unsafe {
            let pos: u32 = self.read_stack(self.sp);
            let data: T = self.read_stack(pos as usize);
            self.write_stack(pos as usize, data - one);
        }
    }

    fn i2f(&mut self) {
        self.unary_op::<i32, _, _>(|a| a as f32);
    }

    fn f2i(&mut self) {
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

    fn bsll(&mut self) {
        self.binary_op::<u32, _, _>(|a, b| b << (a & 0xff))
    }

    fn bsrl(&mut self) {
        self.binary_op::<u32, _, _>(|a, b| b >> (a & 0xff))
    }

    fn bsra(&mut self) {
        self.binary_op::<i32, _, _>(|a, b| b >> (a & 0xff))
    }

    fn ui2f(&mut self) {
        self.unary_op::<u32, _, _>(|a| a as f32);
    }

    fn f2ui(&mut self) {
        self.unary_op::<f32, _, _>(|a| a as u32);
    }

    fn sb(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a as i8) as i32);
    }

    fn sw(&mut self) {
        self.unary_op::<i32, _, _>(|a| (a as i16) as i32);
    }

    fn ub(&mut self) {
        self.unary_op::<u32, _, _>(|a| (a as u8) as u32);
    }

    fn uw(&mut self) {
        self.unary_op::<u32, _, _>(|a| (a as u16) as u32);
    }

    fn wrt1(&mut self) {
        self.binary_op::<u32, _, _>(|a, b| (b & 0xFFFFFF00) + (a & 0xFF));
    }

    fn wrt2(&mut self) {
        self.binary_op::<u32, _, _>(|a, b| (b & 0xFFFF0000) + (a & 0xFFFF));
    }

    fn push_zero(&mut self) {
        self.sp -= 4;
        unsafe {
            self.write_stack(self.sp, 0u32);
        }
    }

    fn copy(&mut self, count: u16) {
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

    fn set8(&mut self, data: u64) {
        unsafe {
            self.sp -= 8;
            self.write_stack(self.sp, data);
        }
    }

    fn rd8(&mut self) {
        unsafe {
            let pos: u32 = self.read_stack(self.sp);
            self.sp += 4;
            let data: u64 = self.read_stack(self.sp);
            self.write_stack(pos as usize, data);
        }
    }

    fn wrt8(&mut self) {
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

    fn d2ui(&mut self) {
        unsafe {
            let data: f64 = self.read_stack(self.sp);
            self.sp += 4;
            self.write_stack(self.sp, data as u32);
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

    fn jmpp(&mut self) {
        unsafe {
            let data: i32 = self.read_stack(self.sp);
            self.sp += 4;
            self.context.as_mut().unwrap().pc += (8 * data) as usize;
        }
    }

    fn sret4(&mut self) {
        unsafe {
            let data: u32 = self.read_stack(self.sp);
            self.sp += 4;
            self.r1 = data;
        }
    }

    fn sret8(&mut self) {
        unsafe {
            self.r1 = self.read_stack(self.sp);
            self.sp += 4;
            self.r2 = self.read_stack(self.sp);
            self.sp += 4;
        }
    }

    fn rret4(&mut self) {
        unsafe {
            self.sp -= 4;
            self.write_stack(self.sp, self.r1);
        }
    }

    fn rret8(&mut self) {
        unsafe {
            self.sp -= 4;
            self.write_stack(self.sp, self.r2);
            self.sp -= 4;
            self.write_stack(self.sp, self.r1);
        }
    }

    fn js_jgez(&mut self, offset: i32) {
        // Opcode 91 is `Js` (AngelScript "jump if signed"): fire the
        // branch when the comparison result on top of stack is `< 0`.
        // The pair-name `js_jgez` is preserved for backwards-compat
        // with the externalised trace `BranchKind` (`docs/agent_interface.md`)
        // — the predicate matches upstream AngelScript's `asBC_JS`
        // (`l_bc` advances on `value < 0`).
        self.j_traced(offset, BranchKind::JsJgez, |data| data < 0);
    }

    fn jns_jlz(&mut self, offset: i32) {
        // Opcode 92 is `Jns` (AngelScript "jump if not signed"):
        // fire the branch when the comparison result is `>= 0`. Pair
        // name retained for trace compatibility — see `js_jgez`.
        self.j_traced(offset, BranchKind::JnsJlz, |data| data >= 0);
    }

    fn jp_jlez(&mut self, offset: i32) {
        // Opcode 93 is `Jp` (AngelScript "jump if positive"): fire
        // the branch when the comparison result is `> 0`. Pair name
        // retained for trace compatibility.
        self.j_traced(offset, BranchKind::JpJlez, |data| data > 0);
    }

    fn jnp_jgz(&mut self, offset: i32) {
        // Opcode 94 is `Jnp` (AngelScript "jump if not positive"):
        // fire the branch when the comparison result is `<= 0`. Pair
        // name retained for trace compatibility.
        self.j_traced(offset, BranchKind::JnpJgz, |data| data <= 0);
    }

    fn cmpi<T: Copy + PartialOrd>(&mut self, rhs: T) {
        // PAL4's `Cmpii` / `Cmpi` (opcodes 95/96/103) pops `lhs` from
        // the operand stack and writes back `sign(lhs - rhs)`. The
        // result is then consumed by a following `Js`/`Jns`/`Jp`/`Jnp`
        // (or `Jz`/`Jnz`), so the arithmetic semantics here must
        // match upstream AngelScript's `asBC_CMPIi` (`as_context.cpp`)
        // and our own `js_jgez/jns_jlz/jp_jlez/jnp_jgz` predicates:
        //
        //   * `lhs <  rhs` → `-1`
        //   * `lhs == rhs` →  `0`
        //   * `lhs >  rhs` → `+1`
        //
        // This used to be sign-inverted (`rhs > data → +1`). At the
        // time the conditional-jump helpers were *also* inverted, so
        // the two errors cancelled out for `Cmpii + Jcc` chains. When
        // commit a5e694f restored the jumps to the upstream predicates
        // (locked by `signed_jumps_match_upstream_angelscript_semantics`),
        // `Cmpii` was left upside-down and gates like
        // `q01/func1001`'s "shared[0] < 11400" plot guard started
        // taking the wrong leg — sending the party from Q01/Q01 to
        // Q01/N02 immediately after the first plot instead of back to
        // Q01/N01 on the fresh-save plot path.
        unsafe {
            let data: T = self.read_stack(self.sp);
            self.write_stack(
                self.sp,
                if data.gt(&rhs) {
                    1
                } else if rhs.gt(&data) {
                    -1
                } else {
                    0
                },
            );
        }
    }

    fn addi<T: Copy + std::ops::Add>(&mut self, rhs: T) {
        unsafe {
            let data: T = self.read_stack(self.sp);
            self.write_stack(self.sp, data + rhs);
        }
    }

    fn subi<T: Copy + std::ops::Sub>(&mut self, rhs: T) {
        unsafe {
            let data: T = self.read_stack(self.sp);
            self.write_stack(self.sp, data - rhs);
        }
    }

    fn muli<T: Copy + std::ops::Mul>(&mut self, rhs: T) {
        unsafe {
            let data: T = self.read_stack(self.sp);
            self.write_stack(self.sp, data * rhs);
        }
    }

    fn pga(&mut self, index: i32) {
        // PAL4's bytecode encodes shared engine globals as **negative**
        // indices (`-(slot + 1)` so that slot 0 ↔ -1, slot 1 ↔ -2, …)
        // and module-local globals as **positive** indices. `index ==
        // 0` is ambiguous in this scheme: if treated as the negative
        // branch it underflows to `-1 as u32 == u32::MAX` and the
        // shared-global Vec (capacity 48) panics with an OOB. We
        // observed this at M07/8 where some scripted `Pga(0)` reached
        // get_global with u32::MAX. Treat `index == 0` as "module
        // slot 0" (the most common interpretation; if the module
        // didn't ship any globals we just read 0 / write into a
        // freshly-grown slot — the failure mode for an unrecognised
        // global is "evaluate to 0", not "crash the engine").
        let (data, scope, slot) = if index > 0 {
            let context = self.context.as_ref().unwrap();
            let module = context.module.borrow();
            let idx = index as usize;
            let value = module.globals.get(idx).copied().unwrap_or(0);
            (value, GlobalScope::Module, index as u32)
        } else if index == 0 {
            let context = self.context.as_ref().unwrap();
            let module = context.module.borrow();
            let value = module.globals.first().copied().unwrap_or(0);
            (value, GlobalScope::Module, 0)
        } else {
            let context = self.g.borrow();
            let slot = (-index - 1) as u32;
            let value = context.vars.get(slot as usize).copied().unwrap_or(0);
            (value, GlobalScope::Shared, slot)
        };

        if self.trace_enabled() {
            let fn_name = self.current_fn_name();
            let pc = self.context.as_ref().map(|c| c.pc).unwrap_or(0);
            self.trace(TraceEventKind::GlobalRead {
                fn_name,
                pc,
                scope,
                slot,
                value: data,
            });
        }

        self.sp -= 4;

        unsafe {
            self.write_stack(self.sp, data);
        }
    }

    fn movga4(&mut self, index: i32) {
        let data: u32 = unsafe { self.read_stack(self.sp) };

        // See `pga` for the index encoding. We mirror its tolerance:
        // `index == 0` writes to module slot 0 (growing the Vec if
        // necessary), not to shared slot u32::MAX (which would OOB
        // the 48-slot vars Vec and crash the engine).
        let (scope, slot) = if index > 0 {
            let context = self.context.as_mut().unwrap();
            let mut module = context.module.borrow_mut();
            let idx = index as usize;
            if module.globals.len() <= idx {
                module.globals.resize(idx + 1, 0);
            }
            module.globals[idx] = data;
            (GlobalScope::Module, index as u32)
        } else if index == 0 {
            let context = self.context.as_mut().unwrap();
            let mut module = context.module.borrow_mut();
            if module.globals.is_empty() {
                module.globals.push(0);
            }
            module.globals[0] = data;
            (GlobalScope::Module, 0)
        } else {
            let mut context = self.g.borrow_mut();
            let slot = (-index - 1) as u32;
            context.set_global(slot as usize, data);
            (GlobalScope::Shared, slot)
        };

        if self.trace_enabled() {
            let fn_name = self.current_fn_name();
            let pc = self.context.as_ref().map(|c| c.pc).unwrap_or(0);
            self.trace(TraceEventKind::GlobalWrite {
                fn_name,
                pc,
                scope,
                slot,
                value: data,
            });
        }

        self.sp += 4;
    }

    fn str(&mut self, index: u16) {
        let module = self.context.as_ref().unwrap().module.clone();
        let module_ref = module.borrow();
        let string = &module_ref.strings[index as usize];
        unsafe {
            self.sp -= 4;
            self.write_stack(self.sp, index as u32);
            self.sp -= 4;
            self.write_stack(self.sp, string.len() as u32);
        }
    }

    #[inline]
    fn j_traced<F: Fn(i32) -> bool>(&mut self, offset: i32, kind: BranchKind, f: F) {
        unsafe {
            // PAL4's AngelScript variant places the result of the
            // preceding `Cmpii` / `Cmpi` / `Subi` on the operand
            // stack (upstream AS uses `m_regs.valueRegister`
            // instead). The conditional jump consumes that result,
            // so `sp` has to advance past it the same way `jz` /
            // `jnz` do above (line ~772, ~795). Forgetting the
            // advance leaks 4 bytes per `Js`/`Jns`/`Jp`/`Jnp` and
            // shifts every subsequent push downwards on the operand
            // stack — eventually causing arg-marshalling mismatches
            // in `string@` / `CallSys` and bizarre underflow
            // panics in long-running scripts.
            let data: i32 = self.read_stack(self.sp);
            self.sp += 4;
            let taken = f(data);
            if self.trace_enabled() {
                let fn_name = self.current_fn_name();
                let pc = self.context.as_ref().map(|c| c.pc).unwrap_or(0);
                self.trace(TraceEventKind::Branch {
                    fn_name,
                    pc,
                    kind,
                    operand: data,
                    offset,
                    taken,
                });
            }
            if taken {
                self.context.as_mut().unwrap().pc += offset as usize;
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
        unsafe {
            let size = std::mem::size_of::<T>();
            if pos >= self.stack.len() || size > self.stack.len() - pos {
                self.dump_stack_context("write_stack", pos, size);
                self.faulted.set(true);
                return;
            }
            // Use `ptr::write_unaligned` rather than `*ptr = data` because
            // `self.stack` is a `Vec<u8>` and `pos` may land at any
            // alignment. Rust ≥1.87 enforces alignment preconditions on
            // raw-pointer deref, which previously silently worked but now
            // panics with "misaligned pointer dereference" (e.g. an i32
            // store at sp=0x...f). Same fix applies in `read_stack`.
            std::ptr::write_unaligned(self.stack.as_mut_ptr().add(pos) as *mut T, data);
        }
    }

    #[inline]
    unsafe fn read_stack<T: Copy>(&self, pos: usize) -> T {
        unsafe {
            let size = std::mem::size_of::<T>();
            if pos >= self.stack.len() || size > self.stack.len() - pos {
                self.dump_stack_context("read_stack", pos, size);
                self.faulted.set(true);
                return std::mem::zeroed();
            }
            std::ptr::read_unaligned(self.stack.as_ptr().add(pos) as *const T)
        }
    }

    fn debug_update_module(&mut self) {
        #[cfg(enable_debug)]
        {
            let _ = self.debug_client.notify(Notification::ModuleChanged {
                module: self
                    .context
                    .as_ref()
                    .and_then(|f| Some(f.module.borrow().clone())),
                function: self
                    .context
                    .as_ref()
                    .and_then(|f| Some(f.function_index as u32))
                    .unwrap_or(0),
            });

            let _ = self
                .debug_client
                .notify(Notification::GlobalFunctionsChanged(
                    self.g
                        .borrow()
                        .functions
                        .iter()
                        .map(|f| f.name.clone())
                        .collect(),
                ));
        }
    }

    fn debug_update_context(&mut self) {
        #[cfg(enable_debug)]
        {
            let _ = self
                .debug_client
                .notify(Notification::ObjectsChanged(self.heap.clone()));
            let _ = self.debug_client.notify(Notification::RegisterChanged {
                pc: self.context.as_ref().and_then(|f| Some(f.pc)).unwrap_or(0),
                sp: self.sp,
                fp: self.fp,
                r1: self.r1,
                r2: self.r2,
                object_register: self.robj,
            });

            let _ = self
                .debug_client
                .notify(Notification::StackChanged(self.stack.clone()));
        }
    }

    fn wait_for_action(&mut self) {
        #[cfg(enable_debug)]
        {
            let _ = self.debug_client.call(Request::WaitForAction);
        }
    }
}

pub(crate) mod data_read {
    use byteorder::{LittleEndian, ReadBytesExt};

    pub(crate) fn u16(inst: &[u8], pc: &mut usize) -> u16 {
        *pc += 2;
        (&inst[*pc - 2..*pc]).read_u16::<LittleEndian>().unwrap()
    }

    pub(crate) fn i16(inst: &[u8], pc: &mut usize) -> i16 {
        *pc += 2;
        (&inst[*pc - 2..*pc]).read_i16::<LittleEndian>().unwrap()
    }

    pub(crate) fn i32(inst: &[u8], pc: &mut usize) -> i32 {
        *pc += 4;
        (&inst[*pc - 4..*pc]).read_i32::<LittleEndian>().unwrap()
    }

    pub(crate) fn u32(inst: &[u8], pc: &mut usize) -> u32 {
        *pc += 4;
        (&inst[*pc - 4..*pc]).read_u32::<LittleEndian>().unwrap()
    }

    pub(crate) fn f32(inst: &[u8], pc: &mut usize) -> f32 {
        *pc += 4;
        (&inst[*pc - 4..*pc]).read_f32::<LittleEndian>().unwrap()
    }

    pub(crate) fn u64(inst: &[u8], pc: &mut usize) -> u64 {
        *pc += 8;
        (&inst[*pc - 8..*pc]).read_u64::<LittleEndian>().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::super::ScriptGlobalContext;
    use super::super::module::{ScriptFunction, ScriptModule};
    use super::super::trace::{
        BranchKind, GlobalScope, TraceEventKind, TraceSink, test_support::VecSink,
    };
    use super::ScriptVm;

    /// Build a 4-byte-aligned opcode header (opcode byte + 3 pad).
    fn op(code: u8) -> [u8; 4] {
        [code, 0, 0, 0]
    }

    /// Concatenate raw-byte instruction fragments into a `Vec<u8>`
    /// matching the encoding `ScriptVm::execute` expects.
    fn assemble(parts: &[&[u8]]) -> Vec<u8> {
        let mut v = Vec::new();
        for p in parts {
            v.extend_from_slice(p);
        }
        v
    }

    fn build_vm(inst: Vec<u8>) -> ScriptVm<()> {
        let function = ScriptFunction::test_function("test_main", inst);
        let module = ScriptModule::test_module(vec![function]);
        let g = Rc::new(RefCell::new(ScriptGlobalContext::<()>::new()));
        ScriptVm::new(g, Rc::new(RefCell::new(module)), 0, ())
    }

    #[test]
    fn no_sink_records_nothing_observable() {
        // Set4 42; Movga4 -1 (write to shared global 0); Suspend.
        let inst = assemble(&[
            &op(2),
            &42u32.to_le_bytes(),
            &op(100),
            &(-1i32).to_le_bytes(),
            &op(108),
        ]);
        let mut vm = build_vm(inst);
        assert!(!vm.has_trace_sink());
        vm.execute(0.0);
        // Effect on global must still happen even without a sink.
        assert_eq!(vm.g.borrow().get_global(0), 42);
    }

    #[test]
    fn global_write_event_carries_slot_and_value() {
        // Set4 11400; Movga4 -3 (write shared global slot 2); Suspend.
        let inst = assemble(&[
            &op(2),
            &11_400u32.to_le_bytes(),
            &op(100),
            &(-3i32).to_le_bytes(),
            &op(108),
        ]);
        let mut vm = build_vm(inst);
        let sink = Rc::new(VecSink::new());
        vm.set_trace_sink(Some(sink.clone() as Rc<dyn TraceSink>));
        vm.execute(0.0);

        let events = sink.snapshot();
        // Sequence numbers monotonic from 0.
        for (i, ev) in events.iter().enumerate() {
            assert_eq!(ev.seq, i as u64, "seq must be monotonic");
        }

        // Expect at minimum: GlobalWrite{Shared, slot=2, value=11400}
        // and a trailing Suspend, in that order.
        let mut write_seen = false;
        let mut suspend_seen = false;
        for ev in &events {
            match &ev.kind {
                TraceEventKind::GlobalWrite {
                    scope: GlobalScope::Shared,
                    slot,
                    value,
                    ..
                } => {
                    assert_eq!(*slot, 2);
                    assert_eq!(*value, 11_400);
                    write_seen = true;
                }
                TraceEventKind::Suspend { .. } => {
                    assert!(write_seen, "Suspend must follow the GlobalWrite");
                    suspend_seen = true;
                }
                _ => {}
            }
        }
        assert!(write_seen, "missing GlobalWrite event: {:?}", events);
        assert!(suspend_seen, "missing Suspend event: {:?}", events);
    }

    #[test]
    fn global_read_event_carries_observed_value() {
        // Pre-seed shared global 1 with 99.
        // Pga -2 (read global slot 1 -> stack); Suspend.
        let inst = assemble(&[&op(65), &(-2i32).to_le_bytes(), &op(108)]);
        let mut vm = build_vm(inst);
        vm.g.borrow_mut().set_global(1, 99);
        let sink = Rc::new(VecSink::new());
        vm.set_trace_sink(Some(sink.clone() as Rc<dyn TraceSink>));
        vm.execute(0.0);

        let events = sink.snapshot();
        let read = events
            .iter()
            .find_map(|ev| match &ev.kind {
                TraceEventKind::GlobalRead {
                    scope: GlobalScope::Shared,
                    slot,
                    value,
                    ..
                } => Some((*slot, *value)),
                _ => None,
            })
            .expect("missing GlobalRead event");
        assert_eq!(read, (1, 99));
    }

    #[test]
    fn jz_branch_records_operand_and_taken() {
        // Two runs of:
        //   Set4 <flag>; Jz <offset>; Suspend; Ret 0;
        // - flag=0 -> taken=true, jumps past Suspend into Ret so
        //   the VM unwinds cleanly to idle.
        // - flag=1 -> taken=false, falls through into Suspend.
        // Either way the sink should record exactly one Jz Branch
        // event with the correct operand and `taken` flag.
        for &(flag, expected_taken) in &[(0u32, true), (1u32, false)] {
            let inst = assemble(&[
                &op(2),
                &flag.to_le_bytes(),
                &op(15),             // Jz
                &4i32.to_le_bytes(), // offset = 4 bytes => skips Suspend
                &op(108),            // Suspend (terminates non-taken path)
                &op(13),             // Ret
                &0u16.to_le_bytes(), // param_size = 0
            ]);
            let mut vm = build_vm(inst);
            let sink = Rc::new(VecSink::new());
            vm.set_trace_sink(Some(sink.clone() as Rc<dyn TraceSink>));
            vm.execute(0.0);

            let branches: Vec<_> = sink
                .snapshot()
                .into_iter()
                .filter_map(|ev| match ev.kind {
                    TraceEventKind::Branch {
                        kind,
                        operand,
                        taken,
                        ..
                    } => Some((kind, operand, taken)),
                    _ => None,
                })
                .collect();
            assert_eq!(branches.len(), 1, "flag={}: {:?}", flag, branches);
            assert_eq!(branches[0].0, BranchKind::Jz);
            assert_eq!(branches[0].1, flag as i32);
            assert_eq!(branches[0].2, expected_taken, "flag={}", flag);
        }
    }

    /// Boundary-case coverage for the signed conditional jumps
    /// (`Js`/`Jns`/`Jp`/`Jnp` — opcodes 91..=94). Each one is fed
    /// the three salient operand values (`-1`, `0`, `+1`) and the
    /// expected `taken` outcome is asserted against upstream
    /// AngelScript semantics (`as_context.cpp`):
    ///   * `Js`  → taken iff `data < 0`
    ///   * `Jns` → taken iff `data >= 0`
    ///   * `Jp`  → taken iff `data > 0`
    ///   * `Jnp` → taken iff `data <= 0`
    /// Regression coverage for the long-latent inversion bug where
    /// each helper checked the opposite condition and PAL4
    /// `q01/func1001`'s plot-progression chain dead-ended (PLOT
    /// BLOCK B unreachable, blocking the "return home with 古玉"
    /// scene). Also asserts the sink records exactly one Branch
    /// event per jump so the agent-trace consumer stays in sync.
    #[test]
    fn signed_jumps_match_upstream_angelscript_semantics() {
        let cases: &[(u8, BranchKind, &str, &[(i32, bool)])] = &[
            (
                91,
                BranchKind::JsJgez,
                "Js",
                &[(-1, true), (0, false), (1, false)],
            ),
            (
                92,
                BranchKind::JnsJlz,
                "Jns",
                &[(-1, false), (0, true), (1, true)],
            ),
            (
                93,
                BranchKind::JpJlez,
                "Jp",
                &[(-1, false), (0, false), (1, true)],
            ),
            (
                94,
                BranchKind::JnpJgz,
                "Jnp",
                &[(-1, true), (0, true), (1, false)],
            ),
        ];
        for &(opcode, expected_kind, name, vectors) in cases {
            for &(operand, expected_taken) in vectors {
                let inst = assemble(&[
                    &op(2),
                    &operand.to_le_bytes(),
                    &op(opcode),
                    &4i32.to_le_bytes(),
                    &op(108),
                    &op(13),
                    &0u16.to_le_bytes(),
                ]);
                let mut vm = build_vm(inst);
                let sink = Rc::new(VecSink::new());
                vm.set_trace_sink(Some(sink.clone() as Rc<dyn TraceSink>));
                vm.execute(0.0);

                let branches: Vec<_> = sink
                    .snapshot()
                    .into_iter()
                    .filter_map(|ev| match ev.kind {
                        TraceEventKind::Branch {
                            kind,
                            operand,
                            taken,
                            ..
                        } => Some((kind, operand, taken)),
                        _ => None,
                    })
                    .collect();
                assert_eq!(
                    branches.len(),
                    1,
                    "{} operand={}: {:?}",
                    name,
                    operand,
                    branches
                );
                assert_eq!(branches[0].0, expected_kind, "{} operand={}", name, operand);
                assert_eq!(branches[0].1, operand, "{} operand={}", name, operand);
                assert_eq!(
                    branches[0].2, expected_taken,
                    "{} operand={} expected taken={}",
                    name, operand, expected_taken
                );
            }
        }
    }

    /// `Jz`/`Jnz` pop their operand off the stack (vm.rs ~770/793)
    /// — `Js`/`Jns`/`Jp`/`Jnp` must do the same. Without that
    /// `sp += 4` the comparison result accumulates on the operand
    /// stack and shifts every subsequent push downwards, corrupting
    /// `string@`/`CallSys` arg marshalling. We construct a tiny
    /// program that pushes a sentinel before the jump and a `Set4`
    /// after; the sentinel must be reachable at the SAME stack
    /// slot the post-`Set4` value occupies after the jump consumes
    /// its operand. We sample stack depth via `Cmpii` + `Jz`: the
    /// second branch's `operand` reads the value at the new `sp`,
    /// which equals the sentinel iff the signed jump popped.
    #[test]
    fn signed_jumps_pop_operand_off_stack() {
        // Layout (operand `1` is `Js`-NOT-taken so the jump is a
        // pure stack-pop):
        //   Set4 sentinel(0x1234abcd)  ; push sentinel
        //   Set4 1                     ; push +1 as jump operand
        //   Js +0                      ; pops +1 (taken=false: data<0)
        //   Cmpii 0x1234abcd           ; pops sentinel; pushes 0 (eq)
        //   Jz +4                      ; pops 0, taken=true -> skips Suspend
        //   Suspend
        //   Ret 0
        const SENTINEL: i32 = 0x1234abcdu32 as i32;
        let inst = assemble(&[
            &op(2),
            &SENTINEL.to_le_bytes(),
            &op(2),
            &1i32.to_le_bytes(),
            &op(91),
            &0i32.to_le_bytes(),
            &op(95),
            &SENTINEL.to_le_bytes(),
            &op(15),
            &4i32.to_le_bytes(),
            &op(108),
            &op(13),
            &0u16.to_le_bytes(),
        ]);
        let mut vm = build_vm(inst);
        let sink = Rc::new(VecSink::new());
        vm.set_trace_sink(Some(sink.clone() as Rc<dyn TraceSink>));
        vm.execute(0.0);

        let branches: Vec<_> = sink
            .snapshot()
            .into_iter()
            .filter_map(|ev| match ev.kind {
                TraceEventKind::Branch {
                    kind,
                    operand,
                    taken,
                    ..
                } => Some((kind, operand, taken)),
                _ => None,
            })
            .collect();
        assert_eq!(branches.len(), 2, "{:?}", branches);
        // Js consumed the +1 operand without taking.
        assert_eq!(branches[0], (BranchKind::JsJgez, 1, false));
        // If Js popped, Cmpii then compared the sentinel against
        // itself and pushed 0, so the following Jz fires.
        assert_eq!(branches[1], (BranchKind::Jz, 0, true));
    }

    /// `Cmpii` (opcode 95 / 96 / 103) must write back `sign(lhs - rhs)`
    /// matching upstream AngelScript (`asBC_CMPIi` in
    /// `as_context.cpp`) — i.e. `-1` when `lhs < rhs`, `0` on equal,
    /// `+1` when `lhs > rhs`. The conditional-jump helpers
    /// (`Js<0`/`Jns>=0`/`Jp>0`/`Jnp<=0`, locked by
    /// `signed_jumps_match_upstream_angelscript_semantics`) consume
    /// that result directly, so a sign inversion here de-syncs every
    /// `Cmpii + Jcc` gate in the shipped PAL4 scripts.
    ///
    /// Regression coverage for the Q01/Q01 → Q01/N01 plot-return bug:
    /// `q01/func1001`'s entry gate is `Rdga4 -1 ; Cmpii 11400 ; Jns
    /// +N`, and with a fresh save (`shared[0] == 0`) it MUST fall
    /// through to Path A (`arena_load Q01/N01`). Pre-fix `Cmpii`
    /// returned `+1` for `0 < 11400`, the `Jns >= 0` predicate took
    /// the wrong leg, and the party landed in `Q01/N02` instead.
    #[test]
    fn cmpi_matches_upstream_angelscript_semantics() {
        // Run `Set4 lhs ; Cmpii(opcode) rhs ; Js +N ; Suspend ; Ret`
        // and inspect the recorded `Js` Branch event. `Js` is taken
        // iff cmpi pushed `-1`; the trace's `operand` field is the
        // raw value popped, so we get both "taken" and "operand"
        // assertions for free.
        struct Case {
            opcode: u8, // 95 = i32, 96 = u32, 103 = f32
            name: &'static str,
            lhs: [u8; 4],
            rhs: [u8; 4],
            expected_sign: i32,
        }
        let cases = [
            // i32 — the case that broke Q01.
            Case {
                opcode: 95,
                name: "i32 lhs<rhs",
                lhs: 0i32.to_le_bytes(),
                rhs: 11_400i32.to_le_bytes(),
                expected_sign: -1,
            },
            Case {
                opcode: 95,
                name: "i32 lhs==rhs",
                lhs: 11_400i32.to_le_bytes(),
                rhs: 11_400i32.to_le_bytes(),
                expected_sign: 0,
            },
            Case {
                opcode: 95,
                name: "i32 lhs>rhs",
                lhs: 11_500i32.to_le_bytes(),
                rhs: 11_400i32.to_le_bytes(),
                expected_sign: 1,
            },
            // i32 with negative operands — guards against accidental
            // unsigned comparison.
            Case {
                opcode: 95,
                name: "i32 negative lhs<rhs",
                lhs: (-5i32).to_le_bytes(),
                rhs: 3i32.to_le_bytes(),
                expected_sign: -1,
            },
            Case {
                opcode: 95,
                name: "i32 lhs>negative rhs",
                lhs: 3i32.to_le_bytes(),
                rhs: (-5i32).to_le_bytes(),
                expected_sign: 1,
            },
            // u32 — same numeric outcome but exercises the
            // monomorphisation specialised on the unsigned generic.
            Case {
                opcode: 96,
                name: "u32 lhs<rhs",
                lhs: 0u32.to_le_bytes(),
                rhs: 11_400u32.to_le_bytes(),
                expected_sign: -1,
            },
            Case {
                opcode: 96,
                name: "u32 lhs>rhs",
                lhs: 11_500u32.to_le_bytes(),
                rhs: 11_400u32.to_le_bytes(),
                expected_sign: 1,
            },
            // f32 — covers float Cmpi (opcode 103) used by camera /
            // movement scripts.
            Case {
                opcode: 103,
                name: "f32 lhs<rhs",
                lhs: 1.5f32.to_le_bytes(),
                rhs: 2.5f32.to_le_bytes(),
                expected_sign: -1,
            },
            Case {
                opcode: 103,
                name: "f32 lhs==rhs",
                lhs: 2.5f32.to_le_bytes(),
                rhs: 2.5f32.to_le_bytes(),
                expected_sign: 0,
            },
            Case {
                opcode: 103,
                name: "f32 lhs>rhs",
                lhs: 3.0f32.to_le_bytes(),
                rhs: 2.5f32.to_le_bytes(),
                expected_sign: 1,
            },
        ];

        for case in &cases {
            let inst = assemble(&[
                &op(2),
                &case.lhs,
                &op(case.opcode),
                &case.rhs,
                &op(91), // Js
                &4i32.to_le_bytes(),
                &op(108), // Suspend
                &op(13),
                &0u16.to_le_bytes(),
            ]);
            let mut vm = build_vm(inst);
            let sink = Rc::new(VecSink::new());
            vm.set_trace_sink(Some(sink.clone() as Rc<dyn TraceSink>));
            vm.execute(0.0);

            let branches: Vec<_> = sink
                .snapshot()
                .into_iter()
                .filter_map(|ev| match ev.kind {
                    TraceEventKind::Branch {
                        kind,
                        operand,
                        taken,
                        ..
                    } => Some((kind, operand, taken)),
                    _ => None,
                })
                .collect();
            assert_eq!(
                branches.len(),
                1,
                "{}: expected exactly one Js branch, got {:?}",
                case.name,
                branches
            );
            let (kind, operand, taken) = branches[0];
            assert_eq!(kind, BranchKind::JsJgez, "{}", case.name);
            assert_eq!(
                operand, case.expected_sign,
                "{}: cmpi pushed wrong sign",
                case.name
            );
            assert_eq!(
                taken,
                case.expected_sign < 0,
                "{}: Js predicate disagrees with cmpi result",
                case.name
            );
        }
    }

    /// End-to-end regression for the Q01/Q01 → Q01/N01 plot-return
    /// bug. Models the actual entry gate of `q01/func1001`:
    ///
    /// ```text
    /// Rdga4 -1            ; push shared[0]
    /// Cmpii 11400         ; push sign(shared[0] - 11400)
    /// Jns +N              ; if shared[0] >= 11400, jump to PATH_B
    /// Set4 PATH_A         ; push PATH_A marker
    /// Jmp +M              ; skip past PATH_B Set4
    /// Set4 PATH_B         ; Jns target
    /// Movga4 -2           ; pop top -> shared[1] (post-execute inspection)
    /// Suspend             ; yield with context intact
    /// ```
    ///
    /// We assert that with `shared[0] == 0` we land in PATH_A (the
    /// "send the party back to Q01/N01" leg), and with
    /// `shared[0] == 11400` / `11500` we land in PATH_B (the "advance
    /// to Q01/N02" leg). The exact `Rdga4 -1 ; Cmpii 11400 ; Jns +N`
    /// triplet is the entry gate of `q01.csb::func1001` as dumped by
    /// `pal4_plot_dump --debug-fn func1001`; the same shape guards
    /// every Q01 plot transition.
    #[test]
    fn q01_func1001_gate_routes_low_plot_to_path_a() {
        const PATH_A: u32 = 0xA0A0_A0A0;
        const PATH_B: u32 = 0xB0B0_B0B0;

        // Byte offsets (each Set4 / Cmpii / Jcc / Jmp / Rdga4 / Movga4
        // is 8 bytes; Suspend is 4):
        //   0x00..0x08  Rdga4 -1
        //   0x08..0x10  Cmpii 11400
        //   0x10..0x18  Jns +16   ; post-fetch pc = 0x18, target = 0x28
        //   0x18..0x20  Set4 PATH_A
        //   0x20..0x28  Jmp  +8   ; post-fetch pc = 0x28, target = 0x30
        //   0x28..0x30  Set4 PATH_B           (Jns target)
        //   0x30..0x38  Movga4 -2             (Jmp target / Path A fall-through)
        //   0x38..0x3c  Suspend
        let assemble_program = || -> Vec<u8> {
            assemble(&[
                &op(99),
                &(-1i32).to_le_bytes(),
                &op(95),
                &11_400i32.to_le_bytes(),
                &op(92),
                &16i32.to_le_bytes(),
                &op(2),
                &PATH_A.to_le_bytes(),
                &op(14),
                &8i32.to_le_bytes(),
                &op(2),
                &PATH_B.to_le_bytes(),
                &op(100),
                &(-2i32).to_le_bytes(),
                &op(108),
            ])
        };

        let cases = [(0i32, PATH_A), (11_400, PATH_B), (11_500, PATH_B)];
        for &(plot_state, expected_marker) in &cases {
            let mut vm = build_vm(assemble_program());
            vm.g.borrow_mut().set_global(0, plot_state as u32);
            vm.execute(0.0);

            let chosen = vm.g.borrow().get_global(1);
            assert_eq!(
                chosen, expected_marker,
                "shared[0]={} should route to marker 0x{:08X}, got 0x{:08X}",
                plot_state, expected_marker, chosen
            );
        }
    }
}
