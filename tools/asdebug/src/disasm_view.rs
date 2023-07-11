use std::path::PathBuf;

use common::store_ext::StoreExt2;
use eframe::egui::{self, CollapsingHeader, ScrollArea, Ui};
use mini_fs::{MiniFs, StoreExt};
use shared::{
    fs::init_virtual_fs,
    openpal4::{app_context::Pal4AppContext, scripting::create_context},
    scripting::angelscript::{disasm, ScriptGlobalContext, ScriptModule},
};

use crate::utils::{get_note, show_strings};

pub struct DisasmView {
    vfs: MiniFs,
    files: Tree,
    file_preview: Option<PathBuf>,
    function_id: usize,
}

impl DisasmView {
    pub fn new() -> Self {
        let vfs = init_virtual_fs("F:\\PAL4", None);
        let mut files = Tree::new("PAL4".to_string(), PathBuf::from("/"), TreeNodeType::Folder);
        Self::construct_folder_tree(&vfs, &mut files);

        Self {
            vfs,
            files,
            file_preview: None,
            function_id: 0,
        }
    }

    pub fn show(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        let window_width = frame.info().window_info.size.x;
        egui::SidePanel::left("left_panel")
            .resizable(true)
            .default_width(400.0)
            .width_range((window_width / 4.)..=(window_width / 4. * 3.))
            .show(ctx, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| match self.files.ui(ui) {
                        Action::None => {}
                        Action::FileClicked(p) => {
                            self.function_id = 0;
                            self.file_preview = Some(p);
                        }
                    });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.file_preview.is_some() {
                let content = self
                    .vfs
                    .read_to_end(self.file_preview.as_ref().unwrap())
                    .unwrap();
                let module = ScriptModule::read_from_buffer(&content).unwrap();
                let context = create_context();
                egui::TopBottomPanel::top("_function_labels")
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        self.show_functions(ctx, ui, &module);
                    });

                egui::SidePanel::right("_strings")
                    .default_width(200.)
                    .width_range(200. ..=500.)
                    .resizable(true)
                    .show_inside(ui, |ui| {
                        ui.label(format!(
                            "Function {}",
                            &module.functions[self.function_id as usize].name
                        ));
                        ui.label("Strings");
                        ScrollArea::both().auto_shrink([false; 2]).show(ui, |ui| {
                            show_strings(ui, &module);
                        });
                    });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    self.show_code_editor(ui, &module, self.function_id, &context);
                });
            }
        });
    }

    fn show_functions(
        &mut self,
        _ctx: &eframe::egui::Context,
        ui: &mut eframe::egui::Ui,
        module: &ScriptModule,
    ) {
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for i in 0..module.functions.len() {
                    if ui.button(&module.functions[i].name).clicked() {
                        self.function_id = i;
                    }
                }
            })
        });
    }

    fn show_code_editor(
        &mut self,
        ui: &mut eframe::egui::Ui,
        module: &ScriptModule,
        function: usize,
        context: &ScriptGlobalContext<Pal4AppContext>,
    ) {
        let insts = disasm(&module.functions[function]);
        let mut content = "".to_string();

        for inst in insts {
            let note = get_note(&inst, module, context);
            content = format!(
                "{}{:?}  {}\n",
                content,
                inst.inst,
                note.unwrap_or("".to_string())
            );
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut content)
                    .font(egui::TextStyle::Monospace) // for cursor height
                    .code_editor()
                    .desired_rows(10)
                    .lock_focus(true)
                    .desired_width(f32::INFINITY),
            );
        });
    }

    fn construct_folder_tree(vfs: &MiniFs, node: &mut Tree) {
        let entries = vfs.entries(&node.path);
        if entries.is_ok() {
            for e in entries.unwrap() {
                if e.is_ok() {
                    let e = e.unwrap();
                    let path = PathBuf::from(&e.name);
                    let filename = path.file_name().unwrap().to_str().unwrap();

                    let new_node = match e.kind {
                        mini_fs::EntryKind::File => Tree::new(
                            filename.to_string(),
                            node.path.join(filename),
                            TreeNodeType::File,
                        ),
                        mini_fs::EntryKind::Dir => {
                            let mut new_node = Tree::new(
                                filename.to_string(),
                                node.path.join(filename),
                                TreeNodeType::Folder,
                            );

                            Self::construct_folder_tree(vfs, &mut new_node);
                            new_node
                        }
                    };

                    node.children.push(new_node);
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum TreeNodeType {
    Folder,
    File,
}

#[derive(Clone, PartialEq)]
enum Action {
    None,
    FileClicked(PathBuf),
}

#[derive(Clone)]
struct Tree {
    name: String,
    children: Vec<Tree>,
    ty: TreeNodeType,
    path: PathBuf,
}

impl Tree {
    pub fn new(name: String, path: PathBuf, ty: TreeNodeType) -> Self {
        Self {
            name,
            children: vec![],
            ty,
            path,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) -> Action {
        Self::ui_impl(ui, 0, self)
    }

    fn ui_impl(ui: &mut Ui, depth: usize, node: &Tree) -> Action {
        CollapsingHeader::new(&node.name)
            .default_open(depth < 1)
            .show(ui, |ui| Self::children_ui(ui, depth, node))
            .body_returned
            .unwrap_or(Action::None)
    }

    fn children_ui(ui: &mut Ui, depth: usize, node: &Tree) -> Action {
        for child in &node.children {
            let action = match child.ty {
                TreeNodeType::Folder => Self::ui_impl(ui, depth + 1, child),
                TreeNodeType::File => Self::file_ui(ui, child),
            };

            if matches!(action, Action::FileClicked(_)) {
                return action;
            }
        }

        Action::None
    }

    fn file_ui(ui: &mut Ui, node: &Tree) -> Action {
        if node.name.to_ascii_lowercase().ends_with(".csb") && ui.button(&node.name).clicked() {
            return Action::FileClicked(node.path.clone());
        }

        Action::None
    }
}
