//! Typed Protosept host functions for the p7-lcl front end.

use std::path::PathBuf;

use p7::embedding::Runtime;
use p7::errors::RuntimeError;
use p7::interpreter::context::Data;
use p7::interpreter::native::{NativeSignature, NativeType};

use crate::service::{JobState, ManagerService};

pub fn register(runtime: &mut Runtime, service: ManagerService) {
    register_string(runtime, "patcher.root_path", {
        let service = service.clone();
        move || Ok(service.root_path())
    });

    runtime.register_native_function(
        "patcher.choose_root",
        NativeSignature::new(Vec::new(), Some(NativeType::String)),
        {
            let service = service.clone();
            move |_context, _args| {
                let selected = match native_dialog::FileDialogBuilder::default()
                    .open_single_dir()
                    .show()
                {
                    Ok(selected) => selected,
                    Err(error) => return Ok(Some(Data::string(error.to_string()))),
                };
                let Some(path) = selected else {
                    return Ok(Some(Data::string("")));
                };
                let result = service
                    .set_root(path)
                    .and_then(|()| service.start_refresh());
                Ok(Some(Data::string(action_result(result))))
            }
        },
    );

    register_action(runtime, "patcher.start_refresh", {
        let service = service.clone();
        move || service.start_refresh()
    });
    register_action_string(runtime, "patcher.start_import", {
        let service = service.clone();
        move |path| service.start_import(PathBuf::from(path))
    });
    register_action_index(runtime, "patcher.start_install", {
        let service = service.clone();
        move |index| service.start_install(index)
    });
    register_action_index(runtime, "patcher.start_uninstall", {
        let service = service.clone();
        move |index| service.start_uninstall(index)
    });
    register_unit(runtime, "patcher.acknowledge_job", {
        let service = service.clone();
        move || {
            service.acknowledge_job();
            Ok(())
        }
    });

    register_int(runtime, "patcher.mod_count", {
        let service = service.clone();
        move || Ok(service.mod_count() as i64)
    });
    register_index_string(runtime, "patcher.mod_label", {
        let service = service.clone();
        move |index| Ok(mod_entry(&service, index)?.label)
    });
    register_index_string(runtime, "patcher.mod_details", {
        let service = service.clone();
        move |index| Ok(mod_entry(&service, index)?.details)
    });
    register_index_string(runtime, "patcher.mod_validation", {
        let service = service.clone();
        move |index| Ok(mod_entry(&service, index)?.validation)
    });
    register_index_bool(runtime, "patcher.mod_applied", {
        let service = service.clone();
        move |index| Ok(mod_entry(&service, index)?.applied)
    });
    register_index_bool(runtime, "patcher.mod_can_install", {
        let service = service.clone();
        move |index| Ok(mod_entry(&service, index)?.can_install)
    });
    register_index_bool(runtime, "patcher.mod_can_uninstall", {
        let service = service.clone();
        move |index| Ok(mod_entry(&service, index)?.can_uninstall)
    });

    register_int(runtime, "patcher.job_state", {
        let service = service.clone();
        move || {
            Ok(match service.job().state {
                JobState::Idle => 0,
                JobState::Running => 1,
                JobState::Succeeded => 2,
                JobState::Failed => 3,
            })
        }
    });
    register_string(runtime, "patcher.job_kind", {
        let service = service.clone();
        move || Ok(service.job().kind)
    });
    register_string(runtime, "patcher.job_message", {
        let service = service.clone();
        move || Ok(service.job().message)
    });
    register_int(runtime, "patcher.job_completed", {
        let service = service.clone();
        move || Ok(service.job().completed as i64)
    });
    register_int(runtime, "patcher.job_total", move || {
        Ok(service.job().total as i64)
    });
}

fn mod_entry(
    service: &ManagerService,
    index: usize,
) -> Result<crate::service::ModEntry, RuntimeError> {
    service
        .mod_entry(index)
        .ok_or_else(|| RuntimeError::Other(format!("invalid mod index {index}")))
}

fn register_unit<F>(runtime: &mut Runtime, name: &str, callback: F)
where
    F: Fn() -> Result<(), RuntimeError> + 'static,
{
    runtime.register_native_function(
        name,
        NativeSignature::new(Vec::new(), None),
        move |_context, _args| {
            callback()?;
            Ok(None)
        },
    );
}

fn register_action<F>(runtime: &mut Runtime, name: &str, callback: F)
where
    F: Fn() -> crate::Result<()> + 'static,
{
    runtime.register_native_function(
        name,
        NativeSignature::new(Vec::new(), Some(NativeType::String)),
        move |_context, _args| Ok(Some(Data::string(action_result(callback())))),
    );
}

fn register_action_string<F>(runtime: &mut Runtime, name: &str, callback: F)
where
    F: Fn(&str) -> crate::Result<()> + 'static,
{
    runtime.register_native_function(
        name,
        NativeSignature::new(vec![NativeType::String], Some(NativeType::String)),
        move |_context, args| {
            Ok(Some(Data::string(action_result(callback(
                args[0].as_str().expect("signature checked string"),
            )))))
        },
    );
}

fn register_action_index<F>(runtime: &mut Runtime, name: &str, callback: F)
where
    F: Fn(usize) -> crate::Result<()> + 'static,
{
    runtime.register_native_function(
        name,
        NativeSignature::new(vec![NativeType::Int], Some(NativeType::String)),
        move |_context, args| {
            Ok(Some(Data::string(action_result(callback(index_arg(
                &args[0],
            )?)))))
        },
    );
}

fn register_int<F>(runtime: &mut Runtime, name: &str, callback: F)
where
    F: Fn() -> Result<i64, RuntimeError> + 'static,
{
    runtime.register_native_function(
        name,
        NativeSignature::new(Vec::new(), Some(NativeType::Int)),
        move |_context, _args| Ok(Some(Data::Int(callback()?))),
    );
}

fn register_string<F>(runtime: &mut Runtime, name: &str, callback: F)
where
    F: Fn() -> Result<String, RuntimeError> + 'static,
{
    runtime.register_native_function(
        name,
        NativeSignature::new(Vec::new(), Some(NativeType::String)),
        move |_context, _args| Ok(Some(Data::string(callback()?))),
    );
}

fn register_index_string<F>(runtime: &mut Runtime, name: &str, callback: F)
where
    F: Fn(usize) -> Result<String, RuntimeError> + 'static,
{
    runtime.register_native_function(
        name,
        NativeSignature::new(vec![NativeType::Int], Some(NativeType::String)),
        move |_context, args| Ok(Some(Data::string(callback(index_arg(&args[0])?)?))),
    );
}

fn register_index_bool<F>(runtime: &mut Runtime, name: &str, callback: F)
where
    F: Fn(usize) -> Result<bool, RuntimeError> + 'static,
{
    runtime.register_native_function(
        name,
        NativeSignature::new(vec![NativeType::Int], Some(NativeType::Bool)),
        move |_context, args| Ok(Some(Data::Int(callback(index_arg(&args[0])?)? as i64))),
    );
}

fn index_arg(data: &Data) -> Result<usize, RuntimeError> {
    let Data::Int(value) = data else {
        unreachable!("signature checked integer")
    };
    usize::try_from(*value)
        .map_err(|_| RuntimeError::Other(format!("index must be non-negative, got {value}")))
}

fn action_result(result: crate::Result<()>) -> String {
    result
        .map(|()| String::new())
        .unwrap_or_else(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use p7::embedding::CallOutcome;

    #[test]
    fn registered_getters_are_callable_from_p7() {
        let module = p7::compile(
            r#"
@intrinsic(name="patcher.mod_count")
fn mod_count() -> int;
@intrinsic(name="patcher.job_message")
fn job_message() -> string;

fn count() -> int { mod_count() }
fn message() -> string { job_message() }
"#
            .to_string(),
        )
        .unwrap();
        let mut runtime = Runtime::new();
        register(&mut runtime, ManagerService::default());
        runtime.load_module(module);

        assert!(matches!(
            runtime.call("count", Vec::new()).unwrap(),
            CallOutcome::Returned(Some(Data::Int(0)))
        ));
        assert!(matches!(
            runtime.call("message", Vec::new()).unwrap(),
            CallOutcome::Returned(Some(Data::String(_)))
        ));
    }
}
