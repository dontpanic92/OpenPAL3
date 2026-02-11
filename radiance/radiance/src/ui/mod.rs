use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use imgui::InputText;
use imgui::Ui;
use lazy_static::lazy_static;
use log::warn;
use roxmltree::Document;

use p7::errors::RuntimeError;
use p7::interpreter::context::{Context, ContextResult, Data};
use p7::ModuleProvider;

const P7_UI_MODULE_SOURCE: &str = r#"
// Host-provided module: radiance.ui

pub struct DataUpdate(
    pub key: string,
    pub value: string,
);

pub struct Message(
    pub kind: string,
    pub target_id: ?string,
    pub payload: string,
);

// Payload variants are not supported by the current p7 runtime.
pub enum UiError(
    InvalidXml,
    UnknownId,
    InvalidPatch,
);

pub enum DataError(
    InvalidKey,
    InvalidValue,
);

@intrinsic(name = "radiance.ui.set_ui")
pub fn[throws] set_ui(xml: string);

@intrinsic(name = "radiance.ui.update_data")
pub fn[throws] update_data(updates: ref<array<DataUpdate>>);
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiError {
    InvalidXml(String),
    UnknownId(String),
    InvalidPatch(String),
}

impl std::fmt::Display for UiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiError::InvalidXml(msg) => write!(f, "Invalid XML: {}", msg),
            UiError::UnknownId(id) => write!(f, "Unknown id: {}", id),
            UiError::InvalidPatch(msg) => write!(f, "Invalid patch: {}", msg),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataError {
    InvalidKey(String),
    InvalidValue(String),
}

impl std::fmt::Display for DataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataError::InvalidKey(key) => write!(f, "Invalid key: {}", key),
            DataError::InvalidValue(value) => write!(f, "Invalid value: {}", value),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DataUpdate {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub kind: String,
    pub target_id: Option<String>,
    pub payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum UiNodeKind {
    Root,
    VBox,
    HBox,
    Label,
    Button,
    TextInput,
    Unknown(String),
}

#[derive(Debug, Clone)]
struct UiNode {
    kind: UiNodeKind,
    id: Option<String>,
    text: Option<String>,
    on_click: Option<String>,
    on_change: Option<String>,
    children: Vec<UiNode>,
}

#[derive(Debug, Clone)]
struct InputState {
    text: String,
    binding_key: Option<String>,
    dirty: bool,
}

struct InputDescriptor {
    raw_text: String,
    binding_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchMode {
    ReplaceChildren,
    AppendChildren,
}

enum UiDocument {
    Full(UiNode),
    Patch {
        target: String,
        mode: PatchMode,
        children: Vec<UiNode>,
    },
}

pub struct UiInterop {
    root: Option<UiNode>,
    data: HashMap<String, String>,
    messages: Vec<Message>,
    input_states: HashMap<String, InputState>,
}

impl UiInterop {
    pub fn new() -> Self {
        Self {
            root: None,
            data: HashMap::new(),
            messages: Vec::new(),
            input_states: HashMap::new(),
        }
    }

    pub fn set_ui(&mut self, xml: &str) -> Result<(), UiError> {
        match parse_ui_document(xml)? {
            UiDocument::Full(root) => {
                self.root = Some(root);
                self.rebuild_input_states();
                Ok(())
            }
            UiDocument::Patch {
                target,
                mode,
                children,
            } => {
                let root = self
                    .root
                    .as_mut()
                    .ok_or_else(|| UiError::UnknownId(target.clone()))?;
                let target_node = find_node_mut(root, &target)
                    .ok_or_else(|| UiError::UnknownId(target.clone()))?;

                match mode {
                    PatchMode::ReplaceChildren => {
                        target_node.children = children;
                    }
                    PatchMode::AppendChildren => {
                        target_node.children.extend(children);
                    }
                }

                self.rebuild_input_states();
                Ok(())
            }
        }
    }

    pub fn update_data(&mut self, updates: &[DataUpdate]) -> Result<(), DataError> {
        for update in updates {
            if update.key.trim().is_empty() {
                return Err(DataError::InvalidKey(update.key.clone()));
            }
            self.data.insert(update.key.clone(), update.value.clone());
            self.sync_input_state_for_key(&update.key, &update.value);
        }

        Ok(())
    }

    pub fn render(&mut self, ui: &Ui) {
        let Some(root) = self.root.clone() else {
            return;
        };

        let data_snapshot = self.data.clone();
        self.render_node(ui, &root, "root", &data_snapshot);
    }

    pub fn drain_messages(&mut self) -> Vec<Message> {
        let mut out = Vec::new();
        std::mem::swap(&mut out, &mut self.messages);
        out
    }

    fn render_node(
        &mut self,
        ui: &Ui,
        node: &UiNode,
        path: &str,
        data: &HashMap<String, String>,
    ) {
        let id_token = ui.push_id(node.id.as_deref().unwrap_or(path));

        match &node.kind {
            UiNodeKind::Root => {
                for (idx, child) in node.children.iter().enumerate() {
                    let child_path = format!("{}:{}", path, idx);
                    self.render_node(ui, child, &child_path, data);
                }
            }
            UiNodeKind::VBox => {
                for (idx, child) in node.children.iter().enumerate() {
                    let child_path = format!("{}:{}", path, idx);
                    self.render_node(ui, child, &child_path, data);
                }
            }
            UiNodeKind::HBox => {
                for (idx, child) in node.children.iter().enumerate() {
                    if idx > 0 {
                        ui.same_line();
                    }
                    let child_path = format!("{}:{}", path, idx);
                    self.render_node(ui, child, &child_path, data);
                }
            }
            UiNodeKind::Label => {
                let text = resolve_text_from(data, node.text.as_deref().unwrap_or(""));
                ui.text(text);
            }
            UiNodeKind::Button => {
                let text = resolve_text_from(data, node.text.as_deref().unwrap_or(""));
                if ui.button(text) {
                    if let Some(kind) = node.on_click.as_ref() {
                        self.messages.push(Message {
                            kind: kind.clone(),
                            target_id: node.id.clone(),
                            payload: String::new(),
                        });
                    }
                }
            }
            UiNodeKind::TextInput => {
                let input_id = node.id.clone().unwrap_or_else(|| path.to_string());
                let raw_text = node.text.as_deref().unwrap_or("");
                let resolved_text = resolve_text_from(data, raw_text);
                let binding_key = extract_binding_key(raw_text);
                let state = self
                    .input_states
                    .entry(input_id.clone())
                    .or_insert_with(|| InputState {
                        text: resolved_text,
                        binding_key,
                        dirty: false,
                    });

                let label = format!("##{}", input_id);
                let changed = InputText::new(ui, &label, &mut state.text).build();
                if changed {
                    state.dirty = true;
                    if let Some(kind) = node.on_change.as_ref() {
                        self.messages.push(Message {
                            kind: kind.clone(),
                            target_id: node.id.clone(),
                            payload: state.text.clone(),
                        });
                    }
                }
            }
            UiNodeKind::Unknown(_) => {
                for (idx, child) in node.children.iter().enumerate() {
                    let child_path = format!("{}:{}", path, idx);
                    self.render_node(ui, child, &child_path, data);
                }
            }
        }

        drop(id_token);
    }

    fn rebuild_input_states(&mut self) {
        let mut bindings: HashMap<String, InputDescriptor> = HashMap::new();
        if let Some(root) = self.root.as_ref() {
            collect_text_inputs(root, &mut bindings);
        }

        self.input_states.retain(|id, _| bindings.contains_key(id));
        let data_snapshot = self.data.clone();

        for (id, descriptor) in bindings {
            let entry = self.input_states.entry(id.clone()).or_insert_with(|| {
                let value = if let Some(key) = descriptor.binding_key.as_ref() {
                    data_snapshot.get(key).cloned().unwrap_or_default()
                } else {
                    resolve_text_from(&data_snapshot, &descriptor.raw_text)
                };
                InputState {
                    text: value,
                    binding_key: descriptor.binding_key.clone(),
                    dirty: false,
                }
            });

            entry.binding_key = descriptor.binding_key.clone();
            if !entry.dirty {
                if let Some(key) = descriptor.binding_key.as_ref() {
                    if let Some(value) = data_snapshot.get(key) {
                        entry.text = value.clone();
                    }
                } else {
                    entry.text = resolve_text_from(&data_snapshot, &descriptor.raw_text);
                }
            }
        }
    }

    fn sync_input_state_for_key(&mut self, key: &str, value: &str) {
        for state in self.input_states.values_mut() {
            if let Some(binding_key) = state.binding_key.as_ref() {
                if binding_key == key {
                    state.text = value.to_string();
                    state.dirty = false;
                }
            }
        }
    }
}

fn collect_text_inputs(node: &UiNode, out: &mut HashMap<String, InputDescriptor>) {
    if matches!(node.kind, UiNodeKind::TextInput) {
        if let Some(id) = node.id.as_ref() {
            let raw_text = node.text.as_deref().unwrap_or("").to_string();
            let binding_key = extract_binding_key(&raw_text);
            out.insert(
                id.clone(),
                InputDescriptor {
                    raw_text,
                    binding_key,
                },
            );
        }
    }

    for child in &node.children {
        collect_text_inputs(child, out);
    }
}

fn extract_binding_key(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() > 2 {
        let inner = &trimmed[1..trimmed.len() - 1];
        if !inner.contains('}') {
            return Some(inner.to_string());
        }
    }

    None
}

fn resolve_text_from(data: &HashMap<String, String>, text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;

    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];

        if let Some(end) = after.find('}') {
            let key = &after[..end];
            let value = data.get(key).map(|s| s.as_str()).unwrap_or("");
            out.push_str(value);
            rest = &after[end + 1..];
        } else {
            out.push_str(&rest[start..]);
            rest = "";
        }
    }

    out.push_str(rest);
    out
}

fn find_node_mut<'a>(node: &'a mut UiNode, id: &str) -> Option<&'a mut UiNode> {
    if node.id.as_deref() == Some(id) {
        return Some(node);
    }

    for child in node.children.iter_mut() {
        if let Some(found) = find_node_mut(child, id) {
            return Some(found);
        }
    }

    None
}

fn parse_ui_document(xml: &str) -> Result<UiDocument, UiError> {
    let document = Document::parse(xml).map_err(|err| UiError::InvalidXml(err.to_string()))?;
    let root = document.root_element();

    match root.tag_name().name() {
        "ui" => {
            let children = parse_children(&root)?;
            Ok(UiDocument::Full(UiNode {
                kind: UiNodeKind::Root,
                id: None,
                text: None,
                on_click: None,
                on_change: None,
                children,
            }))
        }
        "patch" => {
            let target = root
                .attribute("target")
                .ok_or_else(|| UiError::InvalidPatch("missing target".to_string()))?
                .to_string();
            let mode = match root.attribute("mode").unwrap_or("replace-children") {
                "replace-children" => PatchMode::ReplaceChildren,
                "append-children" => PatchMode::AppendChildren,
                other => {
                    return Err(UiError::InvalidPatch(format!(
                        "unknown mode: {}",
                        other
                    )))
                }
            };

            let children = parse_children(&root)?;
            Ok(UiDocument::Patch {
                target,
                mode,
                children,
            })
        }
        other => Err(UiError::InvalidXml(format!(
            "unexpected root element: {}",
            other
        ))),
    }
}

fn parse_children(node: &roxmltree::Node) -> Result<Vec<UiNode>, UiError> {
    let mut children = Vec::new();
    for child in node.children().filter(|n| n.is_element()) {
        children.push(parse_node(child)?);
    }
    Ok(children)
}

fn parse_node(node: roxmltree::Node) -> Result<UiNode, UiError> {
    let name = node.tag_name().name();
    let kind = match name {
        "vbox" => UiNodeKind::VBox,
        "hbox" => UiNodeKind::HBox,
        "label" => UiNodeKind::Label,
        "button" => UiNodeKind::Button,
        "text_input" => UiNodeKind::TextInput,
        _ => UiNodeKind::Unknown(name.to_string()),
    };

    let id = node.attribute("id").map(|s| s.to_string());
    let text = node.attribute("text").map(|s| s.to_string());
    let on_click = node.attribute("on_click").map(|s| s.to_string());
    let on_change = node.attribute("on_change").map(|s| s.to_string());
    let children = parse_children(&node)?;

    Ok(UiNode {
        kind,
        id,
        text,
        on_click,
        on_change,
        children,
    })
}

pub struct RadianceUiModuleProvider {
    inner: Box<dyn ModuleProvider>,
}

impl RadianceUiModuleProvider {
    pub fn new(inner: Box<dyn ModuleProvider>) -> Self {
        Self { inner }
    }
}

impl ModuleProvider for RadianceUiModuleProvider {
    fn load_module(&self, module_path: &str) -> Option<String> {
        if module_path == "radiance.ui" {
            return Some(P7_UI_MODULE_SOURCE.to_string());
        }
        self.inner.load_module(module_path)
    }

    fn clone_boxed(&self) -> Box<dyn ModuleProvider> {
        Box::new(Self {
            inner: self.inner.clone_boxed(),
        })
    }
}

lazy_static! {
    static ref UI_INTEROP_HANDLE: Mutex<Option<Arc<Mutex<UiInterop>>>> = Mutex::new(None);
}

pub fn install_ui_interop_handle(handle: Arc<Mutex<UiInterop>>) {
    let mut slot = UI_INTEROP_HANDLE
        .lock()
        .expect("UI interop handle lock poisoned");
    *slot = Some(handle);
}

pub fn register_p7_ui_host_functions(ctx: &mut Context) {
    ctx.register_host_function("radiance.ui.set_ui".to_string(), host_set_ui);
    ctx.register_host_function("radiance.ui.update_data".to_string(), host_update_data);
}

fn host_set_ui(ctx: &mut Context) -> ContextResult<()> {
    let xml = pop_string(ctx, "radiance.ui.set_ui")?;

    let handle = get_ui_interop_handle()?;
    let mut interop = handle
        .lock()
        .map_err(|_| RuntimeError::Other("UI interop lock poisoned".to_string()))?;

    match interop.set_ui(&xml) {
        Ok(()) => Ok(()),
        Err(err) => {
            warn!("UI set_ui failed: {}", err);
            throw_exception(ctx, UiErrorKind::from(&err) as i32)
        }
    }
}

fn host_update_data(ctx: &mut Context) -> ContextResult<()> {
    let updates_data = ctx
        .stack_frame_mut()?
        .stack
        .pop()
        .ok_or(RuntimeError::StackUnderflow)?;

    let updates = parse_data_updates(ctx, updates_data)?;

    let handle = get_ui_interop_handle()?;
    let mut interop = handle
        .lock()
        .map_err(|_| RuntimeError::Other("UI interop lock poisoned".to_string()))?;

    match interop.update_data(&updates) {
        Ok(()) => Ok(()),
        Err(err) => {
            warn!("UI update_data failed: {}", err);
            throw_exception(ctx, DataErrorKind::from(&err) as i32)
        }
    }
}

fn parse_data_updates(ctx: &mut Context, data: Data) -> ContextResult<Vec<DataUpdate>> {
    let Data::Array(values) = data else {
        return Err(RuntimeError::Other(
            "update_data expects ref<array<DataUpdate>>".to_string(),
        ));
    };

    let mut updates = Vec::with_capacity(values.len());
    for value in values {
        let Data::StructRef(struct_idx) = value else {
            return Err(RuntimeError::Other(
                "update_data expects array of DataUpdate".to_string(),
            ));
        };
        let struct_value = ctx
            .heap
            .get(struct_idx as usize)
            .ok_or(RuntimeError::Other("Invalid DataUpdate reference".to_string()))?;

        if struct_value.fields.len() < 2 {
            return Err(RuntimeError::Other(
                "DataUpdate requires key and value".to_string(),
            ));
        }

        let key = expect_string(struct_value.fields[0].clone(), "DataUpdate.key")?;
        let value = expect_string(struct_value.fields[1].clone(), "DataUpdate.value")?;
        updates.push(DataUpdate { key, value });
    }

    Ok(updates)
}

fn pop_string(ctx: &mut Context, label: &str) -> ContextResult<String> {
    let data = ctx
        .stack_frame_mut()?
        .stack
        .pop()
        .ok_or(RuntimeError::StackUnderflow)?;
    expect_string(data, label)
}

fn expect_string(data: Data, label: &str) -> ContextResult<String> {
    match data {
        Data::String(value) => Ok(value),
        other => Err(RuntimeError::Other(format!(
            "{} expected string, got {:?}",
            label, other
        ))),
    }
}

fn get_ui_interop_handle() -> ContextResult<Arc<Mutex<UiInterop>>> {
    let guard = UI_INTEROP_HANDLE
        .lock()
        .map_err(|_| RuntimeError::Other("UI interop handle lock poisoned".to_string()))?;
    guard
        .as_ref()
        .cloned()
        .ok_or(RuntimeError::Other("UI interop not installed".to_string()))
}

fn throw_exception(ctx: &mut Context, value: i32) -> ContextResult<()> {
    ctx.stack.pop();
    if let Ok(frame) = ctx.stack_frame_mut() {
        frame.stack.push(Data::Exception(value));
    }
    Ok(())
}

#[repr(i32)]
enum UiErrorKind {
    InvalidXml = 0,
    UnknownId = 1,
    InvalidPatch = 2,
}

impl From<&UiError> for UiErrorKind {
    fn from(value: &UiError) -> Self {
        match value {
            UiError::InvalidXml(_) => UiErrorKind::InvalidXml,
            UiError::UnknownId(_) => UiErrorKind::UnknownId,
            UiError::InvalidPatch(_) => UiErrorKind::InvalidPatch,
        }
    }
}

#[repr(i32)]
enum DataErrorKind {
    InvalidKey = 0,
    InvalidValue = 1,
}

impl From<&DataError> for DataErrorKind {
    fn from(value: &DataError) -> Self {
        match value {
            DataError::InvalidKey(_) => DataErrorKind::InvalidKey,
            DataError::InvalidValue(_) => DataErrorKind::InvalidValue,
        }
    }
}
