use std::cell::RefCell;

use crate::{
    comdef::{IDirector, IScene, ISceneManagerImpl},
    ComObject_SceneManager,
};

use crosscom::ComRc;
use imgui::Ui;

pub struct DefaultSceneManager {
    director: RefCell<Option<ComRc<IDirector>>>,
    scenes: RefCell<Vec<ComRc<IScene>>>,
}

ComObject_SceneManager!(super::DefaultSceneManager);

impl DefaultSceneManager {
    pub fn new() -> Self {
        DefaultSceneManager {
            director: RefCell::new(None),
            scenes: RefCell::new(vec![]),
        }
    }
}

impl ISceneManagerImpl for DefaultSceneManager {
    fn update(&self, ui: &Ui, delta_sec: f32) {
        let d = self.director.borrow().clone();
        if let Some(d) = d {
            let director = d.clone();
            let new_director = director.update(ComRc::from_self(self), ui, delta_sec);
            if let Some(d) = new_director {
                d.activate(ComRc::from_self(self));
                self.director.replace(Some(d));
            }
        }

        if let Some(s) = self.scene() {
            s.update(delta_sec);
        }
    }

    fn scene(&self) -> Option<ComRc<IScene>> {
        self.scenes.borrow().last().and_then(|x| Some(x.clone()))
    }

    fn scenes(&self) -> Vec<ComRc<IScene>> {
        self.scenes.borrow().clone()
    }

    fn director(&self) -> Option<ComRc<IDirector>> {
        self.director.borrow().clone()
    }

    fn set_director(&self, director: ComRc<IDirector>) {
        director.activate(ComRc::from_self(self));
        self.director.replace(Some(director));
    }

    fn push_scene(&self, scene: ComRc<IScene>) {
        self.scenes.borrow_mut().push(scene.clone());
        scene.load();
    }

    fn pop_scene(&self) -> Option<ComRc<IScene>> {
        let mut scene = self.scenes.borrow_mut().pop();
        if let Some(s) = scene.as_mut() {
            s.unload();
        }

        scene
    }

    fn unload_all_scenes(&self) {
        while self.pop_scene().is_some() {}
    }

    fn unset_director(&self) {
        self.director.replace(None);
    }
}

impl Drop for DefaultSceneManager {
    fn drop(&mut self) {
        self.unload_all_scenes();
        self.unset_director();
    }
}
