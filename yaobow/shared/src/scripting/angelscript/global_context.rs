use super::ScriptVm;

pub struct ScriptGlobalFunction {
    pub name: String,
    pub func: Box<dyn Fn(&str, &mut ScriptVm)>,
}

impl ScriptGlobalFunction {
    pub fn new<S: AsRef<str>>(name: S, func: Box<dyn Fn(&str, &mut ScriptVm)>) -> Self {
        Self {
            name: name.as_ref().to_string(),
            func,
        }
    }
}

pub struct ScriptGlobalContext {
    vars: Vec<u32>,
    functions: Vec<ScriptGlobalFunction>,
}

impl ScriptGlobalContext {
    pub fn new() -> Self {
        Self {
            vars: vec![0; 32],
            functions: Self::system_functions(),
        }
    }

    pub fn register_function(&mut self, function: ScriptGlobalFunction) {
        self.functions.push(function);
    }

    pub fn call_function(&mut self, vm: &mut ScriptVm, index: usize) {
        (self.functions[index].func)(&self.functions[index].name, vm)
    }

    pub fn get_var(&self, index: usize) -> u32 {
        self.vars[index]
    }

    pub fn set_var(&mut self, index: usize, data: u32) {
        self.vars[index] = data;
    }

    fn system_functions() -> Vec<ScriptGlobalFunction> {
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
            ScriptGlobalFunction::new("string.operator=", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.operator+=", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string@", Box::new(not_implemented)),
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
            ScriptGlobalFunction::new("string.AssignIntToString", Box::new(not_implemented)),
            ScriptGlobalFunction::new("string.AddAssignIntToString", Box::new(not_implemented)),
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

fn abs(_: &str, vm: &mut ScriptVm) {
    let number = vm.pop_stack_i32();
    let ret = number.abs();
    vm.push_ret_i32(ret);
}

pub fn not_implemented(name: &str, _: &mut ScriptVm) {
    unimplemented!("unimplemented function called: {}", name);
}
