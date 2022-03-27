use super::codec::{Error, Result};
use hardware_codec::{
    ffmpeg::{AVHWDeviceType, AVPixelFormat},
    video_decoder::{VideoDecoder, VideoDecoderContext},
    video_encoder::{VideoEncoder, VideoEncoderContext},
};

pub struct HEnc {
    encoder: VideoEncoder,
}

impl HEnc {
    pub fn new(codec_name: String, width: usize, height: usize, fps: i64) -> Result<Self> {
        let ctx = VideoEncoderContext {
            codec_name,
            fps: fps as _,
            src_width: width as _,
            src_height: height as _,
            dst_width: width as _,
            dst_height: height as _,
            pix_fmt: AVPixelFormat::AV_PIX_FMT_YUV420P,
        };
        match VideoEncoder::new(&ctx) {
            Ok(encoder) => Ok(HEnc { encoder }),
            Err(e) => Err(Error::FailedCall(format!("new encoder:{}", e).to_owned())),
        }
    }
    pub fn encode(&mut self, data: Vec<u8>) -> Result<Vec<Vec<u8>>> {
        match self.encoder.encode(data) {
            Ok(v) => {
                let mut data = Vec::<Vec<u8>>::new();
                data.append(v);
                Ok(data)
            }
            Err(ret) => Err(Error::FailedCall(format!("encode ret:{}", ret).to_owned())),
        }
    }
}

pub struct HDec {
    decoder: VideoDecoder,
}

impl HDec {
    pub fn new(codec_name: String) -> Result<Self> {
        let ctx = VideoDecoderContext {
            codec_name,
            device_type: AVHWDeviceType::AV_HWDEVICE_TYPE_NONE,
        };
        match VideoDecoder::new(&ctx) {
            Ok(decoder) => Ok(HDec { decoder }),
            Err(e) => Err(Error::FailedCall(format!("new decoder:{}", e).to_owned())),
        }
    }
    pub fn decode(&mut self, data: &[u8]) -> Result<Vec<Vec<u8>>> {
        match self.decoder.decode(data) {
            Ok(v) => {
                let mut data = Vec::<Vec<u8>>::new();
                data.append(v);
                Ok(data)
            }
            Err(ret) => Err(Error::FailedCall(format!("decode ret:{}", ret).to_owned())),
        }
    }
}
