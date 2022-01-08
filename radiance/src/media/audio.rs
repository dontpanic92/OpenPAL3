extern crate ffmpeg;

use cpal::{Data as CpalStreamData, SampleFormat as CpalSampleFormat};
use ffmpeg::{
    software::resampling::Context as FFmpegResamplingContext,
    util::{
        channel_layout::ChannelLayout,
        format::sample::{Sample as FFmpegSampleFormat, Type as SampleType},
        frame::Audio as AudioFrame,
    },
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Data, Device, SampleFormat, Stream as CpalStream, StreamConfig,
};

use lazy_static::lazy_static;
use log::{debug, error};
use std::{
    collections::VecDeque,
    convert::{TryFrom, TryInto},
    sync::{Arc, Mutex},
};

use super::{LoopState, PacketData, StreamData};

lazy_static! {
    pub static ref AUDIO: Audio = Audio::new();
}

const OUTPUT_AUDIO_BUFFER_MAX: usize = 50_000;

const NO_PACKET_SLEEP: u64 = 20;
const QUEUE_FULL_SLEEP: u64 = 10;
const PAUSE_SLEEP: u64 = 50;

pub struct Audio {
    output_device: Device,
}

#[derive(Clone, Debug)]
pub struct FormatConfig {
    pub config: StreamConfig,
    pub format: SampleFormat,
}

pub struct OutputAudioStream {
    pub stream: CpalStream,
    pub config: FormatConfig,
}

unsafe impl Send for OutputAudioStream {}
unsafe impl Sync for OutputAudioStream {}

impl Audio {
    fn new() -> Self {
        let host = cpal::default_host();
        let output_device = host
            .default_output_device()
            .expect("Failed to open audio output device");

        Self { output_device }
    }

    pub fn create_output_stream<F>(&'static self, callback: F) -> Arc<OutputAudioStream>
    where
        F: Fn(&mut Data, &cpal::OutputCallbackInfo) -> () + Send + Sync + 'static,
    {
        let default_output_config = self
            .output_device
            .default_output_config()
            .expect("error querying default ouput config");

        let format = default_output_config.sample_format();
        let config = default_output_config.into();
        debug!("default playback config: {:?}", config);

        // Create the new output stream.
        let stream = self
            .output_device
            .build_output_stream_raw(&config, format, callback, move |err| {
                // react to errors here.
            })
            .unwrap();
        // Create our wrapper struct
        let audio_stream = Arc::new(OutputAudioStream {
            stream,
            config: FormatConfig { config, format },
        });

        audio_stream
    }
}

impl OutputAudioStream {
    pub fn play(&self) -> Result<(), cpal::PlayStreamError> {
        self.stream.play()
    }

    pub fn pause(&self) -> Result<(), cpal::PauseStreamError> {
        self.stream.pause()
    }
}

impl Drop for OutputAudioStream {
    fn drop(&mut self) {}
}

pub struct AudioStreamData {
    pub stream: StreamData,
    output_stream: Option<(Arc<OutputAudioStream>, OutputFormat)>,
    source_frames: VecDeque<AudioFrame>,
    resampled_frames: VecDeque<AudioFrame>,
    resampler: Option<ResamplingContext>,
    sample_buffer: Arc<Mutex<Option<SampleBuffer>>>,
}

impl AudioStreamData {
    pub fn new(stream: StreamData) -> Self {
        Self {
            stream,
            output_stream: None,
            source_frames: VecDeque::new(),
            resampled_frames: VecDeque::new(),
            resampler: None,
            sample_buffer: Arc::new(Mutex::new(None)),
        }
    }
}
struct ResamplingContext {
    context: FFmpegResamplingContext,
}

unsafe impl Send for ResamplingContext {}

struct OutputFormat {
    format: FFmpegSampleFormat,
    channel_layout: ChannelLayout,
    rate: u32,
}

enum SampleBuffer {
    I16 { buffer: VecDeque<i16> },
    F32 { buffer: VecDeque<f32> },
}

impl TryFrom<&FormatConfig> for OutputFormat {
    type Error = failure::Error;

    fn try_from(config: &FormatConfig) -> Result<Self, Self::Error> {
        let dst_format = match config.format {
            CpalSampleFormat::F32 => FFmpegSampleFormat::F32(SampleType::Packed),
            CpalSampleFormat::I16 => FFmpegSampleFormat::I16(SampleType::Packed),
            CpalSampleFormat::U16 => {
                return Err(failure::err_msg("Unsupported sample format U16!"));
            }
        };
        let channel_layout = match config.config.channels {
            1 => ChannelLayout::FRONT_CENTER,
            2 => ChannelLayout::FRONT_LEFT | ChannelLayout::FRONT_RIGHT,
            c => {
                return Err(failure::format_err!(
                    "Unsupported number of channels: {}!",
                    c
                ));
            }
        };

        Ok(Self {
            format: dst_format,
            channel_layout,
            rate: config.config.sample_rate.0,
        })
    }
}

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
            Err(err) => break,
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
    // Get the stream's output format.
    let (_stream, format) = audio.output_stream.as_ref().unwrap();
    // Get or create the correct resampler.
    let resampler = if let Some(resampler) = audio.resampler.as_mut() {
        resampler
    } else {
        audio.resampler.replace(ResamplingContext {
            context: FFmpegResamplingContext::get(
                source_frame.format(),
                source_frame.channel_layout(),
                source_frame.sample_rate(),
                format.format,
                format.channel_layout,
                format.rate,
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
        resampled_frame.set_channel_layout(format.channel_layout);
        resampled_frame.set_format(format.format);
        resampled_frame.set_rate(format.rate);
        delay = context.flush(&mut resampled_frame).unwrap();
        resampled_frames.push(resampled_frame);
    }

    resampled_frames
}

pub fn play_audio(audio: &mut AudioStreamData) -> LoopState {
    // First of all, check for pause and pause/play the audio stream.
    {
        let time = audio.stream.time.read().unwrap();
        if time.paused.is_some() {
            if let Some(stream) = audio.output_stream.as_ref() {
                let _ = stream.0.pause();
            }
            return LoopState::Sleep(PAUSE_SLEEP);
        } else if let Some(stream) = audio.output_stream.as_ref() {
            let _ = stream.0.play();
        }
    }

    // Create a new audio stream if we don't have one.
    if audio.output_stream.is_none() {
        // Clone the sample buffer Arc so we can pass it to the callback.
        let sample_buffer = Arc::clone(&audio.sample_buffer);
        let err_fn =
            |err: &cpal::OutputCallbackInfo| eprintln!("an error occurred on stream: {:?}", err);
        let output_stream = super::audio::AUDIO.create_output_stream(move |stream_data, err_fn| {
            buffer_callback(stream_data, &sample_buffer)
        });
        output_stream.play().unwrap();
        // Convert the stream format from cpal to ffmpeg.
        let format = match (&output_stream.config).try_into() {
            Ok(format) => format,
            Err(e) => {
                error!("{}", e);
                return LoopState::Exit;
            }
        };
        // Create the sample buffer.
        let buffer = match output_stream.config.format {
            CpalSampleFormat::I16 => SampleBuffer::I16 {
                buffer: VecDeque::new(),
            },
            CpalSampleFormat::F32 => SampleBuffer::F32 {
                buffer: VecDeque::new(),
            },
            CpalSampleFormat::U16 => unreachable!(),
        };
        // Store stream and buffer.
        audio.output_stream.replace((output_stream, format));
        audio.sample_buffer.lock().unwrap().replace(buffer);
    }

    // Try to get a cached frame first.
    let resampled_frame = if let Some(frame) = audio.resampled_frames.pop_front() {
        frame
    } else {
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
        // Resample the frame.
        let mut resampled_frames = resample_source_frame(audio, &source_frame).into();
        audio.resampled_frames.append(&mut resampled_frames);
        audio.resampled_frames.pop_front().unwrap()
    };

    // Get the sample buffer.
    let mut buffer = audio.sample_buffer.lock().unwrap();
    let buffer = buffer.as_mut().unwrap();
    // Check for the sample data type.
    match buffer {
        SampleBuffer::F32 { buffer } => {
            // Check that we don't store too many samples.
            if buffer.len() >= OUTPUT_AUDIO_BUFFER_MAX {
                audio.resampled_frames.push_front(resampled_frame);
                return LoopState::Sleep(QUEUE_FULL_SLEEP);
            }
            // Get frame data in the correct type.
            let frame_data = resampled_frame.data(0);
            let frame_data = unsafe {
                // FFmpeg internally allocates the data pointers, they're definitely aligned.
                #[allow(clippy::cast_ptr_alignment)]
                std::slice::from_raw_parts(
                    frame_data.as_ptr() as *const f32,
                    resampled_frame.samples() * resampled_frame.channels() as usize,
                )
            };
            // Store frame data in the sample buffer.
            buffer.extend(frame_data);
        }
        SampleBuffer::I16 { buffer } => {
            // Check that we don't store too many samples.
            if buffer.len() >= OUTPUT_AUDIO_BUFFER_MAX {
                audio.resampled_frames.push_front(resampled_frame);
                return LoopState::Sleep(QUEUE_FULL_SLEEP);
            }
            // Get frame data in the correct type.
            let frame_data = resampled_frame.data(0);
            let frame_data = unsafe {
                // FFmpeg internally allocates the data pointers, they're definitely aligned.
                #[allow(clippy::cast_ptr_alignment)]
                std::slice::from_raw_parts(
                    frame_data.as_ptr() as *const i16,
                    resampled_frame.samples() * resampled_frame.channels() as usize,
                )
            };
            // Store frame data in the sample buffer.
            buffer.extend(frame_data);
        }
    }

    LoopState::Running
}

fn buffer_callback(
    stream_data: &mut CpalStreamData,
    sample_buffer: &Arc<Mutex<Option<SampleBuffer>>>,
) {
    // Get the sample buffer.
    let mut sample_buffer = sample_buffer.lock().unwrap();
    if let Some(sample_buffer) = sample_buffer.as_mut() {
        // Check that data types match.
        match sample_buffer {
            SampleBuffer::F32 {
                buffer: sample_buffer,
            } => {
                // Copy samples from one buffer to the other.
                copy_buffers(stream_data.as_slice_mut().unwrap(), sample_buffer, 0.0);
            }
            SampleBuffer::I16 {
                buffer: sample_buffer,
            } => {
                // Copy samples from one buffer to the other.
                copy_buffers(stream_data.as_slice_mut().unwrap(), sample_buffer, 0);
            }
        }
    }
}

fn copy_buffers<T: Copy>(
    stream_buffer: &mut [T],
    sample_buffer: &mut VecDeque<T>,
    zero: T,
) -> usize {
    // Check that we don't access anything beyond buffer lengths.
    let len = stream_buffer.len().min(sample_buffer.len());
    let (front, back) = sample_buffer.as_slices();
    if front.len() >= len {
        // Just copy from the first slice, it's enough.
        (&mut stream_buffer[0..len]).copy_from_slice(&front[0..len]);
    } else {
        // Copy from both slices of the VecDeque.
        let front_len = front.len();
        (&mut stream_buffer[0..front_len]).copy_from_slice(&front[0..front_len]);
        (&mut stream_buffer[front_len..len]).copy_from_slice(&back[0..len - front_len]);
    }
    // Remove copied samples from our sample buffer.
    sample_buffer.rotate_left(len);
    sample_buffer.truncate(sample_buffer.len() - len);
    // Fill remaining stream buffer with silence.
    if len < stream_buffer.len() {
        // warn!("Not enough samples to fill stream buffer!");
        for s in stream_buffer[len..].iter_mut() {
            *s = zero;
        }
    }
    len
}
