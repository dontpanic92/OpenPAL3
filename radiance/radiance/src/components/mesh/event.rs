use crosscom::ComRc;

use crate::comdef::IAnimationEventObserver;

#[derive(Clone, Debug)]
pub struct AnimationEvent {
    pub name: String,
    pub tick: f32,
}

impl AnimationEvent {
    pub fn new(name: String, tick: f32) -> Self {
        Self { name, tick }
    }
}

pub struct AnimationEventManager {
    events: Vec<AnimationEvent>,
    events_fired: Vec<bool>,
    current_tick: f32,
    observers: Vec<ComRc<IAnimationEventObserver>>,
}

impl AnimationEventManager {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            events_fired: Vec::new(),
            current_tick: 0.0,
            observers: Vec::new(),
        }
    }

    pub fn set_events(&mut self, events: Vec<AnimationEvent>) {
        self.events_fired = vec![false; events.len()];
        self.events = events;
    }

    pub fn add_event(&mut self, event: AnimationEvent) {
        self.events.push(event);
        self.events_fired.push(false);
    }

    pub fn add_observer(&mut self, observer: ComRc<IAnimationEventObserver>) {
        self.observers.push(observer);
    }

    pub fn tick(&mut self, delta_sec: f32) {
        self.current_tick += delta_sec;

        for (i, event) in self.events.iter().enumerate() {
            if !self.events_fired[i] && self.current_tick >= event.tick {
                for observer in self.observers.iter() {
                    observer.on_animation_event(&event.name);
                }

                self.events_fired[i] = true;
            }
        }
    }

    pub fn reset(&mut self) {
        self.current_tick = 0.0;

        for fired in self.events_fired.iter_mut() {
            *fired = false;
        }
    }

    pub fn clear_observers(&mut self) {
        self.observers.clear();
    }
}
