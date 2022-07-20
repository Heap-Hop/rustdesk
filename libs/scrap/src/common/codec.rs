use std::ops::{Deref, DerefMut};
#[cfg(feature = "hwcodec")]
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[cfg(feature = "hwcodec")]
use crate::hwcodec::*;
use crate::{bgra_to_i420, get_yuv_stride, rgba_to_i420, vpxcodec::*, STRIDE_ALIGN};

use hbb_common::{
    anyhow::anyhow,
    bail, log,
    message_proto::{
        video_frame, EncodedVideoFrame, EncodedVideoFrames, Message, VideoCodecState, VideoFrame,
    },
    ResultType,
};
#[cfg(feature = "hwcodec")]
use hbb_common::{config::Config2, lazy_static};

#[cfg(feature = "hwcodec")]
lazy_static::lazy_static! {
    static ref PEER_DECODER_STATES: Arc<Mutex<HashMap<i32, VideoCodecState>>> = Default::default();
    static ref MY_DECODER_STATE: Arc<Mutex<VideoCodecState>> = Default::default();
}
const SCORE_VPX: i32 = 90;

pub enum RawFrame<'a> {
    RGBA(&'a [u8]),
    BGRA(&'a [u8]),
    YUV(&'a [u8]), // TODO frame + meta
}

impl<'a> RawFrame<'a> {
    fn convert_into_yuv(
        &self,
        width: usize,
        height: usize,
        yuv_buf: &mut Vec<u8>,
        yuv_meta: &YuvMeta,
    ) {
        match (self, &yuv_meta.format) {
            (RawFrame::RGBA(rgba), YuvFormat::I420) => {
                rgba_to_i420(width, height, rgba, yuv_buf, yuv_meta)
            }
            (RawFrame::RGBA(rgba), YuvFormat::NV12) => {
                todo!()
            }
            (RawFrame::BGRA(bgra), YuvFormat::I420) => {
                bgra_to_i420(width, height, bgra, yuv_buf, yuv_meta)
            }
            (RawFrame::BGRA(bgra), YuvFormat::NV12) => {
                todo!()
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CodecFormat {
    VP8,
    #[default]
    VP9,
    H264,
    H265,
}

#[derive(Debug, Clone, Default)]
pub enum YuvFormat {
    #[default]
    I420,
    NV12,
}

#[derive(Debug, Clone, Default)]
pub struct EncoderCfg {
    pub codec_name: String,
    pub codec_format: CodecFormat,
    pub use_hwcodec: bool,
    /// The width (in pixels).
    pub width: usize,
    /// The height (in pixels).
    pub height: usize,
    /// The timebase numerator and denominator (in seconds).
    pub timebase: [i32; 2],
    /// The target bitrate (in kilobits per second).
    pub bitrate: u32,
    pub num_threads: u32,
}

pub trait EncoderApi {
    fn new(cfg: &EncoderCfg) -> ResultType<Self>
    where
        Self: Sized;

    fn encode(
        &mut self,
        yuv: &[u8],
        yuv_cfg: &YuvMeta,
        pts: i64,
    ) -> ResultType<Vec<EncodedVideoFrame>>;

    fn set_bitrate(&mut self, bitrate: u32) -> ResultType<()>;
}

pub struct DecoderCfg {
    pub vpx: VpxDecoderConfig,
}

#[derive(Debug)]
pub struct YuvMeta {
    pub stride: [usize; 2],
    pub offset: [usize; 2],
    pub length: usize,
    pub format: YuvFormat,
}

impl YuvMeta {
    fn new(format: YuvFormat, width: usize, height: usize) -> Self {
        let res = get_yuv_stride(&format, width, height, STRIDE_ALIGN);
        YuvMeta {
            stride: res.0,
            offset: res.1,
            length: res.2,
            format,
        }
    }
}

pub struct Encoder {
    encoder_cfg: EncoderCfg,
    yuv_buf: Vec<u8>,
    yuv_meta: YuvMeta,
    pub codec: Box<dyn EncoderApi>,
}

impl Deref for Encoder {
    type Target = Box<dyn EncoderApi>;

    fn deref(&self) -> &Self::Target {
        &self.codec
    }
}

impl DerefMut for Encoder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.codec
    }
}

pub struct Decoder {
    vpx: VpxDecoder,
    #[cfg(feature = "hwcodec")]
    hw: HwDecoders,
    #[cfg(feature = "hwcodec")]
    i420: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum EncoderUpdate {
    State(VideoCodecState),
    Remove,
    DisableHwIfNotExist,
}

impl Encoder {
    pub fn new(config: EncoderCfg) -> ResultType<Encoder> {
        log::info!("new encoder:{:?}", config);
        let yuv_meta = YuvMeta::new(YuvFormat::default(), config.width as _, config.height as _);
        let codec: Box<dyn EncoderApi> = match (&config.use_hwcodec, &config.codec_format) {
            (false, CodecFormat::VP9) => Box::new(VpxEncoder::new(&config)?),
            #[cfg(feature = "hwcodec")]
            (true, CodecFormat::H264 | CodecFormat::H265) => {
                if let Ok(codec) = HwEncoder::new(&config) {
                    Box::new(codec)
                } else {
                    HwEncoder::best(true, true);
                    bail!("unsupported encoder type");
                }
            }
            _ => bail!("unsupported encoder type"),
        };
        let yuv_buf = Vec::new();
        // yuv_buf.resize(yuv_cfg.length, 0);
        Ok(Encoder {
            encoder_cfg: config,
            yuv_buf,
            yuv_meta,
            codec,
        })
    }

    pub fn encode_to_message(&mut self, frame: RawFrame, pts: i64) -> ResultType<Message> {
        let frames = match frame {
            RawFrame::YUV(yuv) => self.codec.encode(yuv, &self.yuv_meta, pts)?, // TODO meta from quartz
            _ => {
                frame.convert_into_yuv(
                    self.encoder_cfg.width,
                    self.encoder_cfg.height,
                    &mut self.yuv_buf,
                    &self.yuv_meta,
                );
                self.codec.encode(&self.yuv_buf, &self.yuv_meta, pts)?
            }
        };

        let mut msg_out = Message::new();
        let mut vf = VideoFrame::new();
        match self.encoder_cfg.codec_format {
            CodecFormat::VP9 => vf.set_vp9s(EncodedVideoFrames {
                frames,
                ..Default::default()
            }),
            CodecFormat::H264 => vf.set_h264s(EncodedVideoFrames {
                frames,
                ..Default::default()
            }),
            CodecFormat::H265 => vf.set_h265s(EncodedVideoFrames {
                frames,
                ..Default::default()
            }),
            _ => bail!(""),
        }
        msg_out.set_video_frame(vf);
        Ok(msg_out)
    }

    // TODO
    pub fn update_video_encoder(id: i32, update: EncoderUpdate) {
        log::info!("encoder update: {:?}", update);
        #[cfg(feature = "hwcodec")]
        {
            let mut states = PEER_DECODER_STATES.lock().unwrap();
            match update {
                EncoderUpdate::State(state) => {
                    states.insert(id, state);
                }
                EncoderUpdate::Remove => {
                    states.remove(&id);
                }
                EncoderUpdate::DisableHwIfNotExist => {
                    if !states.contains_key(&id) {
                        states.insert(id, VideoCodecState::default());
                    }
                }
            }
            let current_encoder_name = HwEncoder::current_name();
            if states.len() > 0 {
                let (best, _) = HwEncoder::best(false, true);
                let enabled_h264 = best.h264.is_some()
                    && states.len() > 0
                    && states.iter().all(|(_, s)| s.ScoreH264 > 0);
                let enabled_h265 = best.h265.is_some()
                    && states.len() > 0
                    && states.iter().all(|(_, s)| s.ScoreH265 > 0);

                // score encoder
                let mut score_vpx = SCORE_VPX;
                let mut score_h264 = best.h264.as_ref().map_or(0, |c| c.score);
                let mut score_h265 = best.h265.as_ref().map_or(0, |c| c.score);

                // score decoder
                score_vpx += states.iter().map(|s| s.1.ScoreVpx).sum::<i32>();
                if enabled_h264 {
                    score_h264 += states.iter().map(|s| s.1.ScoreH264).sum::<i32>();
                }
                if enabled_h265 {
                    score_h265 += states.iter().map(|s| s.1.ScoreH265).sum::<i32>();
                }

                if enabled_h265 && score_h265 >= score_vpx && score_h265 >= score_h264 {
                    *current_encoder_name.lock().unwrap() = Some(best.h265.unwrap().name);
                } else if enabled_h264 && score_h264 >= score_vpx && score_h264 >= score_h265 {
                    *current_encoder_name.lock().unwrap() = Some(best.h264.unwrap().name);
                } else {
                    *current_encoder_name.lock().unwrap() = None;
                }
                log::info!(
                    "connection count:{}, h264:{}, h265:{}, score: vpx({}), h264({}), h265({}), set current encoder name {:?}",
                    states.len(),
                    enabled_h264,
                    enabled_h265,
                    score_vpx,
                    score_h264,
                    score_h265,
                    current_encoder_name.lock().unwrap()
                    )
            } else {
                *current_encoder_name.lock().unwrap() = None;
            }
        }
        #[cfg(not(feature = "hwcodec"))]
        {
            let _ = id;
            let _ = update;
        }
    }

    #[inline]
    pub fn current_hw_encoder_name() -> Option<String> {
        // TODO add codec_format
        #[cfg(feature = "hwcodec")]
        if check_hwcodec_config() {
            return HwEncoder::current_name().lock().unwrap().clone();
        } else {
            return None;
        }
        #[cfg(not(feature = "hwcodec"))]
        return None;
    }
}

#[cfg(feature = "hwcodec")]
impl Drop for Decoder {
    fn drop(&mut self) {
        *MY_DECODER_STATE.lock().unwrap() = VideoCodecState {
            ScoreVpx: SCORE_VPX,
            ..Default::default()
        };
    }
}

impl Decoder {
    pub fn video_codec_state() -> VideoCodecState {
        // video_codec_state is mainted by creation and destruction of Decoder.
        // It has been ensured to use after Decoder's creation.
        #[cfg(feature = "hwcodec")]
        if check_hwcodec_config() {
            return MY_DECODER_STATE.lock().unwrap().clone();
        } else {
            return VideoCodecState {
                ScoreVpx: SCORE_VPX,
                ..Default::default()
            };
        }
        #[cfg(not(feature = "hwcodec"))]
        VideoCodecState {
            ScoreVpx: SCORE_VPX,
            ..Default::default()
        }
    }

    pub fn new(config: DecoderCfg) -> Decoder {
        let vpx = VpxDecoder::new(config.vpx).unwrap();
        let decoder = Decoder {
            vpx,
            #[cfg(feature = "hwcodec")]
            hw: HwDecoder::new_decoders(),
            #[cfg(feature = "hwcodec")]
            i420: vec![],
        };

        #[cfg(feature = "hwcodec")]
        {
            let mut state = MY_DECODER_STATE.lock().unwrap();
            state.ScoreVpx = SCORE_VPX;
            state.ScoreH264 = decoder.hw.h264.as_ref().map_or(0, |d| d.info.score);
            state.ScoreH265 = decoder.hw.h265.as_ref().map_or(0, |d| d.info.score);
        }

        decoder
    }

    pub fn handle_video_frame(
        &mut self,
        frame: &video_frame::Union,
        rgb: &mut Vec<u8>,
    ) -> ResultType<bool> {
        match frame {
            video_frame::Union::Vp9s(vp9s) => {
                Decoder::handle_vp9s_video_frame(&mut self.vpx, vp9s, rgb)
            }
            #[cfg(feature = "hwcodec")]
            video_frame::Union::H264s(h264s) => {
                if let Some(decoder) = &mut self.hw.h264 {
                    Decoder::handle_hw_video_frame(decoder, h264s, rgb, &mut self.i420)
                } else {
                    Err(anyhow!("don't support h264!"))
                }
            }
            #[cfg(feature = "hwcodec")]
            video_frame::Union::H265s(h265s) => {
                if let Some(decoder) = &mut self.hw.h265 {
                    Decoder::handle_hw_video_frame(decoder, h265s, rgb, &mut self.i420)
                } else {
                    Err(anyhow!("don't support h265!"))
                }
            }
            _ => Err(anyhow!("unsupported video frame type!")),
        }
    }

    fn handle_vp9s_video_frame(
        decoder: &mut VpxDecoder,
        vp9s: &EncodedVideoFrames,
        rgb: &mut Vec<u8>,
    ) -> ResultType<bool> {
        let mut last_frame = Image::new();
        for vp9 in vp9s.frames.iter() {
            for frame in decoder.decode(&vp9.data)? {
                drop(last_frame);
                last_frame = frame;
            }
        }
        for frame in decoder.flush()? {
            drop(last_frame);
            last_frame = frame;
        }
        if last_frame.is_null() {
            Ok(false)
        } else {
            last_frame.rgb(1, true, rgb);
            Ok(true)
        }
    }

    #[cfg(feature = "hwcodec")]
    fn handle_hw_video_frame(
        decoder: &mut HwDecoder,
        frames: &EncodedVideoFrames,
        rgb: &mut Vec<u8>,
        i420: &mut Vec<u8>,
    ) -> ResultType<bool> {
        let mut ret = false;
        for h264 in frames.frames.iter() {
            for image in decoder.decode(&h264.data)? {
                // TODO: just process the last frame
                if image.bgra(rgb, i420).is_ok() {
                    ret = true;
                }
            }
        }
        return Ok(ret);
    }
}

#[cfg(feature = "hwcodec")]
fn check_hwcodec_config() -> bool {
    if let Some(v) = Config2::get().options.get("enable-hwcodec") {
        return v != "N";
    }
    return true; // default is true
}
