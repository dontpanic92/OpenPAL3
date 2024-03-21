use std::rc::Rc;

use shared::{fs::init_virtual_fs, openswd5::asset_loader::AssetLoader};

use crate::components::{table_view::TableViewEvents, window::Window, Events};

pub fn init_window(window: &mut Window) {
    let vfs = init_virtual_fs("F:\\SteamLibrary\\steamapps\\common\\SWDHC", None);
    let index = Rc::new(AssetLoader::load_index(&vfs).unwrap());
    let event_emitter = window.event_emitter.clone();

    let mut table = window.find_component("test").unwrap().as_type();
    let table = table.as_table_view().unwrap();

    table.headers = vec!["Index".to_string()];
    let mut items = Vec::new();
    for i in 0..index.len() {
        items.push(vec![format!(
            "{} {}",
            i,
            if index[i].is_none() { "[None]" } else { "" }
        )]);
    }

    table.items = items;
    table.set_event_emitter(event_emitter);

    window.set_event_handler(move |window, event| match event {
        Events::TableView(TableViewEvents::Selected(row_index)) => {
            window
                .find_component("tree")
                .unwrap()
                .as_type()
                .as_label()
                .unwrap()
                .text = serde_json::to_string_pretty(&index[row_index].as_ref().unwrap()).unwrap();
        }
        _ => {}
    });
}
