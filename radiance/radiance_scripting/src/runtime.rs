use std::panic::AssertUnwindSafe;

use crosscom::{ComInterface, ComRc};
use crosscom_protosept::{
    install_com_dispatcher, scope, ComObjectTable, HostError, HostServices, P7HostContext,
};
use p7::interpreter::context::Data;
use p7::{InMemoryModuleProvider, ModuleProvider};

pub struct RuntimeServices {
    pub com: ComObjectTable,
}

impl Default for RuntimeServices {
    fn default() -> Self {
        Self {
            com: ComObjectTable::new(),
        }
    }
}

impl HostServices for RuntimeServices {
    fn com_table_mut(&mut self) -> &mut ComObjectTable {
        &mut self.com
    }
}

pub struct ScriptRuntime {
    host: P7HostContext<RuntimeServices>,
    state: Option<Data>,
}

impl ScriptRuntime {
    pub fn new() -> Self {
        let mut host = P7HostContext::new(RuntimeServices::default());
        install_com_dispatcher(&mut host.ctx);
        Self { host, state: None }
    }

    pub fn load_source(&mut self, source: &str) -> Result<(), HostError> {
        let module = p7::compile_with_provider(source.to_string(), binding_provider())
            .map_err(|err| HostError::message(format!("p7 compile failed: {:?}", err)))?;
        self.host.ctx.load_module(module);
        Ok(())
    }

    pub fn load_default_bindings(&mut self) -> Result<(), HostError> {
        self.load_source(include_str!(concat!(
            env!("OUT_DIR"),
            "/editor_services.p7"
        )))?;
        self.load_source(include_str!(concat!(env!("OUT_DIR"), "/scripting.p7")))
    }

    pub fn intern<I: ComInterface + 'static>(&mut self, rc: ComRc<I>) -> i64 {
        self.host.services.com_table_mut().intern(rc)
    }

    /// Returns true if the loaded user package defines a function named `name`.
    /// Use this to gate optional script callbacks (e.g. `activate`) before
    /// invoking `call_void` / `call_returning_*`, which would otherwise panic
    /// inside `Context::push_function` on a missing name.
    pub fn has_function(&self, name: &str) -> bool {
        self.host.ctx.has_function(name)
    }

    pub fn with_ctx<R>(&self, body: impl FnOnce(&p7::interpreter::context::Context) -> R) -> R {
        body(&self.host.ctx)
    }

    pub fn foreign_box(&mut self, type_tag: &str, handle: i64) -> Result<Data, HostError> {
        self.host
            .ctx
            .push_foreign(type_tag, handle)
            .map_err(|err| HostError::message(format!("push foreign failed: {:?}", err)))?;
        self.host.ctx.stack[0]
            .stack
            .pop()
            .ok_or_else(|| HostError::message("push foreign produced no stack value"))
    }

    pub fn call_void(&mut self, name: &str, args: Vec<Data>) -> Result<(), HostError> {
        let depth = self.host.ctx.stack[0].stack.len();
        self.call_inner(name, args)?;
        if self.host.ctx.stack[0].stack.len() > depth {
            let _ = self.host.ctx.stack[0].stack.pop();
        }
        Ok(())
    }

    pub fn call_returning_data(&mut self, name: &str, args: Vec<Data>) -> Result<Data, HostError> {
        let depth = self.host.ctx.stack[0].stack.len();
        self.call_inner(name, args)?;
        self.host.ctx.stack[0].stack.pop().ok_or_else(|| {
            HostError::message(format!(
                "function '{name}' returned no value (stack depth before call {depth})"
            ))
        })
    }

    pub fn call_returning_optional_com<I: ComInterface + 'static>(
        &mut self,
        name: &str,
        args: Vec<Data>,
    ) -> Result<Option<ComRc<I>>, HostError> {
        let result = self.call_returning_data(name, args)?;
        let handle = match result {
            Data::Null => return Ok(None),
            Data::Some(inner) => self.foreign_handle(*inner)?,
            other => self.foreign_handle(other)?,
        };
        self.host
            .services
            .com_table_mut()
            .get::<I>(handle)
            .map(Some)
            .ok_or_else(|| {
                HostError::message(format!(
                    "COM handle {handle} does not expose requested interface"
                ))
            })
    }

    pub fn store_state(&mut self, state: Data) {
        self.state = Some(state);
    }

    pub fn state_clone(&self) -> Option<Data> {
        self.state.clone()
    }

    fn call_inner(&mut self, name: &str, args: Vec<Data>) -> Result<(), HostError> {
        if !self.host.ctx.has_function(name) {
            return Err(HostError::message(format!(
                "script function '{name}' is not defined in the loaded package"
            )));
        }
        let host = &mut self.host;
        std::panic::catch_unwind(AssertUnwindSafe(|| {
            let P7HostContext { ctx, services } = host;
            scope(services, || {
                ctx.push_function(name, args);
                ctx.resume()
            })
        }))
        .map_err(|_| HostError::message(format!("script function '{name}' panicked")))?
        .map_err(|err| HostError::message(format!("script function '{name}' failed: {:?}", err)))
    }

    fn foreign_handle(&self, data: Data) -> Result<i64, HostError> {
        match data {
            Data::ProtoBoxRef { box_idx, .. } | Data::BoxRef(box_idx) => {
                match self.host.ctx.box_heap.get(box_idx as usize) {
                    Some(Data::Foreign { handle, .. }) => Ok(*handle),
                    other => Err(HostError::message(format!(
                        "box {box_idx} is not foreign: {:?}",
                        other
                    ))),
                }
            }
            Data::Foreign { handle, .. } => Ok(handle),
            Data::Int(handle) => Ok(handle),
            other => Err(HostError::message(format!(
                "expected optional COM foreign box, got {:?}",
                other
            ))),
        }
    }
}

fn binding_provider() -> Box<dyn ModuleProvider> {
    let mut provider = InMemoryModuleProvider::new();
    provider.add_module(
        "scripting".to_string(),
        include_str!(concat!(env!("OUT_DIR"), "/scripting.p7")).to_string(),
    );
    provider.add_module(
        "editor_services".to_string(),
        include_str!(concat!(env!("OUT_DIR"), "/editor_services.p7")).to_string(),
    );
    provider.add_module(
        "radiance".to_string(),
        include_str!(concat!(env!("OUT_DIR"), "/radiance.p7")).to_string(),
    );
    provider.add_module(
        "ui".to_string(),
        crate::ui_walker::UI_BINDINGS_P7.to_string(),
    );
    provider.add_module(
        "editor".to_string(),
        include_str!(concat!(env!("OUT_DIR"), "/editor.p7")).to_string(),
    );
    Box::new(provider)
}
