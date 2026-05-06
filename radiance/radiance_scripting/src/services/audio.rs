use std::cell::RefCell;
use std::io::{BufReader, Read};
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::{MiniFs, StoreExt};
use radiance::audio::{AudioEngine, AudioMemorySource, AudioSourceState, Codec};

use crate::comdef::services::{IAudioService, IAudioServiceImpl, IAudioSource, IAudioSourceImpl};

pub struct AudioService {
    engine: Rc<dyn AudioEngine>,
    vfs: Rc<MiniFs>,
}

ComObject_AudioService!(super::AudioService);

impl AudioService {
    pub fn create(engine: Rc<dyn AudioEngine>, vfs: Rc<MiniFs>) -> ComRc<IAudioService> {
        ComRc::from_object(Self { engine, vfs })
    }

    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let file = self.vfs.open(path).ok()?;
        let mut bytes = Vec::new();
        BufReader::new(file).read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }
}

impl IAudioServiceImpl for AudioService {
    fn load(&self, vfs_path: &str, codec: i32) -> Option<ComRc<IAudioSource>> {
        let bytes = self.read(vfs_path)?;
        let mut source = self.engine.create_source();
        source.set_data(bytes, codec_from_int(codec));
        Some(AudioSource::create(source))
    }
}

pub struct AudioSource {
    inner: RefCell<Box<dyn AudioMemorySource>>,
}

ComObject_AudioSource!(super::AudioSource);

impl AudioSource {
    pub fn create(inner: Box<dyn AudioMemorySource>) -> ComRc<IAudioSource> {
        ComRc::from_object(Self {
            inner: RefCell::new(inner),
        })
    }
}

impl IAudioSourceImpl for AudioSource {
    fn play(&self, looped: bool) {
        self.inner.borrow_mut().play(looped);
    }
    fn pause(&self) {
        self.inner.borrow_mut().pause();
    }
    fn stop(&self) {
        self.inner.borrow_mut().stop();
    }
    fn update(&self) {
        self.inner.borrow_mut().update();
    }
    fn state(&self) -> i32 {
        match self.inner.borrow().state() {
            AudioSourceState::Stopped => 0,
            AudioSourceState::Playing => 1,
            AudioSourceState::Paused => 2,
        }
    }
}

fn codec_from_int(codec: i32) -> Codec {
    match codec {
        1 => Codec::Mp3,
        2 => Codec::Ogg,
        _ => Codec::Wav,
    }
}
