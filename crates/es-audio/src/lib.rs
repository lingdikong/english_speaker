//! 音频块编解码实现。
//!
//! 当前提供 16-bit 小端 PCM（纯 Rust，所有平台零依赖，24kHz 单声道约 2.9MB/分钟）。
//! Opus（约 1/10 体积）作为后续的 `OpusCodec` 加入：每个块独立编码，核心层无需改动。

use es_core::model::PcmFormat;
use es_core::ports::AudioCodec;
use es_core::{CoreError, Result};

#[derive(Default)]
pub struct PcmS16Codec;

impl AudioCodec for PcmS16Codec {
    fn id(&self) -> &str {
        "pcm_s16le"
    }

    fn extension(&self) -> &str {
        "pcm"
    }

    fn encode(&self, pcm: &[f32], _format: PcmFormat) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(pcm.len() * 2);
        for &s in pcm {
            let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            out.extend_from_slice(&v.to_le_bytes());
        }
        Ok(out)
    }

    fn decode(&self, data: &[u8], _format: PcmFormat) -> Result<Vec<f32>> {
        if !data.len().is_multiple_of(2) {
            return Err(CoreError::Audio("truncated pcm_s16le chunk".into()));
        }
        Ok(data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&b| i16::from_le_bytes(b) as f32 / i16::MAX as f32)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let fmt = PcmFormat {
            sample_rate: 24_000,
            channels: 1,
        };
        let codec = PcmS16Codec;
        let data: Vec<f32> = (0..100).map(|i| i as f32 / 200.0 - 0.25).collect();
        let bytes = codec.encode(&data, fmt).unwrap();
        assert_eq!(bytes.len(), 200);
        let back = codec.decode(&bytes, fmt).unwrap();
        assert!(data.iter().zip(&back).all(|(a, b)| (a - b).abs() < 1e-3));
        assert!(codec.decode(&bytes[..3], fmt).is_err());
    }
}
