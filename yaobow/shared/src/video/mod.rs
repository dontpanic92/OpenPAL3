mod ffmpeg;

pub fn register_opengb_video_decoders() {
    use radiance::video::{Codec, register_video_decoder};
    register_video_decoder(Codec::Bik, ffmpeg::VideoStreamFFmpeg::create);
}
