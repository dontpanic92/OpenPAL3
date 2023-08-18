use ffmpeg::{
    codec::{
        decoder::{Audio as AudioDecoder, Decoder as FFmpegDecoder, Video as VideoDecoder},
        packet::Packet,
    },
    format::{context::Input, stream::Stream},
};
use ffmpeg::{
    media::Type,
    software::scaling::{self, Context as FFmpegScalingContext},
    util::{format::pixel::Pixel as PixelFormat, frame::Video as VideoFrame},
};
use ffmpeg::{
    software::resampling::Context as FFmpegResamplingContext,
    util::{
        format::sample::{Sample as FFmpegSampleFormat, Type as SampleType},
        frame::Audio as AudioFrame,
    },
};

use std::{
    collections::VecDeque,
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

use imgui::TextureId;
use lazy_static::lazy_static;
use log::{debug, error, warn};
use radiance::{
    audio::AudioEngine,
    rendering::{ComponentFactory, Texture},
    utils::SeekRead,
};

use radiance::video::{VideoStream, VideoStreamState};

const OUTPUT_AUDIO_BUFFER_MAX: usize = 20;

const VIDEO_PACKET_QUEUE_MAX: usize = 1024;
const AUDIO_PACKET_QUEUE_MAX: usize = 512;

const QUEUE_FULL_SLEEP: u64 = 10;
const FRAME_TIMEOUT_MAX: u64 = 5;
const NO_PACKET_SLEEP: u64 = 20;
const PAUSE_SLEEP: u64 = 50;
const ENDED_SLEEP: u64 = 50;

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

struct ScalingContext {
    context: FFmpegScalingContext,
}

unsafe impl Send for ScalingContext {}

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
            decoder: decoder_fn(stream.decoder().unwrap()),
            time_base: time_base.numerator() as f64 / time_base.denominator() as f64,
            duration_pts,
            duration,
            time,
            packet_queue: VecDeque::new(),
        }
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

    fn end(&mut self) {
        if self.ended.is_none() {
            self.ended = Some(Instant::now());
        }
    }

    fn ended(&self) -> bool {
        !self.ended.is_none()
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

pub struct VideoStreamFFmpeg {
    reader: Option<Box<dyn SeekRead>>,
    factory: Rc<dyn ComponentFactory>,
    audio_engine: Rc<dyn AudioEngine>,
    state: VideoStreamState,
    looping: bool,
    video_state: Option<Arc<Mutex<VideoState>>>,
    threads: Vec<JoinHandle<()>>,
    time: Option<Arc<RwLock<TimeData>>>,
    current_texture: Option<Box<dyn Texture>>,
}

impl VideoStream for VideoStreamFFmpeg {
    fn set_reader(&mut self, reader: Box<dyn SeekRead>) {
        self.reader = Some(reader);
    }

    fn play(&mut self, looping: bool) -> (u32, u32) {
        self.state = VideoStreamState::Playing;
        self.looping = looping;
        self.set_looping(looping);

        let reader = self.reader.take().unwrap();
        let result = self.init(reader);

        if let Some(time) = self.time.as_ref() {
            let mut time = time.write().unwrap();
            time.play();
        }

        result.size
    }

    fn stop(&mut self) {
        self.state = VideoStreamState::Stopped;
        if let Some(time) = self.time.as_ref() {
            let mut time = time.write().unwrap();
            time.pause();
        }
        self._stop_threads();
    }

    fn pause(&mut self) {
        self.state = VideoStreamState::Paused;
        if let Some(time) = self.time.as_ref() {
            let mut time = time.write().unwrap();
            time.pause();
        }
    }

    fn resume(&mut self) {
        if self.state == VideoStreamState::Paused {
            self.state = VideoStreamState::Playing;
            if let Some(time) = self.time.as_ref() {
                let mut time = time.write().unwrap();
                time.play();
            }
        }
    }

    fn get_texture(&mut self, texture_id: Option<TextureId>) -> Option<TextureId> {
        if let Some(video_state) = self.video_state.as_ref() {
            let video_state = video_state.lock().unwrap();
            let video = video_state.video.lock().unwrap();
            if let Some(frame_data) = &video.scaled_frame {
                let frame = &frame_data.0;
                let (w, h) = (frame.width(), frame.height());
                let buffer_width = (frame.stride(0) as u64 / 4) as u32;
                let (texture, texture_id) = self.factory.create_imgui_texture(
                    frame.data(0),
                    buffer_width,
                    w,
                    h,
                    texture_id,
                );

                // should keep current texture valid until
                self.current_texture = Some(texture);

                return Some(texture_id);
            }
        }

        None
    }

    fn get_state(&self) -> VideoStreamState {
        if let Some(time) = self.time.as_ref() {
            let time = time.read().unwrap();
            if time.ended() {
                return VideoStreamState::Stopped;
            }
        }
        self.state
    }
}

impl VideoStreamFFmpeg {
    pub fn new(factory: Rc<dyn ComponentFactory>, audio_engine: Rc<dyn AudioEngine>) -> Self {
        Self {
            reader: None,
            factory,
            audio_engine,
            state: VideoStreamState::Stopped,
            looping: false,
            video_state: None,
            threads: Vec::new(),
            time: None,
            current_texture: None,
        }
    }

    pub fn create(
        factory: Rc<dyn ComponentFactory>,
        audio_engine: Rc<dyn AudioEngine>,
    ) -> Box<dyn VideoStream> {
        Box::new(Self::new(factory, audio_engine))
    }

    pub fn init(&mut self, io: impl io::Read + io::Seek + 'static) -> InitResult {
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
        let resampled_frames = Arc::new(Mutex::new(VecDeque::new()));
        let mut audio_source = self.audio_engine.create_custom_decoder_source();
        audio_source.set_decoder(Box::new(AudioFFmpegDecoder::new(resampled_frames.clone())));
        let audio_output_stream = Arc::new(OutputAudioStream {
            stream_source: Mutex::new(audio_source),
            resampled_frames,
        });

        let audio = Arc::new(Mutex::new(AudioStreamData::new(
            StreamData::new(
                &input,
                &input.streams().best(Type::Audio).unwrap(),
                Decoder::new_audio,
                Arc::clone(&time),
            ),
            audio_output_stream,
        )));
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

impl Drop for VideoStreamFFmpeg {
    fn drop(&mut self) {
        self._stop_threads();
    }
}

struct AudioFFmpegDecoder {
    resampled_frames: Arc<Mutex<VecDeque<AudioFrame>>>,
}

impl AudioFFmpegDecoder {
    pub fn new(resampled_frames: Arc<Mutex<VecDeque<AudioFrame>>>) -> Self {
        Self { resampled_frames }
    }
}

impl radiance::audio::Decoder for AudioFFmpegDecoder {
    fn fetch_samples(&mut self) -> anyhow::Result<Option<radiance::audio::Samples>> {
        let mut frames = self.resampled_frames.lock().unwrap();

        let frame = frames.pop_front();
        let samples = match frame {
            Some(f) => {
                // Get frame data in the correct type.
                let channels = f.channels() as usize;
                let sample_rate = f.sample_rate() as i32;

                let frame_data = unsafe {
                    // FFmpeg internally allocates the data pointers, they're definitely aligned.
                    #[allow(clippy::cast_ptr_alignment)]
                    std::slice::from_raw_parts(
                        f.data(0).as_ptr() as *const i16,
                        f.samples() * f.channels() as usize,
                    )
                };

                radiance::audio::Samples {
                    data: frame_data.to_vec(),
                    channels,
                    sample_rate,
                }
            }
            None => radiance::audio::Samples {
                data: vec![0; 100],
                channels: 2,
                sample_rate: 44100,
            },
        };

        Ok(Some(samples))
    }

    fn reset(&mut self) {}
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
    decoder.send_packet(&packet).unwrap();
    match decoder.receive_frame(&mut frame) {
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

struct OutputAudioStream {
    stream_source: Mutex<Box<dyn radiance::audio::AudioCustomDecoderSource>>,
    resampled_frames: Arc<Mutex<VecDeque<AudioFrame>>>,
}

struct AudioStreamData {
    pub stream: StreamData,
    output_stream: Arc<OutputAudioStream>,
    source_frames: VecDeque<AudioFrame>,
    resampler: Option<ResamplingContext>,
    target_format: FFmpegSampleFormat,
    target_channel_layout: ffmpeg::ChannelLayout,
    target_sample_rate: u32,
}

impl AudioStreamData {
    pub fn new(stream: StreamData, output_stream: Arc<OutputAudioStream>) -> Self {
        Self {
            stream,
            output_stream,
            source_frames: VecDeque::new(),
            resampler: None,
            target_format: FFmpegSampleFormat::I16(SampleType::Packed),
            target_channel_layout: ffmpeg::ChannelLayout::STEREO,
            target_sample_rate: 44100,
        }
    }
}

struct ResamplingContext {
    context: FFmpegResamplingContext,
}

unsafe impl Send for ResamplingContext {}

fn get_audio_source_frames(audio: &mut AudioStreamData) -> Result<Vec<AudioFrame>, LoopState> {
    // Get a packet from the packet queue.
    let (packet, _loop_count) = if let Some(packet) = audio.stream.packet_queue.pop_front() {
        // Check what we found in the packet queue.
        match packet {
            PacketData::Packet(p, l) => (p, l),
            PacketData::Flush => {
                // Flush the decoder and return the next source frame.
                audio.stream.decoder.as_audio().flush();
                return get_audio_source_frames(audio);
            }
        }
    } else {
        return Err(LoopState::Sleep(NO_PACKET_SLEEP));
    };
    // Decode this packet into one or more frames.
    let decoder = audio.stream.decoder.as_audio();
    match decoder.send_packet(&packet) {
        Err(err) => {
            error!("failed to send audio packet: {}", err);
            return Err(LoopState::Exit);
        }
        Ok(()) => {}
    }

    let mut frames = Vec::new();
    loop {
        let mut frame = AudioFrame::empty();
        match decoder.receive_frame(&mut frame) {
            Err(_) => break,
            Ok(()) => {
                if frame.format() != FFmpegSampleFormat::None {
                    frames.push(frame);
                }
            }
        };
    }

    if frames.is_empty() {
        get_audio_source_frames(audio)
    } else {
        Ok(frames)
    }
}

fn resample_source_frame(
    audio: &mut AudioStreamData,
    source_frame: &AudioFrame,
) -> Vec<AudioFrame> {
    // Get or create the correct resampler.
    let resampler = if let Some(resampler) = audio.resampler.as_mut() {
        resampler
    } else {
        audio.resampler.replace(ResamplingContext {
            context: FFmpegResamplingContext::get(
                source_frame.format(),
                source_frame.channel_layout(),
                source_frame.sample_rate(),
                audio.target_format,
                audio.target_channel_layout,
                audio.target_sample_rate,
            )
            .unwrap(),
        });
        audio.resampler.as_mut().unwrap()
    };

    // Start resampling.
    let context = &mut resampler.context;
    let mut resampled_frames = Vec::new();

    let mut resampled_frame = AudioFrame::empty();
    let mut delay = context.run(source_frame, &mut resampled_frame).unwrap();
    resampled_frames.push(resampled_frame);
    while let Some(_) = delay {
        let mut resampled_frame = AudioFrame::empty();
        resampled_frame.set_channel_layout(audio.target_channel_layout);
        resampled_frame.set_format(audio.target_format);
        resampled_frame.set_rate(audio.target_sample_rate);
        delay = context.flush(&mut resampled_frame).unwrap();
        resampled_frames.push(resampled_frame);
    }

    resampled_frames
}

fn play_audio(audio: &mut AudioStreamData) -> LoopState {
    // First of all, check for pause and pause/play the audio stream.
    {
        let time = audio.stream.time.read().unwrap();
        let mut stream_source = audio.output_stream.stream_source.lock().unwrap();
        stream_source.update();
        if time.paused.is_some()
            && stream_source.state() == radiance::audio::AudioSourceState::Playing
        {
            stream_source.pause();
            return LoopState::Sleep(PAUSE_SLEEP);
        } else if stream_source.state() != radiance::audio::AudioSourceState::Playing {
            stream_source.play(false);
        }
    }

    // No resampled frame available, calculate a new one.
    let source_frame = if let Some(frame) = audio.source_frames.pop_front() {
        frame
    } else {
        // No source frame available, so decode a new one.
        let frames = match get_audio_source_frames(audio) {
            Ok(frames) => frames,
            Err(state) => return state,
        };
        // Store the frames.
        audio.source_frames.extend(frames);
        audio.source_frames.pop_front().unwrap()
    };

    let resampled_frames_len = audio.output_stream.resampled_frames.lock().unwrap().len();
    if resampled_frames_len >= OUTPUT_AUDIO_BUFFER_MAX {
        audio.source_frames.push_front(source_frame);
        return LoopState::Sleep(QUEUE_FULL_SLEEP);
    }

    // Resample the frame.
    let mut frames = resample_source_frame(audio, &source_frame).into();
    audio
        .output_stream
        .resampled_frames
        .lock()
        .unwrap()
        .append(&mut frames);

    LoopState::Running
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
            Err(error) => {
                error!("error reading packet: {}", error);
            }
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
