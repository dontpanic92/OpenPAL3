use std::collections::VecDeque;

use crosscom_protosept::HostError;

use super::kinds;
use super::owned::OwnedNode;

pub trait TextureResolver {
    fn resolve(&mut self, com_id: i64) -> Option<imgui::TextureId>;
}

pub trait CommandSink {
    fn enqueue(&mut self, command_id: i32);
}

#[derive(Default)]
pub struct LocalCommandQueue {
    pub queue: VecDeque<i32>,
}

impl CommandSink for LocalCommandQueue {
    fn enqueue(&mut self, c: i32) {
        self.queue.push_back(c)
    }
}

pub struct WalkContext<'a> {
    pub textures: &'a mut dyn TextureResolver,
    pub commands: &'a mut dyn CommandSink,
    pub fonts: &'a [imgui::FontId],
    pub dpi_scale: f32,
}

#[derive(Debug)]
pub enum WalkError {
    ShapeMismatch(String),
}

impl From<WalkError> for HostError {
    fn from(value: WalkError) -> Self {
        HostError::message(format!("ui walker error: {:?}", value))
    }
}

pub trait UiVisitor {
    fn window(
        &mut self,
        title: &str,
        w: f32,
        h: f32,
        flags: u32,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError>;
    fn window_centered(
        &mut self,
        title: &str,
        w: f32,
        h: f32,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError>;
    fn column(
        &mut self,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError>;
    fn row(
        &mut self,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError>;
    fn text(&mut self, s: &str) -> Result<(), WalkError>;
    fn text_with_font(&mut self, font_idx: usize, s: &str) -> Result<(), WalkError>;
    fn button(&mut self, label: &str, w: f32, h: f32, command_id: i32) -> Result<(), WalkError>;
    fn spacer(&mut self, w: f32, h: f32) -> Result<(), WalkError>;
    fn dummy(&mut self, w: f32, h: f32) -> Result<(), WalkError>;
    fn image(&mut self, com_id: i64, w: f32, h: f32) -> Result<(), WalkError>;
    fn table(
        &mut self,
        num_columns: u32,
        cells: &[OwnedNode],
        walk_cell: &mut dyn FnMut(&mut dyn UiVisitor, &OwnedNode) -> Result<(), WalkError>,
    ) -> Result<(), WalkError>;
    fn style_alpha(
        &mut self,
        alpha: f32,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError>;
    fn group(
        &mut self,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError>;
    fn tree_node(
        &mut self,
        label: &str,
        command_id: i32,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError>;
    fn tree_leaf(&mut self, label: &str, command_id: i32) -> Result<(), WalkError>;
}

pub struct UiAdapter<'a> {
    pub ui: &'a imgui::Ui,
    pub ctx: WalkContext<'a>,
    pub table_counter: std::cell::Cell<u32>,
}

impl<'a> UiVisitor for UiAdapter<'a> {
    fn window(
        &mut self,
        title: &str,
        w: f32,
        h: f32,
        flags: u32,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        let [w, h] = self.scaled_size(w, h);
        let mut result = Ok(());
        self.ui
            .window(title)
            .size([w, h], imgui::Condition::Always)
            .flags(imgui::WindowFlags::from_bits_truncate(flags))
            .build(|| result = body(self));
        result
    }

    fn window_centered(
        &mut self,
        title: &str,
        w: f32,
        h: f32,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        let [w, h] = self.scaled_size(w, h);
        let [display_w, display_h] = self.ui.io().display_size;
        let cx = (display_w - w) / 2.0;
        let cy = (display_h - h) / 2.0;
        let mut result = Ok(());
        self.ui
            .window(title)
            .size([w, h], imgui::Condition::Always)
            .position([cx, cy], imgui::Condition::Always)
            .movable(false)
            .title_bar(false)
            .build(|| result = body(self));
        result
    }

    fn column(
        &mut self,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        body(self)
    }

    fn row(
        &mut self,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        body(self)
    }

    fn text(&mut self, s: &str) -> Result<(), WalkError> {
        self.ui.text(s);
        Ok(())
    }

    fn text_with_font(&mut self, font_idx: usize, s: &str) -> Result<(), WalkError> {
        let font = self
            .ctx
            .fonts
            .get(font_idx)
            .copied()
            .unwrap_or_else(|| self.ui.fonts().fonts()[0]);
        let token = self.ui.push_font(font);
        self.ui.text(s);
        token.pop();
        Ok(())
    }

    fn button(&mut self, label: &str, w: f32, h: f32, command_id: i32) -> Result<(), WalkError> {
        // imgui derives a widget id from the visible label, so multiple
        // buttons with the same caption (e.g. several "选择...") would all
        // share an id and only the first becomes interactive. Append a
        // hidden `##cmd_<id>` suffix to make each unique.
        let id_label = format!("{}##cmd_{}", label, command_id);
        let [w, h] = self.scaled_size(w, h);
        let clicked = self.ui.button_with_size(&id_label, [w, h]);
        if clicked {
            log::info!(
                "ui_walker: button '{}' clicked, enqueuing cmd {}",
                label,
                command_id
            );
            self.ctx.commands.enqueue(command_id);
        }
        Ok(())
    }

    fn spacer(&mut self, w: f32, h: f32) -> Result<(), WalkError> {
        let [w, h] = self.scaled_size(w, h);
        self.ui.dummy([w, h]);
        Ok(())
    }

    fn dummy(&mut self, w: f32, h: f32) -> Result<(), WalkError> {
        let [w, h] = self.scaled_size(w, h);
        self.ui.dummy([w, h]);
        Ok(())
    }

    fn image(&mut self, com_id: i64, w: f32, h: f32) -> Result<(), WalkError> {
        let [w, h] = self.scaled_size(w, h);
        if let Some(texture_id) = self.ctx.textures.resolve(com_id) {
            imgui::Image::new(texture_id, [w, h]).build(self.ui);
        } else {
            self.ui.text("[missing texture]");
        }
        Ok(())
    }

    fn table(
        &mut self,
        num_columns: u32,
        cells: &[OwnedNode],
        walk_cell: &mut dyn FnMut(&mut dyn UiVisitor, &OwnedNode) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        // imgui asserts when two BeginTable calls share an id but differ in
        // column count. Use a per-frame monotonic counter to keep ids unique
        // even when several tables of different shapes appear in one render.
        let n = self.table_counter.get();
        self.table_counter.set(n + 1);
        let table_id = format!("ui_table_{}", n);
        if let Some(_table) = self.ui.begin_table(&table_id, num_columns as usize) {
            for cell in cells {
                self.ui.table_next_column();
                walk_cell(self, cell)?;
            }
        }
        Ok(())
    }

    fn style_alpha(
        &mut self,
        alpha: f32,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        let token = self.ui.push_style_var(imgui::StyleVar::Alpha(alpha));
        let result = body(self);
        token.pop();
        result
    }

    fn group(
        &mut self,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        let mut result = Ok(());
        self.ui.group(|| result = body(self));
        result
    }

    fn tree_node(
        &mut self,
        label: &str,
        command_id: i32,
        body: &mut dyn FnMut(&mut dyn UiVisitor) -> Result<(), WalkError>,
    ) -> Result<(), WalkError> {
        let mut result = Ok(());
        let id_label = format!("{}##cmd_{}", label, command_id);
        if let Some(_node) = self.ui.tree_node_config(&id_label).push() {
            if command_id != 0 && self.ui.is_item_clicked() {
                self.ctx.commands.enqueue(command_id);
            }
            result = body(self);
        } else if command_id != 0 && self.ui.is_item_clicked() {
            self.ctx.commands.enqueue(command_id);
        }
        result
    }

    fn tree_leaf(&mut self, label: &str, command_id: i32) -> Result<(), WalkError> {
        let id_label = format!("{}##cmd_{}", label, command_id);
        let _node = self.ui.tree_node_config(&id_label).leaf(true).push();
        if command_id != 0 && self.ui.is_item_clicked() {
            log::info!(
                "ui_walker: tree leaf '{}' clicked, enqueuing cmd {}",
                label,
                command_id
            );
            self.ctx.commands.enqueue(command_id);
        }
        Ok(())
    }
}

impl<'a> UiAdapter<'a> {
    fn scaled_size(&self, w: f32, h: f32) -> [f32; 2] {
        [
            scale_script_dimension(w, self.ctx.dpi_scale),
            scale_script_dimension(h, self.ctx.dpi_scale),
        ]
    }
}

fn scale_script_dimension(value: f32, dpi_scale: f32) -> f32 {
    if value > 0.0 {
        value * dpi_scale
    } else {
        value
    }
}

pub fn walk(node: &OwnedNode, visitor: &mut dyn UiVisitor) -> Result<(), WalkError> {
    match node.kind {
        kinds::WINDOW => {
            let mut body = |v: &mut dyn UiVisitor| walk_children(&node.children, v);
            visitor.window(&node.label, node.w, node.h, node.i1 as u32, &mut body)
        }
        kinds::WINDOW_CENTERED => {
            let mut body = |v: &mut dyn UiVisitor| walk_children(&node.children, v);
            visitor.window_centered(&node.label, node.w, node.h, &mut body)
        }
        kinds::COLUMN => {
            let mut body = |v: &mut dyn UiVisitor| walk_children(&node.children, v);
            visitor.column(&mut body)
        }
        kinds::ROW => {
            let mut body = |v: &mut dyn UiVisitor| walk_children(&node.children, v);
            visitor.row(&mut body)
        }
        kinds::TEXT => visitor.text(&node.label),
        kinds::TEXT_WITH_FONT => visitor.text_with_font(node.i1 as usize, &node.label),
        kinds::BUTTON => visitor.button(&node.label, node.w, node.h, node.i1 as i32),
        kinds::SPACER => visitor.spacer(node.w, node.h),
        kinds::DUMMY => visitor.dummy(node.w, node.h),
        kinds::IMAGE => visitor.image(node.i1, node.w, node.h),
        kinds::TABLE => {
            let mut walk_cell = |v: &mut dyn UiVisitor, cell: &OwnedNode| walk(cell, v);
            visitor.table(node.i1 as u32, &node.children, &mut walk_cell)
        }
        kinds::STYLE_ALPHA => {
            let mut body = |v: &mut dyn UiVisitor| walk_children(&node.children, v);
            visitor.style_alpha(node.w, &mut body)
        }
        kinds::GROUP => {
            let mut body = |v: &mut dyn UiVisitor| walk_children(&node.children, v);
            visitor.group(&mut body)
        }
        kinds::TREE_NODE => {
            let mut body = |v: &mut dyn UiVisitor| walk_children(&node.children, v);
            visitor.tree_node(&node.label, node.i1 as i32, &mut body)
        }
        kinds::TREE_LEAF => visitor.tree_leaf(&node.label, node.i1 as i32),
        other => Err(WalkError::ShapeMismatch(format!(
            "unknown UiNode kind {other}"
        ))),
    }
}

fn walk_children(children: &[OwnedNode], visitor: &mut dyn UiVisitor) -> Result<(), WalkError> {
    for child in children {
        walk(child, visitor)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::scale_script_dimension;

    #[test]
    fn scales_positive_script_dimensions() {
        assert_eq!(scale_script_dimension(120.0, 2.0), 240.0);
    }

    #[test]
    fn preserves_non_positive_script_dimensions() {
        assert_eq!(scale_script_dimension(-1.0, 2.0), -1.0);
        assert_eq!(scale_script_dimension(0.0, 2.0), 0.0);
    }
}
