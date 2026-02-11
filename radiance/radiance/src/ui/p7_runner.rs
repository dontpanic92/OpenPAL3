use log::warn;

use p7::bytecode::Module;
use p7::errors::Proto7Error;
use p7::errors::RuntimeError;
use p7::interpreter::context::{Context, Data, Struct as P7Struct};
use p7::ModuleProvider;

use super::{register_p7_ui_host_functions, Message, RadianceUiModuleProvider};

pub struct UiScriptRunner {
    context: Context,
    module: Module,
    initialized: bool,
}

impl UiScriptRunner {
    pub fn new(
        source: String,
        provider: Box<dyn ModuleProvider>,
    ) -> Result<Self, Proto7Error> {
        let provider = RadianceUiModuleProvider::new(provider);
        let module = p7::compile_with_provider(source, Box::new(provider))?;

        let mut context = Context::new();
        register_p7_ui_host_functions(&mut context);
        context.load_module(module.clone());

        Ok(Self {
            context,
            module,
            initialized: false,
        })
    }

    pub fn init_if_available(&mut self) -> Result<(), Proto7Error> {
        if self.initialized {
            return Ok(());
        }

        if self.has_function("init") {
            self.call_function("init", Vec::new())?;
        }

        self.initialized = true;
        Ok(())
    }

    pub fn process_messages(&mut self, messages: Vec<Message>) -> Result<(), Proto7Error> {
        if !self.has_function("process_message") {
            return Err(Proto7Error::RuntimeError(RuntimeError::Other(
                "process_message not found".to_string(),
            )));
        }

        for message in messages {
            let data = self.message_to_data(message);
            self.call_function("process_message", vec![data])?;
        }

        Ok(())
    }

    fn call_function(&mut self, name: &str, params: Vec<Data>) -> Result<(), Proto7Error> {
        if !self.has_function(name) {
            return Err(Proto7Error::RuntimeError(RuntimeError::Other(format!(
                "function '{}' not found",
                name
            ))));
        }

        self.context.push_function(name, params);
        self.context
            .resume()
            .map_err(Proto7Error::RuntimeError)?;

        if let Some(frame) = self.context.stack.get_mut(0) {
            let _ = frame.stack.pop();
        }

        Ok(())
    }

    fn has_function(&self, name: &str) -> bool {
        self.module.get_function(name).is_some()
    }

    fn message_to_data(&mut self, message: Message) -> Data {
        let fields = vec![
            Data::String(message.kind),
            nullable_string(message.target_id),
            Data::String(message.payload),
        ];

        let struct_idx = self.context.heap.len() as u32;
        self.context.heap.push(P7Struct { fields });
        Data::StructRef(struct_idx)
    }
}

fn nullable_string(value: Option<String>) -> Data {
    match value {
        Some(text) => Data::Some(Box::new(Data::String(text))),
        None => Data::Null,
    }
}

pub fn warn_if_error(label: &str, result: Result<(), Proto7Error>) {
    if let Err(err) = result {
        warn!("p7 {} failed: {}", label, err);
    }
}
