//! Adapter crate: implements [`crosscom_p7host::HostContext`] for the
//! protosept interpreter [`p7::interpreter::context::Context`].
//!
//! This is the only crate in OpenPAL3 that depends on the protosept
//! workspace directly. By keeping the dep narrow (this single adapter),
//! every other consumer can pull in scripting via the
//! `crosscom-p7host` trait without taking on the protosept toolchain
//! constraints (resolver = "3", edition 2024) themselves.
//!
//! The crate also hosts the generic, AST-free crosscom dispatcher in
//! the [`dispatcher`] submodule. Use [`dispatcher::install_com_dispatcher`]
//! to wire `com.invoke` and `com.release` onto a freshly-created
//! [`p7::interpreter::context::Context`] before loading any modules.

pub mod dispatcher;

pub use dispatcher::install_com_dispatcher;

use crosscom_p7host::{ComObjectTable, HostContext, HostError, HostServices};
use p7::errors::RuntimeError;
use p7::interpreter::context::{Context, Data};

/// Wraps a [`Context`] together with a per-runtime services bundle so the
/// [`HostContext`] trait can be implemented uniformly on the same object
/// the interpreter operates on.
pub struct P7HostContext<S: HostServices> {
    pub ctx: Context,
    pub services: S,
}

impl<S: HostServices> P7HostContext<S> {
    pub fn new(services: S) -> Self {
        Self {
            ctx: Context::new(),
            services,
        }
    }
}

/// Minimal default services bundle — just an empty [`ComObjectTable`].
pub struct MinimalServices {
    pub com: ComObjectTable,
}

impl Default for MinimalServices {
    fn default() -> Self {
        Self {
            com: ComObjectTable::new(),
        }
    }
}

impl HostServices for MinimalServices {
    fn com_table_mut(&mut self) -> &mut ComObjectTable {
        &mut self.com
    }
}

impl<S: HostServices> HostContext for P7HostContext<S> {
    type Services = S;

    fn pop_int(&mut self) -> Result<i64, HostError> {
        let v = self
            .ctx
            .stack_frame_mut()
            .map_err(rt_err)?
            .stack
            .pop()
            .ok_or_else(|| HostError::message("pop_int: stack underflow"))?;
        match v {
            Data::Int(i) => Ok(i),
            other => Err(HostError::message(format!(
                "pop_int: expected int, got {:?}",
                other
            ))),
        }
    }

    fn pop_float(&mut self) -> Result<f64, HostError> {
        let v = self
            .ctx
            .stack_frame_mut()
            .map_err(rt_err)?
            .stack
            .pop()
            .ok_or_else(|| HostError::message("pop_float: stack underflow"))?;
        match v {
            Data::Float(f) => Ok(f),
            Data::Int(i) => Ok(i as f64),
            other => Err(HostError::message(format!(
                "pop_float: expected float, got {:?}",
                other
            ))),
        }
    }

    fn pop_string(&mut self) -> Result<String, HostError> {
        let v = self
            .ctx
            .stack_frame_mut()
            .map_err(rt_err)?
            .stack
            .pop()
            .ok_or_else(|| HostError::message("pop_string: stack underflow"))?;
        match v {
            Data::String(s) => Ok(s),
            other => Err(HostError::message(format!(
                "pop_string: expected string, got {:?}",
                other
            ))),
        }
    }

    fn pop_optional_int(&mut self) -> Result<Option<i64>, HostError> {
        let v = self
            .ctx
            .stack_frame_mut()
            .map_err(rt_err)?
            .stack
            .pop()
            .ok_or_else(|| HostError::message("pop_optional_int: stack underflow"))?;
        match v {
            Data::Null => Ok(None),
            Data::Some(inner) => match *inner {
                Data::Int(i) => Ok(Some(i)),
                other => Err(HostError::message(format!(
                    "pop_optional_int: Some(non-int): {:?}",
                    other
                ))),
            },
            Data::Int(i) => Ok(Some(i)),
            other => Err(HostError::message(format!(
                "pop_optional_int: expected ?int, got {:?}",
                other
            ))),
        }
    }

    fn pop_int_array(&mut self) -> Result<Vec<i64>, HostError> {
        let v = self
            .ctx
            .stack_frame_mut()
            .map_err(rt_err)?
            .stack
            .pop()
            .ok_or_else(|| HostError::message("pop_int_array: stack underflow"))?;
        match v {
            Data::Array(items) => items
                .into_iter()
                .map(|d| match d {
                    Data::Int(i) => Ok(i),
                    other => Err(HostError::message(format!(
                        "pop_int_array: non-int element {:?}",
                        other
                    ))),
                })
                .collect(),
            other => Err(HostError::message(format!(
                "pop_int_array: expected array, got {:?}",
                other
            ))),
        }
    }

    fn push_int(&mut self, value: i64) {
        if let Ok(frame) = self.ctx.stack_frame_mut() {
            frame.stack.push(Data::Int(value));
        }
    }

    fn push_float(&mut self, value: f64) {
        if let Ok(frame) = self.ctx.stack_frame_mut() {
            frame.stack.push(Data::Float(value));
        }
    }

    fn push_string(&mut self, value: String) {
        if let Ok(frame) = self.ctx.stack_frame_mut() {
            frame.stack.push(Data::String(value));
        }
    }

    fn push_optional_int(&mut self, value: Option<i64>) {
        if let Ok(frame) = self.ctx.stack_frame_mut() {
            let d = match value {
                Some(i) => Data::Some(Box::new(Data::Int(i))),
                None => Data::Null,
            };
            frame.stack.push(d);
        }
    }

    fn push_int_array(&mut self, value: Vec<i64>) {
        if let Ok(frame) = self.ctx.stack_frame_mut() {
            let arr = value.into_iter().map(Data::Int).collect();
            frame.stack.push(Data::Array(arr));
        }
    }

    fn register_host_function(
        &mut self,
        _name: &str,
        _func: fn(&mut Self) -> Result<(), HostError>,
    ) -> Result<(), HostError> {
        // The p7 interpreter expects host fns shaped as
        // `fn(&mut p7::Context) -> p7::ContextResult<()>`. Bridging to
        // `fn(&mut Self) -> Result<(), HostError>` requires recovering
        // `Self` (which carries the user-supplied `Services`) from
        // `&mut Context`, which is fundamentally type-erased.
        //
        // The integration strategy is documented in plan.md (§"register
        // shim"). For this adapter MVP the registration is a no-op; the
        // consumer's `ScriptRuntime` performs the real wiring.
        Ok(())
    }

    fn services_mut(&mut self) -> &mut Self::Services {
        &mut self.services
    }
}

fn rt_err(e: RuntimeError) -> HostError {
    HostError::message(format!("p7 runtime error: {:?}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_then_pop_int() {
        let mut h = P7HostContext::new(MinimalServices::default());
        h.push_int(42);
        assert_eq!(h.pop_int().unwrap(), 42);
    }

    #[test]
    fn push_then_pop_string() {
        let mut h = P7HostContext::new(MinimalServices::default());
        h.push_string("hello".to_string());
        assert_eq!(h.pop_string().unwrap(), "hello");
    }

    #[test]
    fn optional_round_trip() {
        let mut h = P7HostContext::new(MinimalServices::default());
        h.push_optional_int(Some(7));
        assert_eq!(h.pop_optional_int().unwrap(), Some(7));
        h.push_optional_int(None);
        assert_eq!(h.pop_optional_int().unwrap(), None);
    }

    #[test]
    fn array_round_trip() {
        let mut h = P7HostContext::new(MinimalServices::default());
        h.push_int_array(vec![1, 2, 3]);
        assert_eq!(h.pop_int_array().unwrap(), vec![1, 2, 3]);
    }
}
