use eframe::egui::{self};
use shared::{
    openpal4::scripting::Pal4AppContext,
    scripting::angelscript::{AsInst, AsInstInstance, ScriptGlobalContext, ScriptModule},
};

pub fn show_strings(ui: &mut egui::Ui, module: &ScriptModule) {
    egui::Grid::new("string_grid")
        .num_columns(2)
        .spacing([4.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            for i in 0..module.strings.len() {
                ui.add(|ui: &mut egui::Ui| ui.label(format!("{}", i)));
                ui.label(format!("{}", module.strings[i]));
                ui.end_row();
            }
        });
}

pub fn show_objects(ui: &mut egui::Ui, objects: &[Option<String>]) {
    egui::Grid::new("object_grid")
        .num_columns(2)
        .spacing([4.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            for i in 0..objects.len() {
                ui.add(|ui: &mut egui::Ui| ui.label(format!("{}", i)));
                ui.label(format!("{:?}", objects[i]));
                ui.end_row();
            }
        });
}

pub fn get_note(
    inst: &AsInstInstance,
    module: &ScriptModule,
    context: &ScriptGlobalContext<Pal4AppContext>,
) -> Option<String> {
    let note = match inst.inst {
        AsInst::CallSys { function_index } => Some(
            context.functions()[(-function_index - 1) as usize]
                .name
                .clone(),
        ),
        AsInst::Str { index } => Some(format!("\"{}\"", module.strings[index as usize].as_str())),
        AsInst::Call { function } => Some(module.functions[function as usize].name.clone()),
        _ => None,
    };

    note.map(|note| format!("// {}", note))
}
