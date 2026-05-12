//! Generic, AST-free `@foreign` proto dispatcher for crosscom IDL types.
//!
//! Wires the protosept runtime up to the crosscom Rust runtime by way of
//! two host functions:
//!
//! - `com.invoke`  — dispatches every method call on a `box<I>` value,
//!   regardless of which IDL it came from. The runtime pushes the
//!   metadata it discovered at compile time (`type_tag`, `method_name`,
//!   `vtable_slot`, `return_ty`) on top of the args; the dispatcher
//!   uses those plus the runtime shape of each popped `Data` value to
//!   marshal a C-ABI virtual call against the COM vtable.
//! - `com.release` — the finalizer, called when an owned `box<I>` is
//!   collected. Releases the strong reference held by `ComObjectTable`.
//!
//! No per-IDL Rust code is generated. Adding a new crosscom IDL only
//! requires running `crosscom-ccidl --protosept` to produce its `.p7`
//! source — the dispatcher in this file already understands every
//! `@foreign` proto via runtime metadata.
//!
//! ## C-ABI invocation
//!
//! crosscom Rust vtables are `#[repr(C)]` with `unsafe extern "system" fn`
//! pointers and use **direct returns** for primitives plus
//! [`crosscom::RawPointer`] (`*const *const c_void`) for COM-pointer
//! returns. The dispatcher classifies each popped argument by the
//! runtime shape of its [`Data`] and each declared return value by the
//! [`HostReturnTy`] decoder, then assembles a libffi CIF from the
//! resulting type list and invokes the loaded vtable cell. The arg
//! side is fully data-driven (one `MarshalledArg` per popped Data,
//! mapped to the matching `libffi::middle::Type`). The return side
//! makes a small 4-way static dispatch on `RetKind` because
//! libffi-rs's `Cif::call::<R>` is generic over the Rust return type
//! at the call site.

use std::ffi::{c_char, c_float, c_long, c_void, CString};
use std::os::raw::c_int;

use crate::{with_services, ComObjectTable};
use libffi::middle::{arg, Arg, Cif, CodePtr, Type};
use p7::errors::RuntimeError;
use p7::interpreter::context::{Context, Data};
use p7::semantic::HostReturnTy;

/// Register the generic crosscom dispatcher and finalizer on `ctx`. Any
/// previously-registered handler under the same names is overwritten.
/// Idempotent: safe to call once per [`Context`] (multiple calls just
/// re-register).
///
/// Use after `Context::new()` and before loading any modules whose
/// `@foreign` protos use these dispatcher names.
pub fn install_com_dispatcher(ctx: &mut Context) {
    ctx.register_host_function("com.invoke".to_string(), com_invoke);
    ctx.register_host_function("com.release".to_string(), com_release);
}

// ---------------------------------------------------------------------------
// com.invoke
// ---------------------------------------------------------------------------

/// Generic dispatcher for every crosscom `@foreign` proto method call.
///
/// Stack layout entering this function (top → bottom, per p7's
/// `SymbolKind::HostMethod` dispatch convention):
///
/// ```text
///   type_tag:    Data::String
///   method_name: Data::String
///   vtable_slot: Data::Int
///   return_ty:   Data::Array (encoded HostReturnTy tree)
///   arg_N
///   ...
///   arg_1
///   receiver:    Data::ProtoBoxRef → Data::Foreign { type_tag, handle }
/// ```
fn com_invoke(ctx: &mut Context) -> Result<(), RuntimeError> {
    let type_tag = pop_string(ctx, "com.invoke: type_tag")?;
    let _method_name = pop_string(ctx, "com.invoke: method_name")?;
    let vtable_slot = pop_int(ctx, "com.invoke: vtable_slot")? as usize;
    let return_ty = pop_return_ty(ctx)?;

    // Pop any preceding arguments. We don't get a separate arg count
    // pushed by p7, so we walk down the stack until we hit something
    // that looks like a foreign-receiver — the receiver is always the
    // bottom of the call's frame portion. Concretely: pop Data values
    // and classify each. The first `ProtoBoxRef → Data::Foreign` whose
    // tag matches `type_tag` is the receiver.
    let mut popped_args: Vec<MarshalledArg> = Vec::new();
    let recv_handle = loop {
        let data = ctx
            .stack_frame_mut()?
            .stack
            .pop()
            .ok_or(RuntimeError::StackUnderflow)?;
        match classify_pop(ctx, data, &type_tag)? {
            ClassifiedPop::Receiver(handle) => break handle,
            ClassifiedPop::Arg(arg) => popped_args.push(arg),
        }
    };
    // We popped args in reverse declaration order; reverse so arg_1 is
    // first in the libffi-style call list.
    popped_args.reverse();

    // Resolve the receiver via ComObjectTable + QI by the proto's UUID.
    let recv_uuid_str = ctx
        .foreign_uuid(&type_tag)
        .ok_or_else(|| {
            RuntimeError::Other(format!(
                "com.invoke: no UUID registered for type_tag '{}' (did the @foreign \
                 proto omit `uuid=\"...\"`?)",
                type_tag
            ))
        })?
        .to_string();
    let recv_uuid_bytes = parse_uuid(&recv_uuid_str)?;

    let this_ptr: *const *const c_void =
        with_services(|s| s.com_table_mut().get_raw_qi(recv_handle, recv_uuid_bytes))
            .map_err(|e| RuntimeError::Other(format!("com.invoke: with_services: {}", e)))?
            .ok_or_else(|| {
                RuntimeError::Other(format!(
                    "com.invoke: receiver handle {} did not expose interface for type_tag '{}'",
                    recv_handle, type_tag
                ))
            })?;

    // Compute the vtable function pointer. The +3 skips IUnknown's
    // prefix slots; p7's `vtable_slot` is the method's index *within
    // its @foreign proto*.
    let fn_ptr: *const c_void = unsafe {
        let vtbl_ptr = *(this_ptr as *const *const c_void);
        let cell = (vtbl_ptr as *const *const c_void).add(3 + vtable_slot);
        *cell
    };

    // Marshal the call via the closed signature catalog.
    let raw = unsafe {
        invoke_via_catalog(fn_ptr, this_ptr, &popped_args, &return_ty).map_err(|e| {
            RuntimeError::Other(format!(
                "com.invoke: dispatch failed for type_tag='{}', slot={}: {}",
                type_tag, vtable_slot, e
            ))
        })?
    };

    // Drop the QI'd strong ref (the COM table still holds the original).
    unsafe {
        let unk_vtbl = *(this_ptr as *const *const crosscom::IUnknownVirtualTable);
        ((*unk_vtbl).release)(this_ptr);
    }

    push_return(ctx, &return_ty, raw)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// com.release
// ---------------------------------------------------------------------------

/// Finalizer for owned `box<I>` foreign cells. Called once per dropped
/// box by the GC. Stack layout entering this function (top → bottom):
///
/// ```text
///   type_tag: Data::String   (passed by p7's foreign-cell finalizer hook)
///   handle:   Data::Int      (ComObjectTable id)
/// ```
///
/// We accept either order (with or without a leading type_tag) for
/// resilience as the runtime protocol firms up.
fn com_release(ctx: &mut Context) -> Result<(), RuntimeError> {
    let top = ctx
        .stack_frame_mut()?
        .stack
        .pop()
        .ok_or(RuntimeError::StackUnderflow)?;
    let handle = match top {
        Data::Int(i) => i,
        Data::String(_) => match ctx.stack_frame_mut()?.stack.pop() {
            Some(Data::Int(i)) => i,
            other => {
                return Err(RuntimeError::Other(format!(
                    "com.release: expected handle int after type_tag, got {:?}",
                    other
                )));
            }
        },
        other => {
            return Err(RuntimeError::Other(format!(
                "com.release: expected handle int (or type_tag string + handle), got {:?}",
                other
            )));
        }
    };
    let _ = with_services(|s| {
        let table: &mut ComObjectTable = s.com_table_mut();
        table.release(handle);
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Argument classification
// ---------------------------------------------------------------------------

/// One marshalled argument value, classified by its C-ABI kind. The
/// dispatcher builds the libffi CIF from the [`Type`] of each variant
/// and the call's `Arg` slice from `as_ffi_arg`.
#[derive(Debug)]
enum MarshalledArg {
    Long(c_long),
    Float(c_float),
    Pointer(*const c_void),
    /// Owns the C string for the duration of the call; lifetime is the
    /// dispatcher's stack frame (this Rust fn's locals).
    Str(CString),
}

impl MarshalledArg {
    /// libffi `Type` matching this arg's C-ABI shape.
    fn ffi_type(&self) -> Type {
        match self {
            MarshalledArg::Long(_) => Type::c_long(),
            MarshalledArg::Float(_) => Type::f32(),
            MarshalledArg::Pointer(_) | MarshalledArg::Str(_) => Type::pointer(),
        }
    }
}

enum ClassifiedPop {
    Receiver(i64),
    Arg(MarshalledArg),
}

/// Decide whether the popped `Data` is the receiver (a foreign cell of
/// `recv_tag`) or an ordinary argument, marshalling args to their C-ABI
/// form along the way. Foreign args of *other* tags are also accepted —
/// they are interface arguments to this method, marshalled to a raw
/// `*const *const c_void` via `ComObjectTable + QI`.
fn classify_pop(
    ctx: &mut Context,
    data: Data,
    recv_tag: &str,
) -> Result<ClassifiedPop, RuntimeError> {
    match data {
        Data::Int(i) => Ok(ClassifiedPop::Arg(MarshalledArg::Long(i as c_long))),
        Data::Float(f) => Ok(ClassifiedPop::Arg(MarshalledArg::Float(f as c_float))),
        Data::String(s) => {
            let c = CString::new(s).map_err(|_| {
                RuntimeError::Other("com.invoke: string arg contained interior NUL".into())
            })?;
            Ok(ClassifiedPop::Arg(MarshalledArg::Str(c)))
        }
        Data::Null => Ok(ClassifiedPop::Arg(MarshalledArg::Pointer(std::ptr::null()))),
        Data::Some(inner) => classify_pop(ctx, *inner, recv_tag),
        Data::ProtoBoxRef { box_idx, .. }
        | Data::ProtoRefRef {
            ref_idx: box_idx, ..
        } => classify_foreign_box(ctx, box_idx, recv_tag),
        Data::BoxRef(idx) => classify_foreign_box(ctx, idx, recv_tag),
        other => Err(RuntimeError::Other(format!(
            "com.invoke: unsupported argument shape: {:?}",
            other
        ))),
    }
}

fn classify_foreign_box(
    ctx: &mut Context,
    box_idx: u32,
    recv_tag: &str,
) -> Result<ClassifiedPop, RuntimeError> {
    let payload = ctx
        .box_heap
        .get(box_idx as usize)
        .ok_or_else(|| RuntimeError::Other(format!("com.invoke: invalid box index {}", box_idx)))?
        .clone();
    match payload {
        Data::Foreign {
            type_tag, handle, ..
        } => {
            if type_tag == recv_tag {
                return Ok(ClassifiedPop::Receiver(handle));
            }
            // Interface argument: QI to the arg's own UUID and pass the
            // raw pointer.
            let uuid_str = ctx
                .foreign_uuid(&type_tag)
                .ok_or_else(|| {
                    RuntimeError::Other(format!(
                        "com.invoke: no UUID for foreign arg type_tag '{}'",
                        type_tag
                    ))
                })?
                .to_string();
            let uuid_bytes = parse_uuid(&uuid_str)?;
            let p: *const *const c_void =
                with_services(|s| s.com_table_mut().get_raw_qi(handle, uuid_bytes))
                    .map_err(|e| RuntimeError::Other(format!("com.invoke: with_services: {}", e)))?
                    .ok_or_else(|| {
                        RuntimeError::Other(format!(
                            "com.invoke: foreign-arg handle {} does not expose '{}'",
                            handle, type_tag
                        ))
                    })?;
            Ok(ClassifiedPop::Arg(MarshalledArg::Pointer(
                p as *const c_void,
            )))
        }
        other => Err(RuntimeError::Other(format!(
            "com.invoke: box did not contain a Foreign value: {:?}",
            other
        ))),
    }
}

// ---------------------------------------------------------------------------
// libffi-driven invocation
// ---------------------------------------------------------------------------

/// Return-side classification driving libffi dispatch. We collapse
/// every Optional/Array of foreign things to `Pointer` because the
/// underlying C ABI is the same; the structural meaning is reapplied at
/// `push_return` time.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum RetKind {
    Void,
    Long,
    Float,
    Pointer,
}

impl RetKind {
    /// libffi `Type` for the C-ABI return slot.
    fn ffi_type(self) -> Type {
        match self {
            RetKind::Void => Type::void(),
            RetKind::Long => Type::c_long(),
            RetKind::Float => Type::f32(),
            RetKind::Pointer => Type::pointer(),
        }
    }
}

fn ret_kind_of(rt: &HostReturnTy) -> RetKind {
    match rt {
        HostReturnTy::Void => RetKind::Void,
        HostReturnTy::Int => RetKind::Long,
        HostReturnTy::Float => RetKind::Float,
        HostReturnTy::String => RetKind::Pointer,
        HostReturnTy::Foreign { .. } => RetKind::Pointer,
        HostReturnTy::Optional(inner) => match ret_kind_of(inner) {
            // Optional primitives aren't expressible in the COM ABI
            // (crosscom IDL only supports optional interfaces today).
            // Treat them as Pointer for safety; `push_return` decides.
            RetKind::Void => RetKind::Pointer,
            other => other,
        },
        HostReturnTy::Array(_) => RetKind::Pointer,
    }
}

/// Raw return value carried back from a virtual call before being
/// re-shaped by `push_return`.
enum RawReturn {
    Void,
    Long(c_long),
    Float(c_float),
    Pointer(*const c_void),
}

/// Invoke a vtable cell via libffi. The CIF is built fresh per call from
/// the runtime [`MarshalledArg`] list — no per-shape Rust code exists for
/// the argument side. The return side performs a 4-way static dispatch
/// on [`RetKind`] because libffi-rs's `Cif::call::<R>` is parametric in
/// the Rust return type.
///
/// On x86 Windows `extern "system"` maps to the stdcall ABI; on every
/// other supported target it matches libffi's default. We currently
/// rely on `FFI_DEFAULT_ABI` matching; if 32-bit Windows support is
/// resurrected, set the ABI explicitly via `Cif::set_abi(FFI_STDCALL)`
/// here under `cfg(all(target_os="windows", target_arch="x86"))`.
unsafe fn invoke_via_catalog(
    fn_ptr: *const c_void,
    this: *const *const c_void,
    args: &[MarshalledArg],
    rt: &HostReturnTy,
) -> Result<RawReturn, String> {
    let ret_kind = ret_kind_of(rt);

    // Build the CIF type list: receiver + each marshalled arg.
    let mut arg_types: Vec<Type> = Vec::with_capacity(args.len() + 1);
    arg_types.push(Type::pointer());
    for a in args {
        arg_types.push(a.ffi_type());
    }
    let cif = Cif::new(arg_types.into_iter(), ret_kind.ffi_type());

    // Build the Arg slice. Each Arg borrows the underlying value, so
    // every referent must live until `cif.call` returns. The pattern:
    // for `Str`, snapshot the `*const c_char` once into a stable local
    // (`StrPtr`) and pass libffi a reference to *that* local — passing
    // a temporary like `arg(&s.as_ptr())` would dangle the moment the
    // expression ends.
    let str_ptrs: Vec<*const c_char> = args
        .iter()
        .map(|a| match a {
            MarshalledArg::Str(s) => s.as_ptr(),
            _ => std::ptr::null(),
        })
        .collect();
    let mut ffi_args: Vec<Arg> = Vec::with_capacity(args.len() + 1);
    ffi_args.push(arg(&this));
    for (i, a) in args.iter().enumerate() {
        let argp: Arg = match a {
            MarshalledArg::Long(v) => arg(v),
            MarshalledArg::Float(v) => arg(v),
            MarshalledArg::Pointer(v) => arg(v),
            MarshalledArg::Str(_) => arg(&str_ptrs[i]),
        };
        ffi_args.push(argp);
    }

    let code = CodePtr::from_ptr(fn_ptr);

    // libffi-rs's `call` is generic over the Rust return type, so we
    // need a small static dispatch on the four COM-relevant return
    // shapes. Anything richer (struct returns, etc.) would extend
    // `RetKind` first, then add an arm here.
    Ok(unsafe {
        match ret_kind {
            RetKind::Void => {
                let _: () = cif.call(code, &ffi_args);
                RawReturn::Void
            }
            RetKind::Long => RawReturn::Long(cif.call::<c_long>(code, &ffi_args)),
            RetKind::Float => RawReturn::Float(cif.call::<c_float>(code, &ffi_args)),
            RetKind::Pointer => RawReturn::Pointer(cif.call::<*const c_void>(code, &ffi_args)),
        }
    })
}

// ---------------------------------------------------------------------------
// Stack-decoding helpers
// ---------------------------------------------------------------------------

fn pop_string(ctx: &mut Context, what: &str) -> Result<String, RuntimeError> {
    match ctx.stack_frame_mut()?.stack.pop() {
        Some(Data::String(s)) => Ok(s),
        other => Err(RuntimeError::Other(format!(
            "{}: expected string, got {:?}",
            what, other
        ))),
    }
}

fn pop_int(ctx: &mut Context, what: &str) -> Result<i64, RuntimeError> {
    match ctx.stack_frame_mut()?.stack.pop() {
        Some(Data::Int(i)) => Ok(i),
        other => Err(RuntimeError::Other(format!(
            "{}: expected int, got {:?}",
            what, other
        ))),
    }
}

/// Decode the `Data::Array`-encoded `HostReturnTy` pushed by the p7
/// runtime ahead of every `@foreign` proto dispatch. See
/// `p7::interpreter::context::execution::encode_return_ty` for the
/// inverse encoder; the format is:
///
///   [0]               → Void
///   [1]               → Int
///   [2]               → Float
///   [3]               → String
///   [4, type_tag]     → Foreign { type_tag }
///   [5, inner]        → Optional(inner)
///   [6, inner]        → Array(inner)
fn pop_return_ty(ctx: &mut Context) -> Result<HostReturnTy, RuntimeError> {
    let top = ctx
        .stack_frame_mut()?
        .stack
        .pop()
        .ok_or(RuntimeError::StackUnderflow)?;
    decode_return_ty(top)
}

fn decode_return_ty(data: Data) -> Result<HostReturnTy, RuntimeError> {
    let items = match data {
        Data::Array(items) => items,
        other => {
            return Err(RuntimeError::Other(format!(
                "pop_return_ty: expected Array, got {:?}",
                other
            )));
        }
    };
    let mut iter = items.into_iter();
    let tag = match iter.next() {
        Some(Data::Int(i)) => i,
        other => {
            return Err(RuntimeError::Other(format!(
                "pop_return_ty: expected variant tag int, got {:?}",
                other
            )));
        }
    };
    Ok(match tag {
        0 => HostReturnTy::Void,
        1 => HostReturnTy::Int,
        2 => HostReturnTy::Float,
        3 => HostReturnTy::String,
        4 => match iter.next() {
            Some(Data::String(s)) => HostReturnTy::Foreign {
                type_tag: p7::intern::InternedString::from(s),
            },
            other => {
                return Err(RuntimeError::Other(format!(
                    "pop_return_ty: Foreign expects [type_tag], got {:?}",
                    other
                )));
            }
        },
        5 => HostReturnTy::Optional(Box::new(decode_return_ty(iter.next().ok_or_else(
            || RuntimeError::Other("pop_return_ty: Optional missing inner".into()),
        )?)?)),
        6 => HostReturnTy::Array(Box::new(decode_return_ty(iter.next().ok_or_else(
            || RuntimeError::Other("pop_return_ty: Array missing inner".into()),
        )?)?)),
        other => {
            return Err(RuntimeError::Other(format!(
                "pop_return_ty: unknown variant tag {}",
                other
            )));
        }
    })
}

// ---------------------------------------------------------------------------
// Return marshaling
// ---------------------------------------------------------------------------

fn push_return(ctx: &mut Context, rt: &HostReturnTy, raw: RawReturn) -> Result<(), RuntimeError> {
    match (rt, raw) {
        (HostReturnTy::Void, RawReturn::Void) => {
            // crosscom IDL `void` is mapped to protosept `int` (= 0 by
            // convention) so existing scripts that read the return get a
            // sensible value.
            ctx.stack_frame_mut()?.stack.push(Data::Int(0));
            Ok(())
        }
        (HostReturnTy::Int, RawReturn::Long(v)) => {
            ctx.stack_frame_mut()?.stack.push(Data::Int(v as i64));
            Ok(())
        }
        (HostReturnTy::Float, RawReturn::Float(v)) => {
            ctx.stack_frame_mut()?.stack.push(Data::Float(v as f64));
            Ok(())
        }
        (HostReturnTy::String, RawReturn::Pointer(p)) => {
            if p.is_null() {
                ctx.stack_frame_mut()?
                    .stack
                    .push(Data::String(String::new()));
            } else {
                let s = unsafe { std::ffi::CStr::from_ptr(p as *const c_char) }
                    .to_string_lossy()
                    .into_owned();
                ctx.stack_frame_mut()?.stack.push(Data::String(s));
            }
            Ok(())
        }
        (HostReturnTy::Foreign { type_tag }, RawReturn::Pointer(p)) => {
            push_returned_foreign(ctx, type_tag.as_str(), p, /*nullable=*/ false)
        }
        (HostReturnTy::Optional(inner), raw) => match (inner.as_ref(), raw) {
            (HostReturnTy::Foreign { type_tag }, RawReturn::Pointer(p)) => {
                push_returned_foreign(ctx, type_tag.as_str(), p, true)
            }
            (HostReturnTy::Int, RawReturn::Long(v)) => {
                ctx.stack_frame_mut()?
                    .stack
                    .push(Data::Some(Box::new(Data::Int(v as i64))));
                Ok(())
            }
            (HostReturnTy::Float, RawReturn::Float(v)) => {
                ctx.stack_frame_mut()?
                    .stack
                    .push(Data::Some(Box::new(Data::Float(v as f64))));
                Ok(())
            }
            (other_inner, _) => Err(RuntimeError::Other(format!(
                "com.invoke: unsupported Optional return shape: {:?}",
                other_inner
            ))),
        },
        (HostReturnTy::Array(_), _) => Err(RuntimeError::Other(
            "com.invoke: array returns not yet implemented in dispatcher".into(),
        )),
        (rt, raw) => Err(RuntimeError::Other(format!(
            "com.invoke: return marshaling mismatch (declared {:?}, raw discriminant {})",
            rt,
            match raw {
                RawReturn::Void => "Void",
                RawReturn::Long(_) => "Long",
                RawReturn::Float(_) => "Float",
                RawReturn::Pointer(_) => "Pointer",
            }
        ))),
    }
}

/// Intern a returned raw COM pointer into ComObjectTable and push the
/// resulting handle wrapped in a `Data::Foreign`. `nullable` controls
/// whether a null pointer maps to `Data::Null` (Optional) or an error
/// (non-nullable).
fn push_returned_foreign(
    ctx: &mut Context,
    type_tag: &str,
    p: *const c_void,
    nullable: bool,
) -> Result<(), RuntimeError> {
    if p.is_null() {
        if nullable {
            ctx.stack_frame_mut()?.stack.push(Data::Null);
            return Ok(());
        }
        return Err(RuntimeError::Other(format!(
            "com.invoke: non-nullable foreign return for type_tag '{}' was null",
            type_tag
        )));
    }
    // The raw pointer is already a strong ref (COM vtable returns add
    // a ref). We hand it to the table as `ComRc<IUnknown>` for safe
    // lifetime tracking; the table balances the count on `release`.
    let unk = unsafe {
        crosscom::ComRc::<crosscom::IUnknown>::from_raw_pointer(p as *const *const c_void)
    };
    let handle: i64 = with_services(|s| s.com_table_mut().intern_unknown(unk))
        .map_err(|e| RuntimeError::Other(format!("com.invoke: with_services: {}", e)))?;
    if nullable {
        ctx.push_foreign_optional(type_tag, Some(handle))
    } else {
        ctx.push_foreign(type_tag, handle)
    }
}

// ---------------------------------------------------------------------------
// Misc helpers
// ---------------------------------------------------------------------------

fn parse_uuid(s: &str) -> Result<[u8; 16], RuntimeError> {
    uuid::Uuid::parse_str(s)
        .map(|u| *u.as_bytes())
        .map_err(|e| RuntimeError::Other(format!("invalid UUID '{}': {}", s, e)))
}

#[allow(dead_code)]
fn _unused_c_int_marker(_: c_int) {}

#[cfg(test)]
mod tests {
    use super::*;
    use p7::interpreter::context::Context;
    use p7::semantic::HostReturnTy;

    #[test]
    fn install_registers_com_invoke_and_com_release() {
        let mut ctx = Context::new();
        install_com_dispatcher(&mut ctx);
        let _ = ctx;
    }

    #[test]
    fn return_ty_decoder_round_trips() {
        let cases = vec![
            HostReturnTy::Void,
            HostReturnTy::Int,
            HostReturnTy::Float,
            HostReturnTy::String,
            HostReturnTy::Foreign {
                type_tag: p7::intern::InternedString::from("a.b.IFoo"),
            },
            HostReturnTy::Optional(Box::new(HostReturnTy::Int)),
            HostReturnTy::Array(Box::new(HostReturnTy::Foreign {
                type_tag: p7::intern::InternedString::from("a.b.IBar"),
            })),
        ];
        for original in cases {
            let encoded = p7::interpreter::context::encode_return_ty(&original);
            let decoded = decode_return_ty(encoded).expect("decode");
            assert_eq!(decoded, original);
        }
    }

    #[test]
    fn ret_kind_classification() {
        assert_eq!(ret_kind_of(&HostReturnTy::Void), RetKind::Void);
        assert_eq!(ret_kind_of(&HostReturnTy::Int), RetKind::Long);
        assert_eq!(ret_kind_of(&HostReturnTy::Float), RetKind::Float);
        assert_eq!(
            ret_kind_of(&HostReturnTy::Foreign {
                type_tag: p7::intern::InternedString::from("x"),
            }),
            RetKind::Pointer,
        );
        assert_eq!(
            ret_kind_of(&HostReturnTy::Optional(Box::new(HostReturnTy::Foreign {
                type_tag: p7::intern::InternedString::from("x"),
            }))),
            RetKind::Pointer,
        );
    }
}
