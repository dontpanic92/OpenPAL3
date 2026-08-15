use super::ScriptVm;

pub type GlobalFunctionContinuation<TAppContext> =
    Box<dyn FnMut(&mut ScriptVm<TAppContext>, f32) -> ContinuationState>;

pub enum GlobalFunctionState<TAppContext: 'static> {
    Yield(GlobalFunctionContinuation<TAppContext>),
    Completed,
}

pub enum ContinuationState {
    Loop,
    Concurrent,
    Completed,
}

pub struct ScriptGlobalFunction<TAppContext: 'static> {
    pub name: String,
    pub func: Box<dyn Fn(&str, &mut ScriptVm<TAppContext>) -> GlobalFunctionState<TAppContext>>,
}

impl<TAppContext: 'static> ScriptGlobalFunction<TAppContext> {
    pub fn new<S: AsRef<str>>(
        name: S,
        func: Box<dyn Fn(&str, &mut ScriptVm<TAppContext>) -> GlobalFunctionState<TAppContext>>,
    ) -> Self {
        Self {
            name: name.as_ref().to_string(),
            func,
        }
    }
}

#[macro_export]
macro_rules! as_params {
    ($vm: ident $(, $param_name: ident : $param_type: ident)*) => {
        $(let $param_name = $vm.stack_pop::<$param_type>();)*
    }
}

pub struct ScriptGlobalContext<TAppContext: 'static> {
    pub(crate) vars: Vec<u32>,
    pub(crate) functions: Vec<ScriptGlobalFunction<TAppContext>>,
}

impl<TAppContext: 'static> ScriptGlobalContext<TAppContext> {
    pub fn new() -> Self {
        Self {
            vars: vec![0; 48],
            functions: Self::system_functions(),
        }
    }

    pub fn register_function(&mut self, function: ScriptGlobalFunction<TAppContext>) {
        self.functions.push(function);
    }

    pub fn call_function(
        &self,
        vm: &mut ScriptVm<TAppContext>,
        index: usize,
    ) -> GlobalFunctionState<TAppContext> {
        log::debug!("Calling: {}", self.functions[index].name);
        (self.functions[index].func)(&self.functions[index].name, vm)
    }

    pub fn functions(&self) -> &[ScriptGlobalFunction<TAppContext>] {
        &self.functions
    }

    pub fn get_global(&self, index: usize) -> u32 {
        self.vars[index]
    }

    pub fn set_global(&mut self, index: usize, data: u32) {
        self.vars[index] = data;
    }

    /// Snapshot all shared global variables. Used by the PAL4 save
    /// system to persist cross-scene story-plot flags.
    pub fn globals_snapshot(&self) -> Vec<u32> {
        self.vars.clone()
    }

    /// Restore previously snapshotted global variables. Only as many
    /// slots as currently exist are overwritten, so a save taken with
    /// a different `vars` length still loads safely.
    pub fn restore_globals(&mut self, globals: &[u32]) {
        let len = self.vars.len().min(globals.len());
        self.vars[..len].copy_from_slice(&globals[..len]);
    }

    fn system_functions() -> Vec<ScriptGlobalFunction<TAppContext>> {
        vec![
            ScriptGlobalFunction::new("ArrayObjectConstructor_Generic", Box::new(not_implemented)),
            ScriptGlobalFunction::new("ArrayObjectConstructor2_Generic", Box::new(not_implemented)),
            ScriptGlobalFunction::new("GCObject_AddRef_Generic", Box::new(not_implemented)),
            ScriptGlobalFunction::new("GCObject_Release_Generic", Box::new(not_implemented)),
            ScriptGlobalFunction::new("ArrayObjectAssignment_Generic", Box::new(not_implemented)),
            ScriptGlobalFunction::new("ArrayObjectAt_Generic", Box::new(not_implemented)),
            ScriptGlobalFunction::new("ArrayObjectAt_Generic", Box::new(not_implemented)),
            ScriptGlobalFunction::new("ArrayObjectLength_Generic", Box::new(not_implemented)),
            ScriptGlobalFunction::new("ArrayObjectResize_Generic", Box::new(not_implemented)),
            ScriptGlobalFunction::new("ScriptStruct_Construct_Generic", Box::new(not_implemented)),
            ScriptGlobalFunction::new("GCObject_AddRef_Generic", Box::new(not_implemented)),
            ScriptGlobalFunction::new("GCObject_Release_Generic", Box::new(not_implemented)),
            ScriptGlobalFunction::new("ScriptStruct_Assignment_Generic", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.ConstructString", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.AddRef", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.Release", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.operator=", Box::new(string_assign)),
            ScriptGlobalFunction::new("string.operator+=", Box::new(string_add_assign)),
            ScriptGlobalFunction::new("string@", Box::new(string_factory)),
            ScriptGlobalFunction::new("string::operator==", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string::operator!=", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string::operator<=", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string::operator>=", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string::operator <", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string::operator >", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string::operator +", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.charat", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.chatat_const", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.length", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.AssignDoubleToString", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.AddAssignDoubleToString", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string::AddStringDouble", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string::AddDoubleString", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.AssignIntToString", Box::new(string_assign_int)),
            ScriptGlobalFunction::new(
                "string.AddAssignIntToString",
                Box::new(string_add_assign_int),
            ),
            ScriptGlobalFunction::new("string::AddStringInt", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string::AddIntString", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.AssignUIntToString", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.AddAssignUIntToString", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string::AddStringUInt", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string::AddUIntString", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.AssignBitsToString", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.AddAssignBitsToString", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string::AddStringBits", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string::AddBitsString", Box::new(not_implemented)),
            ScriptGlobalFunction::new("abs", Box::new(abs)),
            ScriptGlobalFunction::new("fabs", Box::new(not_implemented)),
            ScriptGlobalFunction::new("sqrtf", Box::new(not_implemented)),
            ScriptGlobalFunction::new("sinf", Box::new(not_implemented)),
            ScriptGlobalFunction::new("cosf", Box::new(not_implemented)),
            ScriptGlobalFunction::new("acosf", Box::new(not_implemented)),
            ScriptGlobalFunction::new("asinf", Box::new(not_implemented)),
            ScriptGlobalFunction::new("tanf", Box::new(not_implemented)),
            ScriptGlobalFunction::new("atanf", Box::new(not_implemented)),
        ]
    }
}

fn abs<TAppContext>(_: &str, vm: &mut ScriptVm<TAppContext>) -> GlobalFunctionState<TAppContext> {
    as_params!(vm, number: i32);

    let ret = number.abs();
    vm.stack_push::<i32>(ret);

    GlobalFunctionState::Completed
}

fn string_factory<TAppContext>(
    _: &str,
    vm: &mut ScriptVm<TAppContext>,
) -> GlobalFunctionState<TAppContext> {
    as_params!(vm, _len: u32, str_id: u32);
    let string = vm.context.as_ref().unwrap().module.borrow().strings[str_id as usize].clone();
    let ret = vm.push_object(string);

    vm.robj = ret;

    GlobalFunctionState::Completed
}

/// AngelScript `string` member operators.
///
/// PAL4's block scripts build object names at runtime — `M06`'s
/// periodic `func9001` does `string s = "item"; s += n;` to sweep the
/// dungeon's numbered floor covers — so these have to work or the
/// script dies on the first assignment.
///
/// Calling convention (observed from PAL4 bytecode): the callee pops
/// only the *value* operand; the `this` pointer stays on the stack
/// below it, doubling as the returned `string&` reference that the
/// caller later consumes (typically with `FREE`). An object operand is
/// pushed either as its heap index or as the address of the stack slot
/// holding that index, so both are resolved through
/// [`ScriptVm::resolve_object_index`].
fn string_assign<TAppContext>(
    _: &str,
    vm: &mut ScriptVm<TAppContext>,
) -> GlobalFunctionState<TAppContext> {
    string_binary_op(vm, |dst, src| *dst = src)
}

fn string_add_assign<TAppContext>(
    _: &str,
    vm: &mut ScriptVm<TAppContext>,
) -> GlobalFunctionState<TAppContext> {
    string_binary_op(vm, |dst, src| dst.push_str(&src))
}

fn string_binary_op<TAppContext>(
    vm: &mut ScriptVm<TAppContext>,
    apply: impl FnOnce(&mut String, String),
) -> GlobalFunctionState<TAppContext> {
    let src_word: u32 = vm.stack_pop();
    let Some(dst_word) = vm.stack_peek::<u32>() else {
        log::warn!("string operator: missing `this` pointer");
        return GlobalFunctionState::Completed;
    };

    let Some(dst_index) = vm.resolve_object_index(dst_word) else {
        log::warn!("string operator: unresolvable destination {:#x}", dst_word);
        return GlobalFunctionState::Completed;
    };

    // The source operand reaches us one of two ways. PAL4's bytecode
    // loads it onto the stack when it is a plain variable, but for the
    // very common `s = "literal"` it leaves the temporary produced by
    // the `string@` factory in the object register and the stack slot
    // ends up naming the destination again. Prefer the stack operand
    // when it denotes a *different* object; otherwise take the object
    // register.
    let stack_src = vm
        .resolve_object_index(src_word)
        .filter(|i| *i != dst_index);
    let src_index = stack_src.or_else(|| vm.get_object(vm.robj).map(|_| vm.robj));
    let Some(src) = src_index.and_then(|i| vm.get_object(i).cloned()) else {
        log::warn!("string operator: unresolvable source {:#x}", src_word);
        return GlobalFunctionState::Completed;
    };

    let mut value = vm.get_object(dst_index).cloned().unwrap_or_default();
    apply(&mut value, src);
    vm.set_object(dst_index, value);
    vm.robj = dst_index;

    GlobalFunctionState::Completed
}

/// `string &opAssign(int)` — the non-appending sibling of
/// [`string_add_assign_int`].
fn string_assign_int<TAppContext>(
    _: &str,
    vm: &mut ScriptVm<TAppContext>,
) -> GlobalFunctionState<TAppContext> {
    let value: i32 = vm.stack_pop();
    let Some(dst_word) = vm.stack_peek::<u32>() else {
        log::warn!("string.AssignIntToString: missing `this` pointer");
        return GlobalFunctionState::Completed;
    };

    let Some(dst_index) = vm.resolve_object_index(dst_word) else {
        log::warn!(
            "string.AssignIntToString: unresolvable destination {:#x}",
            dst_word
        );
        return GlobalFunctionState::Completed;
    };

    vm.set_object(dst_index, value.to_string());
    vm.robj = dst_index;

    GlobalFunctionState::Completed
}

/// `string &opAddAssign(int)` — appends the decimal form of an
/// integer, the other half of the `"item" + n` name-building pattern.
fn string_add_assign_int<TAppContext>(
    _: &str,
    vm: &mut ScriptVm<TAppContext>,
) -> GlobalFunctionState<TAppContext> {
    let value: i32 = vm.stack_pop();
    let Some(dst_word) = vm.stack_peek::<u32>() else {
        log::warn!("string.AddAssignIntToString: missing `this` pointer");
        return GlobalFunctionState::Completed;
    };

    let Some(dst_index) = vm.resolve_object_index(dst_word) else {
        log::warn!(
            "string.AddAssignIntToString: unresolvable destination {:#x}",
            dst_word
        );
        return GlobalFunctionState::Completed;
    };

    let mut string = vm.get_object(dst_index).cloned().unwrap_or_default();
    string.push_str(&value.to_string());
    vm.set_object(dst_index, string);
    vm.robj = dst_index;

    GlobalFunctionState::Completed
}

pub fn not_implemented<TAppContext>(
    name: &str,
    _: &mut ScriptVm<TAppContext>,
) -> GlobalFunctionState<TAppContext> {
    panic!("unimplemented function called: {}", name);

    // GlobalFunctionState::Completed
}
