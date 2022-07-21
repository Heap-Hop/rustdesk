use crate::{
    codec::{EncoderApi, EncoderCfg},
    hw,
};
use hbb_common::{
    anyhow::{anyhow, Context},
    bail,
    config::HwCodecConfig,
    lazy_static, log,
    message_proto::{EncodedVideoFrame, EncodedVideoFrames, Message, VideoFrame},
    ResultType,
};
use hwcodec::{
    decode::{DecodeContext, DecodeFrame, Decoder},
    encode::{EncodeContext, EncodeFrame, Encoder},
    ffmpeg::{CodecInfo, CodecInfos, DataFormat},
    AVPixelFormat,
    Quality::{self, *},
    RateContorl::{self, *},
    AV_NUM_DATA_POINTERS,
};
use std::sync::{Arc, Mutex};

lazy_static::lazy_static! {
    static ref HW_ENCODER_NAME: Arc<Mutex<Option<String>>> = Default::default();
}

const CFG_KEY_ENCODER: &str = "bestHwEncoders";
const CFG_KEY_DECODER: &str = "bestHwDecoders";

const DEFAULT_PIXFMT: AVPixelFormat = AVPixelFormat::AV_PIX_FMT_YUV420P;
const DEFAULT_TIME_BASE: [i32; 2] = [1, 30];
const DEFAULT_GOP: i32 = 60;
const DEFAULT_HW_QUALITY: Quality = Quality_Default;
const DEFAULT_RC: RateContorl = RC_DEFAULT;

pub struct HwEncoder {
    encoder: Encoder,
    // yuv: Vec<u8>,
    // pub format: DataFormat,
    // pub pixfmt: AVPixelFormat,
}

impl EncoderApi for HwEncoder {
    fn new(cfg: &EncoderCfg, yuv_cfg: &crate::YuvMeta) -> ResultType<Self>
    where
        Self: Sized,
    {
        let mut linesize = Vec::<i32>::new();
        linesize.resize(AV_NUM_DATA_POINTERS as _, 0);
        linesize[0] = yuv_cfg.stride[0] as _;
        linesize[1] = yuv_cfg.stride[1] as _;
        linesize[2] = yuv_cfg.stride[1] as _;
        let mut offset = Vec::<i32>::new();
        offset.resize(AV_NUM_DATA_POINTERS as _, 0);
        offset[0] = yuv_cfg.offset[0] as _;
        offset[1] = yuv_cfg.offset[1] as _;

        if !cfg.use_hwcodec {
            bail!("Failed to create encoder, cfg.use_hwcodec is false");
        }
        let ctx = EncodeContext {
            name: cfg.codec_name.clone(),
            width: cfg.width as _,
            height: cfg.height as _,
            pixfmt: DEFAULT_PIXFMT,
            linesize,
            offset,
            bitrate: (cfg.bitrate * 1000) as _,
            timebase: DEFAULT_TIME_BASE,
            gop: DEFAULT_GOP,
            quality: DEFAULT_HW_QUALITY,
            rc: DEFAULT_RC,
        };
        // TODO format_from_name
        let format = match Encoder::format_from_name(cfg.codec_name.clone()) {
            Ok(format) => format,
            Err(_) => {
                return Err(anyhow!(format!(
                    "failed to get format from name:{}",
                    cfg.codec_name
                )))
            }
        };

        match Encoder::new(ctx) {
            Ok(encoder) => Ok(HwEncoder { encoder }),
            Err(_) => Err(anyhow!(format!("Failed to create encoder"))),
        }
    }

    fn encode(&mut self, yuv: &[u8], _pts: i64) -> ResultType<Vec<EncodedVideoFrame>> {
        let mut encoder_frames = Vec::<EncodeFrame>::new();
        if let Ok(frames) = self.encoder.encode(yuv) {
            encoder_frames.append(frames);
        } else {
            bail!("Failed to encode");
        }

        let mut frames = Vec::new();
        for frame in encoder_frames {
            frames.push(EncodedVideoFrame {
                data: frame.data,
                pts: frame.pts as _,
                ..Default::default()
            });
        }
        Ok(frames)
    }

    fn set_bitrate(&mut self, bitrate: u32) -> ResultType<()> {
        self.encoder.set_bitrate((bitrate * 1000) as _).ok();
        Ok(())
    }
}

impl HwEncoder {
    /// Get best encoders.
    ///
    /// # Parameter  
    /// `force_reset`: force to refresh config.  
    /// `write`: write to config file.  
    ///
    /// # Return  
    /// `CodecInfos`: infos.  
    /// `bool`: whether the config is refreshed.  
    pub fn best(force_reset: bool, write: bool) -> (CodecInfos, bool) {
        let config = get_config(CFG_KEY_ENCODER);
        if !force_reset && config.is_ok() {
            (config.unwrap(), false)
        } else {
            let ctx = EncodeContext {
                name: String::from(""),
                width: 1920,
                height: 1080,
                pixfmt: DEFAULT_PIXFMT,
                linesize: vec![],
                offset: vec![], // TODO
                bitrate: 0,
                timebase: DEFAULT_TIME_BASE,
                gop: DEFAULT_GOP,
                quality: DEFAULT_HW_QUALITY,
                rc: DEFAULT_RC,
            };
            let encoders = CodecInfo::score(Encoder::avaliable_encoders(ctx));
            if write {
                set_config(CFG_KEY_ENCODER, &encoders)
                    .map_err(|e| log::error!("{:?}", e))
                    .ok();
            }
            (encoders, true)
        }
    }

    pub fn current_name() -> Arc<Mutex<Option<String>>> {
        HW_ENCODER_NAME.clone()
    }

    // pub fn encode(&mut self, yuv: &[u8]) -> ResultType<Vec<EncodeFrame>> {
    //     match self.pixfmt {
    //         AVPixelFormat::AV_PIX_FMT_YUV420P => hw::hw_bgra_to_i420(
    //             self.encoder.ctx.width as _,
    //             self.encoder.ctx.height as _,
    //             &self.encoder.linesize,
    //             &self.encoder.offset,
    //             self.encoder.length,
    //             bgra,
    //             &mut self.yuv,
    //         ),
    //         AVPixelFormat::AV_PIX_FMT_NV12 => hw::hw_bgra_to_nv12(
    //             self.encoder.ctx.width as _,
    //             self.encoder.ctx.height as _,
    //             &self.encoder.linesize,
    //             &self.encoder.offset,
    //             self.encoder.length,
    //             bgra,
    //             &mut self.yuv,
    //         ),
    //     }

    //     match self.encoder.encode(&self.yuv) {
    //         Ok(v) => {
    //             let mut data = Vec::<EncodeFrame>::new();
    //             data.append(v);
    //             Ok(data)
    //         }
    //         Err(_) => Ok(Vec::<EncodeFrame>::new()),
    //     }
    // }
}

pub struct HwDecoder {
    decoder: Decoder,
    pub info: CodecInfo,
}

pub struct HwDecoders {
    pub h264: Option<HwDecoder>,
    pub h265: Option<HwDecoder>,
}

impl HwDecoder {
    /// See HwEncoder::best
    fn best(force_reset: bool, write: bool) -> (CodecInfos, bool) {
        let config = get_config(CFG_KEY_DECODER);
        if !force_reset && config.is_ok() {
            (config.unwrap(), false)
        } else {
            let decoders = CodecInfo::score(Decoder::avaliable_decoders());
            if write {
                set_config(CFG_KEY_DECODER, &decoders)
                    .map_err(|e| log::error!("{:?}", e))
                    .ok();
            }
            (decoders, true)
        }
    }

    pub fn new_decoders() -> HwDecoders {
        let (best, _) = HwDecoder::best(false, true);
        let mut h264: Option<HwDecoder> = None;
        let mut h265: Option<HwDecoder> = None;
        let mut fail = false;

        if let Some(info) = best.h264 {
            h264 = HwDecoder::new(info).ok();
            if h264.is_none() {
                fail = true;
            }
        }
        if let Some(info) = best.h265 {
            h265 = HwDecoder::new(info).ok();
            if h265.is_none() {
                fail = true;
            }
        }
        if fail {
            HwDecoder::best(true, true);
        }
        HwDecoders { h264, h265 }
    }

    pub fn new(info: CodecInfo) -> ResultType<Self> {
        let ctx = DecodeContext {
            name: info.name.clone(),
            device_type: info.hwdevice.clone(),
        };
        match Decoder::new(ctx) {
            Ok(decoder) => Ok(HwDecoder { decoder, info }),
            Err(_) => Err(anyhow!(format!("Failed to create decoder"))),
        }
    }
    pub fn decode(&mut self, data: &[u8]) -> ResultType<Vec<HwDecoderImage>> {
        match self.decoder.decode(data) {
            Ok(v) => Ok(v.iter().map(|f| HwDecoderImage { frame: f }).collect()),
            Err(_) => Ok(vec![]),
        }
    }
}

pub struct HwDecoderImage<'a> {
    frame: &'a DecodeFrame,
}

impl HwDecoderImage<'_> {
    pub fn bgra(&self, bgra: &mut Vec<u8>, i420: &mut Vec<u8>) -> ResultType<()> {
        let frame = self.frame;
        match frame.pixfmt {
            AVPixelFormat::AV_PIX_FMT_NV12 => hw::hw_nv12_to_bgra(
                frame.width as _,
                frame.height as _,
                &frame.data[0],
                &frame.data[1],
                frame.linesize[0] as _,
                frame.linesize[1] as _,
                bgra,
                i420,
                0,
            ),
            AVPixelFormat::AV_PIX_FMT_YUV420P => {
                hw::hw_i420_to_bgra(
                    frame.width as _,
                    frame.height as _,
                    &frame.data[0],
                    &frame.data[1],
                    &frame.data[2],
                    frame.linesize[0] as _,
                    frame.linesize[1] as _,
                    frame.linesize[2] as _,
                    bgra,
                );
                return Ok(());
            }
        }
    }
}

fn get_config(k: &str) -> ResultType<CodecInfos> {
    let v = HwCodecConfig::load()
        .options
        .get(k)
        .unwrap_or(&"".to_owned())
        .to_owned();
    match CodecInfos::deserialize(&v) {
        Ok(v) => Ok(v),
        Err(_) => Err(anyhow!("Failed to get config:{}", k)),
    }
}

fn set_config(k: &str, v: &CodecInfos) -> ResultType<()> {
    match v.serialize() {
        Ok(v) => {
            let mut config = HwCodecConfig::load();
            config.options.insert(k.to_owned(), v);
            config.store();
            Ok(())
        }
        Err(_) => Err(anyhow!("Failed to set config:{}", k)),
    }
}

pub fn check_config() {
    let (encoders, update_encoders) = HwEncoder::best(false, false);
    let (decoders, update_decoders) = HwDecoder::best(false, false);
    if update_encoders || update_decoders {
        if let Ok(encoders) = encoders.serialize() {
            if let Ok(decoders) = decoders.serialize() {
                let mut config = HwCodecConfig::load();
                config.options.insert(CFG_KEY_ENCODER.to_owned(), encoders);
                config.options.insert(CFG_KEY_DECODER.to_owned(), decoders);
                config.store();
                return;
            }
        }
        log::error!("Failed to serialize codec info");
    }
}
