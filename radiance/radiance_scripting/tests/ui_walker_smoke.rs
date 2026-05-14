use std::collections::HashSet;

use radiance_scripting::ui_walker::kinds;
use radiance_scripting::{walk, LocalCommandQueue, OwnedNode, UiVisitor, WalkError};

#[derive(Debug, PartialEq)]
enum Event {
    Window {
        title: String,
        w: f32,
        h: f32,
        flags: u32,
    },
    EnterContainer(&'static str),
    ExitContainer(&'static str),
    Text(String),
    Button {
        label: String,
        w: f32,
        h: f32,
        command: i32,
    },
    Image {
        com_id: i64,
        w: f32,
        h: f32,
    },
    TreeNode {
        label: String,
        command: i32,
    },
    TreeLeaf {
        label: String,
        command: i32,
    },
}

#[derive(Default)]
struct TestRecorder {
    events: Vec<Event>,
    commands: LocalCommandQueue,
    commands_to_click: HashSet<i32>,
    missing_textures: HashSet<i64>,
}

impl UiVisitor for TestRecorder {
    fn window(
        &mut self,
        title: &str,
        w: f32,
        h: f32,
        flags: u32,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        self.events.push(Event::Window {
            title: title.to_owned(),
            w,
            h,
            flags,
        });
        self.events.push(Event::EnterContainer("window"));
        body(self)?;
        self.events.push(Event::ExitContainer("window"));
        Ok(())
    }

    fn window_centered(
        &mut self,
        title: &str,
        w: f32,
        h: f32,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        self.window(title, w, h, 0, body)
    }

    fn column(
        &mut self,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        self.events.push(Event::EnterContainer("column"));
        body(self)?;
        self.events.push(Event::ExitContainer("column"));
        Ok(())
    }

    fn row(
        &mut self,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        self.events.push(Event::EnterContainer("row"));
        body(self)?;
        self.events.push(Event::ExitContainer("row"));
        Ok(())
    }

    fn text(&mut self, s: &str) -> Result<(), WalkError> {
        self.events.push(Event::Text(s.to_owned()));
        Ok(())
    }

    fn text_with_font(&mut self, _font_idx: usize, s: &str) -> Result<(), WalkError> {
        self.text(s)
    }

    fn button(&mut self, label: &str, w: f32, h: f32, command_id: i32) -> Result<(), WalkError> {
        self.events.push(Event::Button {
            label: label.to_owned(),
            w,
            h,
            command: command_id,
        });
        if self.commands_to_click.contains(&command_id) {
            self.commands.queue.push_back(command_id);
        }
        Ok(())
    }

    fn spacer(&mut self, w: f32, h: f32) -> Result<(), WalkError> {
        self.events.push(Event::Image { com_id: -1, w, h });
        Ok(())
    }

    fn dummy(&mut self, w: f32, h: f32) -> Result<(), WalkError> {
        self.spacer(w, h)
    }

    fn image(&mut self, com_id: i64, w: f32, h: f32) -> Result<(), WalkError> {
        if self.missing_textures.contains(&com_id) {
            self.events
                .push(Event::Text("[missing texture]".to_owned()));
        } else {
            self.events.push(Event::Image { com_id, w, h });
        }
        Ok(())
    }

    fn table(
        &mut self,
        _num_columns: u32,
        cells: &[OwnedNode],
        walk_cell: &mut dyn FnMut(&mut dyn UiVisitor, &OwnedNode) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        self.events.push(Event::EnterContainer("table"));
        for cell in cells {
            walk_cell(self, cell)?;
        }
        self.events.push(Event::ExitContainer("table"));
        Ok(())
    }

    fn style_alpha(
        &mut self,
        _alpha: f32,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        self.events.push(Event::EnterContainer("style_alpha"));
        body(self)?;
        self.events.push(Event::ExitContainer("style_alpha"));
        Ok(())
    }

    fn group(
        &mut self,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        self.events.push(Event::EnterContainer("group"));
        body(self)?;
        self.events.push(Event::ExitContainer("group"));
        Ok(())
    }

    fn tree_node(
        &mut self,
        label: &str,
        command_id: i32,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        self.events.push(Event::TreeNode {
            label: label.to_owned(),
            command: command_id,
        });
        if self.commands_to_click.contains(&command_id) {
            self.commands.queue.push_back(command_id);
        }
        self.events.push(Event::EnterContainer("tree_node"));
        body(self)?;
        self.events.push(Event::ExitContainer("tree_node"));
        Ok(())
    }

    fn tree_leaf(&mut self, label: &str, command_id: i32) -> Result<(), WalkError> {
        self.events.push(Event::TreeLeaf {
            label: label.to_owned(),
            command: command_id,
        });
        if self.commands_to_click.contains(&command_id) {
            self.commands.queue.push_back(command_id);
        }
        Ok(())
    }

    fn multiline_text(&mut self, content: &str, _w: f32, _h: f32) -> Result<(), WalkError> {
        self.events.push(Event::Text(content.to_owned()));
        Ok(())
    }

    fn tab_bar(
        &mut self,
        _id: &str,
        items: &[OwnedNode],
        walk_item: &mut dyn FnMut(&mut dyn UiVisitor, &OwnedNode) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        self.events.push(Event::EnterContainer("tab_bar"));
        for item in items {
            walk_item(self, item)?;
        }
        self.events.push(Event::ExitContainer("tab_bar"));
        Ok(())
    }

    fn tab_item(
        &mut self,
        _label: &str,
        _close_command_id: i32,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        self.events.push(Event::EnterContainer("tab_item"));
        body(self)?;
        self.events.push(Event::ExitContainer("tab_item"));
        Ok(())
    }

    fn child_window(
        &mut self,
        _id: &str,
        _w: f32,
        _h: f32,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        self.events.push(Event::EnterContainer("child_window"));
        body(self)?;
        self.events.push(Event::ExitContainer("child_window"));
        Ok(())
    }

    fn same_line(&mut self) -> Result<(), WalkError> {
        Ok(())
    }

    fn image_fit(&mut self, com_id: i64, src_w: f32, src_h: f32) -> Result<(), WalkError> {
        if self.missing_textures.contains(&com_id) {
            self.events
                .push(Event::Text("[missing texture]".to_owned()));
        } else {
            self.events.push(Event::Image {
                com_id,
                w: src_w,
                h: src_h,
            });
        }
        Ok(())
    }
}

#[test]
fn walks_window_text_and_button_in_order() {
    let node = window(
        "t",
        100.0,
        200.0,
        0,
        vec![text("hello"), button("ok", 50.0, 20.0, 42)],
    );

    let mut recorder = TestRecorder::default();
    walk(&node, &mut recorder).unwrap();

    assert_eq!(
        recorder.events,
        vec![
            Event::Window {
                title: "t".to_owned(),
                w: 100.0,
                h: 200.0,
                flags: 0,
            },
            Event::EnterContainer("window"),
            Event::Text("hello".to_owned()),
            Event::Button {
                label: "ok".to_owned(),
                w: 50.0,
                h: 20.0,
                command: 42,
            },
            Event::ExitContainer("window"),
        ]
    );
}

#[test]
fn recorder_can_enqueue_clicked_button_command() {
    let node = button("ok", 50.0, 20.0, 42);
    let mut recorder = TestRecorder::default();
    recorder.commands_to_click.insert(42);

    walk(&node, &mut recorder).unwrap();

    assert_eq!(
        recorder.commands.queue.into_iter().collect::<Vec<_>>(),
        vec![42]
    );
}

#[test]
fn recorder_can_mirror_missing_texture_fallback() {
    let node = image(999, 50.0, 50.0);
    let mut recorder = TestRecorder::default();
    recorder.missing_textures.insert(999);

    walk(&node, &mut recorder).unwrap();

    assert_eq!(
        recorder.events,
        vec![Event::Text("[missing texture]".to_owned())]
    );
}

#[test]
fn recorder_walks_tree_nodes_and_enqueues_leaf_commands() {
    let node = tree_node("dir", 55, vec![tree_leaf("file.txt", 77)]);
    let mut recorder = TestRecorder::default();
    recorder.commands_to_click.insert(77);

    walk(&node, &mut recorder).unwrap();

    assert_eq!(
        recorder.events,
        vec![
            Event::TreeNode {
                label: "dir".to_owned(),
                command: 55,
            },
            Event::EnterContainer("tree_node"),
            Event::TreeLeaf {
                label: "file.txt".to_owned(),
                command: 77,
            },
            Event::ExitContainer("tree_node"),
        ]
    );
    assert_eq!(
        recorder.commands.queue.into_iter().collect::<Vec<_>>(),
        vec![77]
    );
}

fn window(title: &str, w: f32, h: f32, flags: i64, children: Vec<OwnedNode>) -> OwnedNode {
    node(kinds::WINDOW, title, w, h, flags, 0, children)
}

fn text(value: &str) -> OwnedNode {
    node(kinds::TEXT, value, 0.0, 0.0, 0, 0, Vec::new())
}

fn button(label: &str, w: f32, h: f32, command_id: i64) -> OwnedNode {
    node(kinds::BUTTON, label, w, h, command_id, 0, Vec::new())
}

fn image(com_id: i64, w: f32, h: f32) -> OwnedNode {
    node(kinds::IMAGE, "", w, h, com_id, 0, Vec::new())
}

fn tree_node(label: &str, command_id: i64, children: Vec<OwnedNode>) -> OwnedNode {
    node(kinds::TREE_NODE, label, 0.0, 0.0, command_id, 0, children)
}

fn tree_leaf(label: &str, command_id: i64) -> OwnedNode {
    node(kinds::TREE_LEAF, label, 0.0, 0.0, command_id, 0, Vec::new())
}

fn node(
    kind: i64,
    label: &str,
    w: f32,
    h: f32,
    i1: i64,
    i2: i64,
    children: Vec<OwnedNode>,
) -> OwnedNode {
    OwnedNode {
        kind,
        label: label.to_owned(),
        w,
        h,
        i1,
        i2,
        children,
    }
}
