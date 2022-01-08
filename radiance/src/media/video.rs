extern crate ffmpeg;

use ffmpeg::{
    media::Type,
    software::scaling::{self, Context as FFmpegScalingContext},
    util::{format::pixel::Pixel as PixelFormat, frame::Video as VideoFrame},
};

use std::{
    io,
    ops::Add,
    rc::Rc,
    sync::{
        mpsc::{channel, Sender},
        Arc, Mutex, RwLock, Weak,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::rendering::ComponentFactory;
use imgui::TextureId;
use lazy_static::lazy_static;
use log::{error, warn};

use super::{
    audio::{play_audio, AudioStreamData},
    enqueue_next_packet, run_player_thread, Decoder, LoopState, PacketData, StreamData, TimeData,
    VideoState,
};
use super::{VideoSource, VideoSourceState};

const NO_PACKET_SLEEP: u64 = 20;
const FRAME_TIMEOUT_MAX: u64 = 5;
const PAUSE_SLEEP: u64 = 50;

lazy_static! {
    static ref FRAME_SLEEP_EPSILON: Duration = Duration::from_millis(1);
}

pub struct InitResult {
    pub duration: i64,
    pub size: (u32, u32),
}

pub struct VideoStreamData {
    pub stream: StreamData,
    size_sender: Option<Sender<(u32, u32)>>,
    target_size: (u32, u32),
    source_frame: Option<(VideoFrame, u32)>,
    scaled_frame: Option<(VideoFrame, u32)>,
    scaler: Option<ScalingContext>,
}

struct ScalingContext {
    context: FFmpegScalingContext,
}

unsafe impl Send for ScalingContext {}

impl VideoStreamData {
    fn new(stream: StreamData) -> Self {
        Self {
            stream,
            size_sender: None,
            target_size: (800, 600),
            source_frame: None,
            scaled_frame: None,
            scaler: None,
        }
    }
}

pub struct FFMpegVideoSource {
    factory: Rc<dyn ComponentFactory>,
    state: VideoSourceState,
    looping: bool,
    video_state: Option<Arc<Mutex<VideoState>>>,
    threads: Vec<JoinHandle<()>>,
    time: Option<Arc<RwLock<TimeData>>>,
}

impl VideoSource for FFMpegVideoSource {
    fn update(&mut self) {}

    fn get_texture(&mut self, texture_id: Option<TextureId>) -> Option<TextureId> {
        if let Some(video_state) = self.video_state.as_ref() {
            let video_state = video_state.lock().unwrap();
            let video = video_state.video.lock().unwrap();
            if let Some(frame_data) = &video.scaled_frame {
                let frame = &frame_data.0;
                let (w, h) = (frame.width(), frame.height());
                let buffer_width = (frame.stride(0) as u64 / 4) as u32;
                let (_, texture_id) = self.factory.create_imgui_texture(
                    frame.data(0),
                    buffer_width,
                    w,
                    h,
                    texture_id,
                );

                return Some(texture_id);
            }
        }

        None
    }

    fn play(&mut self, data: Vec<u8>, looping: bool) -> (u32, u32) {
        self.state = VideoSourceState::Playing;
        self.looping = looping;

        let result = self.init_with_data(data);
        self.set_looping(looping);

        if let Some(time) = self.time.as_ref() {
            let mut time = time.write().unwrap();
            time.play();
        }

        result.size
    }

    fn restart(&mut self) {
        if let Some(time) = self.time.as_ref() {
            let mut time = time.write().unwrap();
            time.reset();
        }
    }

    fn stop(&mut self) {
        self.state = VideoSourceState::Stopped;
        if let Some(time) = self.time.as_ref() {
            let mut time = time.write().unwrap();
            time.pause();
        }
        self._stop_threads();
    }

    fn state(&self) -> VideoSourceState {
        if let Some(time) = self.time.as_ref() {
            let time = time.read().unwrap();
            if time.ended() {
                return VideoSourceState::Stopped;
            }
        }
        self.state
    }

    fn pause(&mut self) {
        self.state = VideoSourceState::Paused;
        if let Some(time) = self.time.as_ref() {
            let mut time = time.write().unwrap();
            time.pause();
        }
    }

    fn resume(&mut self) {
        if self.state == VideoSourceState::Paused {
            self.state = VideoSourceState::Playing;
            if let Some(time) = self.time.as_ref() {
                let mut time = time.write().unwrap();
                time.play();
            }
        }
    }
}

impl FFMpegVideoSource {
    pub fn new(factory: Rc<dyn ComponentFactory>) -> Self {
        Self {
            factory,
            state: VideoSourceState::Stopped,
            looping: false,
            video_state: None,
            threads: Vec::new(),
            time: None,
        }
    }

    pub fn init_with_data(&mut self, data: Vec<u8>) -> InitResult {
        use std::io::Cursor;
        self._init(Cursor::new(data))
    }

    fn _init(&mut self, io: impl io::Read + io::Seek + 'static) -> InitResult {
        let time = Arc::new(RwLock::new(TimeData::new()));
        let input = ffmpeg::format::io::input(io).unwrap();
        let video = Arc::new(Mutex::new(VideoStreamData::new(StreamData::new(
            &input,
            &input.streams().best(Type::Video).unwrap(),
            Decoder::new_video,
            Arc::clone(&time),
        ))));
        let rx = {
            let (tx, rx) = channel();
            let mut video = video.lock().unwrap();
            video.size_sender = Some(tx);
            rx
        };

        let weak_video = Arc::downgrade(&video);
        let duration = video.lock().unwrap().stream.duration;
        let duration_pts = video.lock().unwrap().stream.duration_pts;
        // Now create the audio stream data.
        let audio = Arc::new(Mutex::new(AudioStreamData::new(StreamData::new(
            &input,
            &input.streams().best(Type::Audio).unwrap(),
            Decoder::new_audio,
            Arc::clone(&time),
        ))));
        let weak_audio = Arc::downgrade(&audio);
        // Create the state.
        let state = Arc::new(Mutex::new(VideoState {
            loop_count: 0,
            input,
            video,
            audio,
            time: Arc::clone(&time),
        }));
        let weak_state = Arc::downgrade(&state);

        // Store the duration and then move the TimeData into self.
        {
            let mut time = time.write().unwrap();
            time.duration = duration;
            time.duration_pts = duration_pts;
        }
        self.time.replace(time);

        self.threads.push(thread::spawn(|| {
            run_player_thread(weak_state, "queue".into(), enqueue_next_packet)
        }));
        let weak_video_2 = Weak::clone(&weak_video);
        self.threads.push(thread::spawn(move || {
            run_player_thread(weak_video, "video player".into(), play_video)
        }));
        self.threads.push(thread::spawn(|| {
            run_player_thread(weak_audio, "audio player".into(), play_audio)
        }));

        // Wait until the first frame has been decoded and we know the video size.
        let size = rx.recv().unwrap();

        self.video_state.replace(state);

        InitResult { duration, size }
    }

    pub fn set_looping(&self, looping: bool) {
        if let Some(time) = self.time.as_ref() {
            let mut time = time.write().unwrap();
            time.looping = looping;
        }
    }

    pub fn _get_position(&self) -> i64 {
        if let Some(time) = self.time.as_ref() {
            let time = time.read().unwrap();
            // Respect the pause state if necessary.
            let now = if let Some(paused) = time.paused {
                paused
            } else {
                Instant::now()
            };
            if now <= time.start_time {
                0
            } else {
                // Get only the position in the current loop.
                now.duration_since(time.start_time).as_millis() as i64 % time.duration
            }
        } else {
            0
        }
    }

    fn _stop_threads(&mut self) {
        // Drop the Arc<VideoState> to signal threads to exit.
        self.video_state.take();
        // Wait for each thread to exit and print errors.
        while let Some(t) = self.threads.pop() {
            if let Err(err) = t.join() {
                warn!("thread exited with error: {:?}", err);
            }
        }
    }
}

impl Drop for FFMpegVideoSource {
    fn drop(&mut self) {
        self._stop_threads();
    }
}

fn get_source_frame(video: &mut VideoStreamData) -> Result<(VideoFrame, u32), LoopState> {
    let (packet, loop_count) = if let Some(packet) = video.stream.packet_queue.pop_front() {
        match packet {
            PacketData::Packet(p, l) => (p, l),
            PacketData::Flush => {
                video.stream.decoder.as_video().flush();
                return get_source_frame(video);
            }
        }
    } else {
        return Err(LoopState::Sleep(NO_PACKET_SLEEP));
    };
    let decoder = video.stream.decoder.as_video();
    let mut frame = VideoFrame::empty();
    match decoder.decode(&packet, &mut frame) {
        Err(err) => {
            error!("failed to decode video frame: {}", err);
            Err(LoopState::Exit)
        }
        Ok(_) => {
            if frame.format() == PixelFormat::None {
                get_source_frame(video)
            } else {
                Ok((frame, loop_count))
            }
        }
    }
}

fn scale_source_frame(
    video: &mut VideoStreamData,
    source_frame: &VideoFrame,
) -> Result<VideoFrame, LoopState> {
    let size = video.target_size;
    if let Some(scaler) = video.scaler.as_ref() {
        if scaler.context.input().width != source_frame.width()
            || scaler.context.input().height != source_frame.height()
            || scaler.context.input().format != source_frame.format()
            || scaler.context.output().width != size.0
            || scaler.context.output().height != size.1
        {
            video.scaler.take();
        }
    }
    let scaler = if let Some(scaler) = video.scaler.as_mut() {
        scaler
    } else {
        video.scaler.replace(ScalingContext {
            context: FFmpegScalingContext::get(
                source_frame.format(),
                source_frame.width(),
                source_frame.height(),
                PixelFormat::RGBA,
                size.0,
                size.1,
                scaling::flag::Flags::BILINEAR,
            )
            .unwrap(),
        });
        video.scaler.as_mut().unwrap()
    };
    let mut scaled_frame = VideoFrame::empty();
    scaler.context.run(source_frame, &mut scaled_frame).unwrap();
    scaled_frame.set_pts(source_frame.pts());
    Ok(scaled_frame)
}

fn play_video(video: &mut VideoStreamData) -> LoopState {
    let (rgb_frame, loop_count) = if let Some(frame) = video.scaled_frame.take() {
        frame
    } else {
        // no texture frame available, get one from a source frame
        let (source_frame, loop_count) = if let Some(frame) = video.source_frame.take() {
            frame
        } else {
            // no source frame available either, decode a new one
            match get_source_frame(video) {
                Ok(frame) => frame,
                Err(state) => return state,
            }
        };
        // store size for external access
        if let Some(tx) = video.size_sender.take() {
            tx.send((source_frame.width(), source_frame.height()))
                .unwrap();
        }

        // always use the original frame size and only let sws_scale convert pixel format
        // it's recommended to do actual scaling with hardware acceleration
        video.target_size = (source_frame.width(), source_frame.height());

        // scale frame to texture size and pixel format
        match scale_source_frame(video, &source_frame) {
            Ok(frame) => (frame, loop_count),
            Err(state) => return state,
        }
    };

    let mut video_time = video.stream.time.write().unwrap();
    let start_time = {
        if video_time.paused.is_some() {
            video.scaled_frame.replace((rgb_frame, loop_count));
            return LoopState::Sleep(PAUSE_SLEEP);
        }
        video_time.start_time
    };

    // calculate video end
    if video_time.duration_pts <= rgb_frame.pts().unwrap() + 1 && !video_time.looping {
        video_time.end();
    }

    // calculate correct display time for frame
    let display_time = rgb_frame.pts().unwrap() as f64 * video.stream.time_base;
    let display_time =
        (display_time * 1000_f64) as u64 + (video.stream.duration as u64 * loop_count as u64);
    let display_time = start_time.add(Duration::from_millis(display_time));
    let now = Instant::now();
    if display_time > now {
        let diff = display_time.duration_since(now);
        if diff > *FRAME_SLEEP_EPSILON {
            video.scaled_frame.replace((rgb_frame, loop_count));
            return LoopState::Sleep((diff.as_millis() as u64).max(FRAME_TIMEOUT_MAX));
        }
    }

    LoopState::Running
}
