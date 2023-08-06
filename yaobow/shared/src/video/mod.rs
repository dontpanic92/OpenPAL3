#[cfg(target_os = "windows")]
mod ffmpeg;

pub fn register_opengb_video_decoders() {
    #[cfg(target_os = "windows")]
    {
        use radiance::video::{register_video_decoder, Codec};
        register_video_decoder(Codec::Bik, ffmpeg::VideoStreamFFmpeg::ctor);
    }
}
