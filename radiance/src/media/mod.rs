extern crate ffmpeg;
mod audio;
mod video;

use ffmpeg::{
    codec::{
        decoder::{Audio as AudioDecoder, Decoder as FFmpegDecoder, Video as VideoDecoder},
        packet::Packet,
    },
    format::{context::Input, stream::Stream},
};

use std::{
    collections::VecDeque,
    rc::Rc,
    sync::{Arc, Mutex, RwLock, Weak},
    thread,
    time::{Duration, Instant},
};

use self::{
    audio::AudioStreamData,
    video::{FFMpegVideoSource, VideoStreamData},
};
use crate::{
    audio::{AudioSource, OpenAlAudioSource},
    rendering::ComponentFactory,
};
use alto::{Alto, Context};
use imgui::TextureId;
use log::debug;

const VIDEO_PACKET_QUEUE_MAX: usize = 1024;
const AUDIO_PACKET_QUEUE_MAX: usize = 512;

const QUEUE_FULL_SLEEP: u64 = 10;
const ENDED_SLEEP: u64 = 50;

pub trait MediaEngine {
    fn create_video_source(&self, factory: Rc<dyn ComponentFactory>) -> Box<dyn VideoSource>;
    fn create_audio_source(&self) -> Box<dyn AudioSource>;
}
pub struct FFMpegMediaEngine {
    context: Rc<Context>,
}

impl MediaEngine for FFMpegMediaEngine {
    fn create_video_source(&self, factory: Rc<dyn ComponentFactory>) -> Box<dyn VideoSource> {
        Box::new(FFMpegVideoSource::new(factory))
    }
    fn create_audio_source(&self) -> Box<dyn AudioSource> {
        Box::new(OpenAlAudioSource::new(self.context.clone()))
    }
}

impl FFMpegMediaEngine {
    pub fn new() -> Self {
        let alto = Alto::load_default().unwrap();
        let device = alto.open(None).unwrap();
        let context = Rc::new(device.new_context(None).unwrap());

        Self { context }
    }
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum VideoSourceState {
    Stopped,
    Playing,
    Paused,
}

pub trait VideoSource {
    fn update(&mut self);

    fn get_texture(&mut self, texture_id: Option<TextureId>) -> Option<TextureId>;

    fn play(&mut self, data: Vec<u8>, looping: bool) -> (u32, u32);
    fn restart(&mut self);
    fn pause(&mut self);
    fn resume(&mut self);

    fn stop(&mut self);
    fn state(&self) -> VideoSourceState;
}
enum PacketData {
    Packet(Packet, u32),
    Flush,
}

enum Decoder {
    Video(VideoDecoder),
    Audio(AudioDecoder),
}

pub struct StreamData {
    stream_index: usize,
    decoder: Decoder,
    time_base: f64,
    duration: i64,
    duration_pts: i64,
    time: Arc<RwLock<TimeData>>,
    packet_queue: VecDeque<PacketData>,
}

impl StreamData {
    fn new<D: FnOnce(FFmpegDecoder) -> Decoder>(
        input: &Input,
        stream: &Stream,
        decoder_fn: D,
        time: Arc<RwLock<TimeData>>,
    ) -> Self {
        let time_base = stream.time_base();
        // calculate duration in ms
        let input_duration = input.duration();
        let stream_duration = stream.duration();
        let input_duration_s = input.duration().map(|d| d as f64 * f64::from(time_base));
        let stream_duration_s = stream.duration().map(|d| d as f64 * f64::from(time_base));
        let duration_pts = stream_duration.unwrap_or(input_duration.unwrap());
        let duration_s = stream_duration_s.unwrap_or(input_duration_s.unwrap());
        let duration = (duration_s * 1000_f64) as i64;
        Self {
            stream_index: stream.index(),
            decoder: decoder_fn(stream.codec().decoder()),
            time_base: time_base.numerator() as f64 / time_base.denominator() as f64,
            duration_pts,
            duration,
            time,
            packet_queue: VecDeque::new(),
        }
    }
}

#[derive(Debug)]
struct TimeData {
    start_time: Instant,
    paused: Option<Instant>,
    ended: Option<Instant>,
    looping: bool,
    duration: i64,
    duration_pts: i64,
}

impl TimeData {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            start_time: now,
            paused: Some(now),
            ended: None,
            looping: false,
            duration: 0,
            duration_pts: 0,
        }
    }

    fn pause(&mut self) {
        if self.paused.is_none() {
            self.paused = Some(Instant::now());
        }
    }

    fn play(&mut self) {
        if let Some(paused) = self.paused.take() {
            self.start_time += Instant::now() - paused;
        }
    }

    fn reset(&mut self) {
        self.start_time = Instant::now();
    }

    fn end(&mut self) {
        if self.ended.is_none() {
            self.ended = Some(Instant::now());
        }
    }

    fn ended(&self) -> bool {
        !self.ended.is_none()
    }
}

impl Decoder {
    fn new_video(d: FFmpegDecoder) -> Self {
        Decoder::Video(d.video().unwrap())
    }

    fn as_video(&mut self) -> &mut VideoDecoder {
        if let Decoder::Video(d) = self {
            d
        } else {
            panic!("wrong type")
        }
    }

    fn new_audio(d: FFmpegDecoder) -> Self {
        Decoder::Audio(d.audio().unwrap())
    }

    fn as_audio(&mut self) -> &mut AudioDecoder {
        if let Decoder::Audio(d) = self {
            d
        } else {
            panic!("wrong type")
        }
    }
}

#[derive(Debug)]
pub enum LoopState {
    Running,
    Sleep(u64),
    Exit,
}
struct VideoState {
    input: Input,
    loop_count: u32,
    video: Arc<Mutex<VideoStreamData>>,
    audio: Arc<Mutex<AudioStreamData>>,
    time: Arc<RwLock<TimeData>>,
}

fn run_player_thread<F, T>(state: Weak<Mutex<T>>, description: String, f: F)
where
    F: Fn(&mut T) -> LoopState,
{
    debug!(
        "thread '{}' ({:?}) starting",
        description,
        thread::current().id()
    );
    // exit this loop as soon as the state itself has been lost
    while let Some(state) = state.upgrade() {
        // run this in a block to drop the mutex guard before sleeping
        let loop_state = {
            let mut state = state.lock().unwrap();
            f(&mut *state)
        };

        match loop_state {
            LoopState::Exit => break,
            LoopState::Sleep(millis) => thread::sleep(Duration::from_millis(millis)),
            LoopState::Running => (),
        }
    }
    debug!(
        "thread '{}' ({:?}) exiting",
        description,
        thread::current().id()
    );
}

fn enqueue_next_packet(state: &mut VideoState) -> LoopState {
    let video = state.video.lock().unwrap();
    let audio = state.audio.lock().unwrap();

    // sleep if the queues are full
    if video.stream.packet_queue.len() >= VIDEO_PACKET_QUEUE_MAX
        || audio.stream.packet_queue.len() >= AUDIO_PACKET_QUEUE_MAX
    {
        return LoopState::Sleep(QUEUE_FULL_SLEEP);
    }

    // unlock video and audio while getting next packet
    drop(video);
    drop(audio);

    // read input packets and queue them to the correct queue
    let packet = state.input.packets().next();
    let mut video = state.video.lock().unwrap();
    let mut audio = state.audio.lock().unwrap();
    match packet {
        Some(_packet) => match _packet {
            Ok((stream, packet)) => {
                let idx = stream.index();
                if idx == video.stream.stream_index {
                    video
                        .stream
                        .packet_queue
                        .push_back(PacketData::Packet(packet, state.loop_count));
                } else if idx == audio.stream.stream_index {
                    audio
                        .stream
                        .packet_queue
                        .push_back(PacketData::Packet(packet, state.loop_count));
                }
            }
            Err(error) => {}
        },
        None => {
            // Caution! It's not end of file.
            let time = state.time.read().unwrap();
            if !time.looping {
                return LoopState::Sleep(ENDED_SLEEP);
            }
            // looping -> seek to beginning?
            let _ = state.input.seek(0, 0..i64::max_value());
            video.stream.packet_queue.push_back(PacketData::Flush);
            audio.stream.packet_queue.push_back(PacketData::Flush);
            state.loop_count += 1;
        }
    }

    LoopState::Running
}
