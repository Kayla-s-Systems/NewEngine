use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalPcmLoopRegion {
    pub start_frame: u64,
    pub end_frame: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanonicalPcmBuffer {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
    pub loop_region: Option<CanonicalPcmLoopRegion>,
}

impl CanonicalPcmBuffer {
    pub fn frame_count(&self) -> Result<u64, String> {
        if self.channels == 0 || self.channels > 32 {
            return Err(format!(
                "canonical PCM channels out of range: {}",
                self.channels
            ));
        }
        if !(8_000..=384_000).contains(&self.sample_rate_hz) {
            return Err(format!(
                "canonical PCM sample rate out of range: {} Hz",
                self.sample_rate_hz
            ));
        }
        let channels = usize::from(self.channels);
        if !self.samples.len().is_multiple_of(channels) {
            return Err(format!(
                "canonical PCM sample count {} is not divisible by channels {}",
                self.samples.len(),
                self.channels
            ));
        }
        u64::try_from(self.samples.len() / channels)
            .map_err(|_| "canonical PCM frame count exceeds u64".to_owned())
    }

    pub fn validate(&self) -> Result<(), String> {
        let frame_count = self.frame_count()?;
        if self.samples.iter().any(|sample| !sample.is_finite()) {
            return Err("canonical PCM contains non-finite samples".to_owned());
        }
        if let Some(loop_region) = self.loop_region {
            if loop_region.start_frame >= loop_region.end_frame
                || loop_region.end_frame > frame_count
            {
                return Err(format!(
                    "canonical PCM loop range invalid start={} end={} frames={frame_count}",
                    loop_region.start_frame, loop_region.end_frame
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_interleaved_f32_frames() {
        let pcm = CanonicalPcmBuffer {
            sample_rate_hz: 48_000,
            channels: 2,
            samples: vec![0.0, 0.25, -0.25, 0.5],
            loop_region: Some(CanonicalPcmLoopRegion {
                start_frame: 1,
                end_frame: 2,
            }),
        };
        pcm.validate().expect("valid canonical pcm");
        assert_eq!(pcm.frame_count().unwrap(), 2);
    }
}
