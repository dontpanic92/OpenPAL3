#[cfg(any(windows, linux, macos))]
pub mod debug;

mod disassembler;
mod global_context;
mod module;
pub mod trace;
mod vm;

pub use disassembler::{disasm, AsInst, AsInstInstance};
pub use global_context::{
    not_implemented, ContinuationState, GlobalFunctionContinuation, GlobalFunctionState,
    ScriptGlobalContext, ScriptGlobalFunction,
};
pub use module::ScriptModule;
pub use trace::{BranchKind, GlobalScope, TraceEvent, TraceEventKind, TraceSink};
pub use vm::ScriptVm;
