//! `ScriptHost` is the single, app-lifetime owner of the p7 interpreter, the
//! `ComObjectTable`, and every script-side GC root the host hands out.
//!
//! Methods take `&self` and use interior mutability so the host can be shared
//! freely as `Rc<ScriptHost>` without callers having to thread a
//! `RefCell<...>` around. There is no separate "runtime handle" wrapper — the
//! `ScriptHost` itself is the only public type.
//!
//! ## Re-entrancy
//!
//! p7 scripts routinely call back into Rust via `@foreign` dispatchers, and
//! those Rust handlers (host services like `IAppService::open_game` or
//! `IConfigService::pick_folder`) commonly need to call back into
//! `ScriptHost` — to `intern` a fresh ComObject, build a `foreign_box`, root a
//! returned value, or trigger another script function. The call shape is
//! therefore inherently re-entrant on a single thread: `&mut Inner` is live
//! at every depth of the call stack.
//!
//! `RefCell` rejects this pattern — it tracks borrow counts dynamically and
//! panics on the second `borrow_mut()`. We use `UnsafeCell<Inner>` instead and
//! gate every access through [`ScriptHost::with_inner`], which produces an
//! `&mut Inner` whose lifetime is the closure body only. Within the closure
//! the borrow is exclusive; across closures (e.g. nested re-entrant calls)
//! exclusivity is upheld by the single-threaded, stack-disciplined nature of
//! p7's interpreter loop and the foreign dispatcher.

use std::cell::{Cell, OnceCell, UnsafeCell};
use std::panic::AssertUnwindSafe;
use std::rc::Rc;

use crosscom::{ComInterface, ComRc};
use crosscom_protosept::{
    install_com_dispatcher, scope, scope_context, with_context, ComObjectTable, HostError,
    HostServices, P7HostContext, RuntimeAccess, RuntimeHandle,
};
use p7::interpreter::context::{Context, Data};
use p7::{InMemoryModuleProvider, ModuleProvider};
use radiance::radiance::CoreRadianceEngine;

pub struct RuntimeServices {
    pub com: ComObjectTable,
    /// Weak handle to the owning [`ScriptHost`], installed by
    /// [`ScriptHost::new`] / [`ScriptHost::install`] once the
    /// `Rc<ScriptHost>` exists. Reverse-wrap CCWs constructed inside
    /// the `com.invoke` dispatcher capture this so their thunks and
    /// `Drop` can re-enter the runtime outside any active
    /// [`scope_context`].
    pub runtime_handle: RuntimeHandle,
}

impl Default for RuntimeServices {
    fn default() -> Self {
        Self {
            com: ComObjectTable::new(),
            runtime_handle: RuntimeHandle::dangling(),
        }
    }
}

impl HostServices for RuntimeServices {
    fn com_table_mut(&mut self) -> &mut ComObjectTable {
        &mut self.com
    }

    fn runtime_handle(&self) -> RuntimeHandle {
        self.runtime_handle.clone()
    }
}

pub const DIRECTOR_BINDINGS_P7: &str = include_str!("../bindings/director.p7");

struct Inner {
    host: P7HostContext<RuntimeServices>,
    epoch: u64,
    extra_bindings: Vec<(String, String)>,
}

impl Inner {
    fn fresh() -> Self {
        Self::with_bindings(Vec::new())
    }

    fn with_bindings(extra_bindings: Vec<(String, String)>) -> Self {
        let mut host = P7HostContext::new(RuntimeServices::default());
        install_com_dispatcher(&mut host.ctx);
        Self {
            host,
            epoch: 0,
            extra_bindings,
        }
    }
}

/// Opaque handle to a rooted script `Data` value (typically a `box<Director>`).
///
/// Carries an internal epoch so that operations against a handle outlive a
/// `ScriptHost::reload` silently return `None` / no-op instead of indexing into
/// the freshly-rebuilt interpreter state.
#[derive(Clone, Copy, Debug)]
pub struct ScriptDirectorHandle {
    index: usize,
    epoch: u64,
}

pub struct ScriptHost {
    inner: UnsafeCell<Inner>,
    next_epoch: Cell<u64>,
    /// Weak-handle to `self`, installed by [`ScriptHost::new`] /
    /// [`ScriptHost::install`] once the `Rc<Self>` exists. Stored
    /// here so [`reload`](Self::reload) — which rebuilds the inner
    /// services bundle from scratch — can re-stamp the handle into
    /// the freshly-constructed services without needing the original
    /// `Rc<Self>` again.
    runtime_handle: OnceCell<RuntimeHandle>,
}

impl ScriptHost {
    /// Single point of mutable access to the interpreter state.
    ///
    /// # Safety contract (re-entrant single-threaded discipline)
    ///
    /// p7 is single-threaded and `ScriptHost` is not `Sync`, so two `&mut
    /// Inner` references can never be live concurrently on different threads.
    /// Re-entrant calls form a strict stack: an outer `with_inner` body
    /// invokes p7's interpreter, p7 calls back into a Rust host service, the
    /// host service calls another `with_inner` body, that body returns, and
    /// only then does the outer body's `&mut Inner` resume use. Each `&mut
    /// Inner` is therefore confined to its own non-overlapping call frame,
    /// matching the exclusive-borrow invariant in practice even though the
    /// borrow checker cannot prove it statically across the foreign-dispatch
    /// boundary.
    ///
    /// Callers must not stash the `&mut Inner` (or any derived `&mut`
    /// reference) anywhere that outlives the closure, must not spawn threads
    /// that touch `ScriptHost`, and must keep all interpreter-driving work
    /// inside `with_inner` closures.
    fn with_inner<R>(&self, body: impl FnOnce(&mut Inner) -> R) -> R {
        // SAFETY: see the module-level "Re-entrancy" doc and this function's
        // safety contract.
        unsafe { body(&mut *self.inner.get()) }
    }
}

impl ScriptHost {
    pub fn new() -> Rc<Self> {
        let host = Rc::new(Self {
            inner: UnsafeCell::new(Inner::fresh()),
            next_epoch: Cell::new(0),
            runtime_handle: OnceCell::new(),
        });
        Self::install_runtime_handle(&host);
        host
    }

    /// Installs a single `ScriptHost` on the radiance engine, creating it on
    /// first call and returning the existing instance thereafter.
    pub fn install(engine: &CoreRadianceEngine) -> Rc<Self> {
        let host = engine.get_or_insert_service(|| Self {
            inner: UnsafeCell::new(Inner::fresh()),
            next_epoch: Cell::new(0),
            runtime_handle: OnceCell::new(),
        });
        Self::install_runtime_handle(&host);
        host
    }

    /// Wire a `Weak<ScriptHost>` into the inner [`RuntimeServices`] so
    /// reverse-wrap CCWs (constructed by the `com.invoke` dispatcher
    /// when a script passes a SAM-coerced callback across the host
    /// boundary) capture a handle that survives past any single
    /// script call.
    ///
    /// Idempotent — only the first call ever stores the handle; later
    /// calls re-stamp the same handle into the current services
    /// bundle (used by [`reload`](Self::reload) after it rebuilds
    /// `Inner`).
    fn install_runtime_handle(host: &Rc<Self>) {
        let handle = RuntimeHandle::from_rc(host);
        let _ = host.runtime_handle.set(handle.clone());
        host.with_inner(|inner| {
            inner.host.services.runtime_handle = handle;
        });
    }

    /// Registers an additional p7 binding module that will be visible to every
    /// subsequently-loaded script via `import <name>`. Must be called before
    /// `load_source` (registrations made afterward apply only to later
    /// `load_source` calls; previously compiled modules are not re-resolved).
    /// The binding survives `reload`.
    pub fn add_binding(&self, name: impl Into<String>, source: impl Into<String>) {
        let name = name.into();
        let source = source.into();
        self.with_inner(|inner| {
            if let Some(existing) = inner
                .extra_bindings
                .iter_mut()
                .find(|(n, _)| *n == name)
            {
                existing.1 = source;
            } else {
                inner.extra_bindings.push((name, source));
            }
        });
    }

    pub fn load_source(&self, source: &str) -> Result<(), HostError> {
        let extra = self.with_inner(|inner| inner.extra_bindings.clone());
        let module = p7::compile_with_provider(source.to_string(), binding_provider(&extra))
            .map_err(|err| HostError::message(format!("p7 compile failed: {:?}", err)))?;
        self.with_inner(|inner| inner.host.ctx.load_module(module));
        Ok(())
    }

    /// Discards every loaded module, every rooted handle, and every interned
    /// ComObject, then re-initialises a fresh interpreter. Any
    /// `ScriptDirectorHandle` outstanding from before the call is silently
    /// invalidated by an epoch bump. Extra binding modules registered via
    /// `add_binding` are preserved.
    ///
    /// Must NOT be called while a script is executing (i.e. from within a
    /// host service invoked by p7) — there is no static check enforcing this,
    /// only the re-entrancy contract on `with_inner`.
    pub fn reload(&self) {
        let new_epoch = self.next_epoch.get().wrapping_add(1);
        self.next_epoch.set(new_epoch);
        self.with_inner(|inner| {
            let extra = std::mem::take(&mut inner.extra_bindings);
            *inner = Inner::with_bindings(extra);
            inner.epoch = new_epoch;
            // The fresh `Inner` carries a dangling runtime_handle in
            // its services bundle. Re-stamp the original Weak so
            // dispatcher-built CCWs keep working after a reload.
            if let Some(handle) = self.runtime_handle.get() {
                inner.host.services.runtime_handle = handle.clone();
            }
        });
    }

    pub fn intern<I: ComInterface + 'static>(&self, rc: ComRc<I>) -> i64 {
        self.with_inner(|inner| inner.host.services.com_table_mut().intern(rc))
    }

    pub fn foreign_box(&self, type_tag: &str, handle: i64) -> Result<Data, HostError> {
        self.with_inner(|inner| {
            // Each foreign box owns one strong handle on the underlying
            // ComObject; its GC finalizer balances this by calling
            // `com.release`. Without the explicit `add_ref` the host's
            // own intern handle would be consumed by the first
            // collected box, invalidating any sibling box that shares
            // the same id (e.g. a singleton `IUiHost` materialised
            // anew each frame by `ScriptedImmediateDirector`).
            if !inner.host.services.com_table_mut().add_ref(handle) {
                return Err(HostError::message(format!(
                    "foreign_box: invalid COM object handle {}",
                    handle,
                )));
            }
            let pushed = inner
                .host
                .ctx
                .push_foreign(type_tag, handle)
                .map_err(|err| {
                    // Undo the add_ref above so a failed push doesn't
                    // leak a handle.
                    inner.host.services.com_table_mut().release(handle);
                    HostError::message(format!("push foreign failed: {:?}", err))
                });
            pushed?;
            current_frame_stack_pop(inner)
                .ok_or_else(|| HostError::message("push foreign produced no stack value"))
        })
    }

    pub fn has_function(&self, name: &str) -> bool {
        self.with_inner(|inner| inner.host.ctx.has_function(name))
    }

    pub fn with_ctx<R>(&self, body: impl FnOnce(&Context) -> R) -> R {
        self.with_inner(|inner| body(&inner.host.ctx))
    }

    pub fn with_ctx_mut<R>(&self, body: impl FnOnce(&mut Context) -> R) -> R {
        self.with_inner(|inner| body(&mut inner.host.ctx))
    }

    pub fn call_void(&self, name: &str, args: Vec<Data>) -> Result<(), HostError> {
        self.with_inner(|inner| {
            let depth = current_frame_stack_len(inner);
            Self::call_inner(inner, name, args)?;
            if current_frame_stack_len(inner) > depth {
                let _ = current_frame_stack_pop(inner);
            }
            Ok(())
        })
    }

    pub fn call_returning_data(&self, name: &str, args: Vec<Data>) -> Result<Data, HostError> {
        self.with_inner(|inner| {
            let depth = current_frame_stack_len(inner);
            Self::call_inner(inner, name, args)?;
            current_frame_stack_pop(inner).ok_or_else(|| {
                HostError::message(format!(
                    "function '{name}' returned no value (stack depth before call {depth})"
                ))
            })
        })
    }

    pub fn call_method_void(
        &self,
        receiver: Data,
        method_name: &str,
        args: Vec<Data>,
    ) -> Result<(), HostError> {
        self.with_inner(|inner| {
            let depth = current_frame_stack_len(inner);
            Self::call_method_inner(inner, receiver, method_name, args)?;
            if current_frame_stack_len(inner) > depth {
                let _ = current_frame_stack_pop(inner);
            }
            Ok(())
        })
    }

    pub fn call_method_returning_data(
        &self,
        receiver: Data,
        method_name: &str,
        args: Vec<Data>,
    ) -> Result<Data, HostError> {
        self.with_inner(|inner| {
            let depth = current_frame_stack_len(inner);
            Self::call_method_inner(inner, receiver, method_name, args)?;
            current_frame_stack_pop(inner).ok_or_else(|| {
                HostError::message(format!(
                    "method '{method_name}' returned no value (stack depth before call {depth})"
                ))
            })
        })
    }

    /// Roots `data` against GC and returns an opaque handle valid until either
    /// `unroot` or `reload` is called.
    pub fn root(&self, data: Data) -> ScriptDirectorHandle {
        self.with_inner(|inner| {
            let index = inner.host.ctx.add_external_root(data);
            ScriptDirectorHandle {
                index,
                epoch: inner.epoch,
            }
        })
    }

    pub fn unroot(&self, handle: ScriptDirectorHandle) {
        self.with_inner(|inner| {
            if handle.epoch == inner.epoch {
                inner.host.ctx.remove_external_root(handle.index);
            }
        });
    }

    /// Returns a clone of the rooted `Data`, or `None` if the handle is stale
    /// (i.e. its epoch predates the most recent `reload`).
    pub fn deref_handle(&self, handle: ScriptDirectorHandle) -> Option<Data> {
        self.with_inner(|inner| {
            if handle.epoch != inner.epoch {
                return None;
            }
            inner.host.ctx.external_root(handle.index)
        })
    }

    fn call_inner(inner: &mut Inner, name: &str, args: Vec<Data>) -> Result<(), HostError> {
        if !inner.host.ctx.has_function(name) {
            return Err(HostError::message(format!(
                "script function '{name}' is not defined in the loaded package"
            )));
        }
        let host = &mut inner.host;
        std::panic::catch_unwind(AssertUnwindSafe(|| {
            let P7HostContext { ctx, services } = host;
            // `scope_context` parks the active interpreter pointer in a
            // thread-local so that re-entrant invocations from
            // crosscom-wrapped script callbacks (e.g. `IAction.invoke()`
            // dispatched from a host method while body codegen runs)
            // can recover the same `Context` via `with_context`.
            // `scope` does the analogous park for `HostServices` (the
            // `ComObjectTable`). Both are required: SAM-coerced
            // closures (§L2) cross the script/host boundary in both
            // directions during a single render frame.
            scope(services, || {
                scope_context(ctx, || {
                    with_context(|ctx| {
                        ctx.push_function(name, args);
                        ctx.resume()
                    })
                    .map_err(|err| {
                        p7::errors::RuntimeError::Other(format!(
                            "with_context unavailable: {:?}",
                            err
                        ))
                    })?
                })
            })
        }))
        .map_err(|_| HostError::message(format!("script function '{name}' panicked")))?
        .map_err(|err| HostError::message(format!("script function '{name}' failed: {:?}", err)))
    }

    fn call_method_inner(
        inner: &mut Inner,
        receiver: Data,
        method_name: &str,
        args: Vec<Data>,
    ) -> Result<(), HostError> {
        let host = &mut inner.host;
        std::panic::catch_unwind(AssertUnwindSafe(|| {
            let P7HostContext { ctx, services } = host;
            scope(services, || {
                scope_context(ctx, || {
                    with_context(|ctx| {
                        ctx.push_proto_method(receiver, method_name, args)?;
                        ctx.resume()
                    })
                    .map_err(|err| {
                        p7::errors::RuntimeError::Other(format!(
                            "with_context unavailable: {:?}",
                            err
                        ))
                    })?
                })
            })
        }))
        .map_err(|_| HostError::message(format!("script method '{method_name}' panicked")))?
        .map_err(|err| {
            HostError::message(format!("script method '{method_name}' failed: {:?}", err))
        })
    }
}

/// Bridge between reverse-wrap CCWs (in `crosscom-protosept`) and the
/// `ScriptHost` runtime. CCWs carry a `Weak<dyn RuntimeAccess>`
/// pointing at the owning `Rc<ScriptHost>`; on each thunk / Drop they
/// upgrade the weak and call back through this impl to re-enter the
/// interpreter.
///
/// Installing `scope_context` (and `scope`) inside the body is what
/// makes the existing dispatcher / host-fn shims keep working
/// transparently: code that reads the active context via
/// `with_context` (or services via `with_services`) sees the same
/// pointers whether the call entered through `ScriptHost::call_*`
/// (which already brackets `scope`/`scope_context`) or through a
/// CCW thunk (which brackets via this trait impl).
impl RuntimeAccess for ScriptHost {
    fn with_ctx(&self, body: &mut dyn FnMut(&mut Context)) {
        // See `with_inner` re-entrancy contract: nested `with_inner`
        // calls produce overlapping `&mut Inner` references. The
        // existing call paths (e.g. `intern` invoked from inside a
        // host service that was invoked from inside `call_inner`)
        // already exercise this pattern, so a CCW thunk re-entering
        // here mid-call is structurally identical.
        self.with_inner(|inner| {
            let P7HostContext { ctx, services } = &mut inner.host;
            scope(services, || {
                scope_context(ctx, || {
                    let _ = with_context(|c| {
                        body(c);
                    });
                });
            });
        });
    }
}

// Push/pop helpers that target the *current* top stack frame rather than the
// hard-coded entry frame `stack[0]`. Critical for re-entrant ScriptHost
// methods invoked from inside a script call: in that case the relevant frame
// is the script's currently-executing one, not the host's idle entry frame.
fn current_frame_stack_len(inner: &Inner) -> usize {
    inner
        .host
        .ctx
        .stack
        .last()
        .map(|frame| frame.stack.len())
        .unwrap_or(0)
}

fn current_frame_stack_pop(inner: &mut Inner) -> Option<Data> {
    inner.host.ctx.stack.last_mut().and_then(|frame| frame.stack.pop())
}

fn binding_provider(extra: &[(String, String)]) -> Box<dyn ModuleProvider> {
    let mut provider = InMemoryModuleProvider::new();
    provider.add_module(
        "crosscom".to_string(),
        include_str!(concat!(env!("OUT_DIR"), "/crosscom.p7")).to_string(),
    );
    provider.add_module(
        "scripting".to_string(),
        include_str!(concat!(env!("OUT_DIR"), "/scripting.p7")).to_string(),
    );
    provider.add_module(
        "editor_services".to_string(),
        include_str!(concat!(env!("OUT_DIR"), "/editor_services.p7")).to_string(),
    );
    provider.add_module(
        "immediate_director".to_string(),
        include_str!(concat!(env!("OUT_DIR"), "/immediate_director.p7")).to_string(),
    );
    provider.add_module(
        "radiance".to_string(),
        include_str!(concat!(env!("OUT_DIR"), "/radiance.p7")).to_string(),
    );
    provider.add_module("director".to_string(), DIRECTOR_BINDINGS_P7.to_string());
    provider.add_module(
        "editor".to_string(),
        include_str!(concat!(env!("OUT_DIR"), "/editor.p7")).to_string(),
    );
    for (name, source) in extra {
        provider.add_module(name.clone(), source.clone());
    }
    Box::new(provider)
}
