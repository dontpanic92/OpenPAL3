mod ffmpeg;

pub fn register_opengb_video_decoders() {
    use radiance::video::{register_video_decoder, Codec};
    register_video_decoder(Codec::Bik, ffmpeg::VideoStreamFFmpeg::ctor);
}
