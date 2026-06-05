#[cfg(any(windows, linux, macos))]
pub mod debug;

mod disassembler;
mod global_context;
mod module;
pub mod trace;
mod vm;

pub use disassembler::{AsInst, AsInstInstance, disasm};
pub use global_context::{
    ContinuationState, GlobalFunctionContinuation, GlobalFunctionState, ScriptGlobalContext,
    ScriptGlobalFunction, not_implemented,
};
pub use module::ScriptModule;
pub use trace::{BranchKind, GlobalScope, TraceEvent, TraceEventKind, TraceSink};
pub use vm::ScriptVm;
