use std::cell::RefCell;

use crate::comdef::{IDirector, IScene, ISceneManagerImpl};

use crosscom::ComRc;

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

    /// Inherent counterpart to the formerly-IDL `scenes`. Access from
    /// a `ComRc<ISceneManager>` via [`ISceneManagerExt`].
    pub fn scenes(&self) -> Vec<ComRc<IScene>> {
        self.scenes.borrow().clone()
    }

    /// Single funnel for every director transition. Fires
    /// `deactivate` on the previously-installed director (if any)
    /// *before* the new director's `activate` runs and *before* the
    /// old ComRc is released. Callers that just want to clear the
    /// director pass `None`.
    fn replace_director(&self, new: Option<ComRc<IDirector>>) {
        // Snapshot the old binding out of the RefCell first so a
        // `deactivate` impl that re-enters the scene manager (e.g.
        // pushes/pops scenes during shutdown) sees a consistent
        // "no director currently installed" state.
        let old = self.director.replace(None);
        if let Some(old) = old {
            old.deactivate();
        }

        if let Some(n) = new {
            n.activate();
            self.director.replace(Some(n));
        }
    }
}

impl ISceneManagerImpl for DefaultSceneManager {
    fn update(&self, delta_sec: f32) {
        let d = self.director.borrow().clone();
        if let Some(d) = d {
            let new_director = d.update(delta_sec);
            if let Some(new) = new_director {
                self.replace_director(Some(new));
            }
        }

        if let Some(s) = self.scene() {
            s.update(delta_sec);
        }
    }

    fn scene(&self) -> Option<ComRc<IScene>> {
        self.scenes.borrow().last().and_then(|x| Some(x.clone()))
    }

    fn director(&self) -> Option<ComRc<IDirector>> {
        self.director.borrow().clone()
    }

    fn set_director(&self, director: ComRc<IDirector>) {
        self.replace_director(Some(director));
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
        self.replace_director(None);
    }
}

impl Drop for DefaultSceneManager {
    fn drop(&mut self) {
        self.unload_all_scenes();
        self.unset_director();
    }
}

/// Extension trait exposing `DefaultSceneManager`'s formerly-IDL
/// accessors on a `ComRc<ISceneManager>` handle.
pub trait ISceneManagerExt {
    fn scenes(&self) -> Vec<ComRc<IScene>>;
}

impl ISceneManagerExt for ComRc<crate::comdef::ISceneManager> {
    fn scenes(&self) -> Vec<ComRc<IScene>> {
        self.with_inner::<DefaultSceneManager, _, _>(|sm| sm.scenes())
    }
}
