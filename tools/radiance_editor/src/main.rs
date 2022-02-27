#![feature(arbitrary_self_types)]
use application::EditorApplication;

mod application;
mod director;
mod scene;
mod ui;

fn main() {
    let mut application = EditorApplication::new(None);
    application.initialize();
    application.run();
}
