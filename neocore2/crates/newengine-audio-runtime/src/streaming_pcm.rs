use std::io::{BufReader, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use newengine_audio_api::AudioStreamBufferConfig;
use rodio::source::{SeekError, Source};
use rodio::{ChannelCount, Decoder, SampleRate};

use crate::streaming_asset::StreamingAssetIoStats;

#[derive(Debug)]
pub(crate) struct StreamingStats {
    channels: usize,
    capacity_frames: usize,
    queued_samples: AtomicUsize,
    underruns: AtomicU64,
    seek_operations: AtomicU64,
    finished: AtomicBool,
    asset_io: Option<Arc<StreamingAssetIoStats>>,
}

impl StreamingStats {
    #[inline]
    pub(crate) fn buffered_frames(&self) -> usize {
        self.queued_samples.load(Ordering::Relaxed) / self.channels.max(1)
    }

    #[inline]
    pub(crate) fn capacity_frames(&self) -> usize {
        self.capacity_frames
    }

    #[inline]
    pub(crate) fn underruns(&self) -> u64 {
        self.underruns.load(Ordering::Relaxed)
    }

    #[inline]
    pub(crate) fn seek_operations(&self) -> u64 {
        self.seek_operations.load(Ordering::Relaxed)
    }

    #[inline]
    pub(crate) fn range_requests(&self) -> u64 {
        self.asset_io
            .as_ref()
            .map(|stats| stats.range_requests())
            .unwrap_or(0)
    }

    #[inline]
    pub(crate) fn compressed_bytes_fetched(&self) -> u64 {
        self.asset_io
            .as_ref()
            .map(|stats| stats.compressed_bytes_fetched())
            .unwrap_or(0)
    }

    #[inline]
    pub(crate) fn finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }

    fn subtract_queued(&self, samples: usize) {
        let _ = self
            .queued_samples
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some(value.saturating_sub(samples))
            });
    }
}

struct PcmChunk {
    generation: u64,
    samples: Vec<f32>,
}

enum StreamCommand {
    Seek {
        position: Duration,
        generation: u64,
        reply: Sender<Result<(), String>>,
    },
}

pub(crate) struct StreamingPcmSource {
    receiver: Receiver<PcmChunk>,
    commands: Sender<StreamCommand>,
    current: Vec<f32>,
    current_index: usize,
    generation: u64,
    channels: ChannelCount,
    sample_rate: SampleRate,
    total_duration: Option<Duration>,
    stats: Arc<StreamingStats>,
    underrun_latched: bool,
}

impl StreamingPcmSource {
    fn drain_queued_chunks(&mut self) {
        while let Ok(chunk) = self.receiver.try_recv() {
            self.stats.subtract_queued(chunk.samples.len());
        }
    }
}

impl Iterator for StreamingPcmSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current_index < self.current.len() {
                let sample = self.current[self.current_index];
                self.current_index += 1;
                return Some(sample);
            }

            match self.receiver.try_recv() {
                Ok(chunk) => {
                    self.stats.subtract_queued(chunk.samples.len());
                    if chunk.generation != self.generation {
                        continue;
                    }
                    self.current = chunk.samples;
                    self.current_index = 0;
                    self.underrun_latched = false;
                }
                Err(TryRecvError::Empty) => {
                    if self.stats.finished() {
                        return None;
                    }
                    if !self.underrun_latched {
                        self.stats.underruns.fetch_add(1, Ordering::Relaxed);
                        self.underrun_latched = true;
                    }
                    return Some(0.0);
                }
                Err(TryRecvError::Disconnected) => return None,
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl Source for StreamingPcmSource {
    #[inline]
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    #[inline]
    fn channels(&self) -> ChannelCount {
        self.channels
    }

    #[inline]
    fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.total_duration
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        let old_generation = self.generation;
        let next_generation = old_generation.wrapping_add(1);
        self.current.clear();
        self.current_index = 0;
        self.drain_queued_chunks();
        self.stats.finished.store(false, Ordering::Release);
        self.underrun_latched = false;

        let (reply_tx, reply_rx) = crossbeam_channel::bounded(1);
        self.commands
            .send(StreamCommand::Seek {
                position,
                generation: next_generation,
                reply: reply_tx,
            })
            .map_err(|_| seek_other("stream decoder worker is no longer available"))?;

        match reply_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(())) => {
                self.generation = next_generation;
                self.stats.seek_operations.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Ok(Err(error)) => {
                self.generation = old_generation;
                Err(seek_other(error))
            }
            Err(_) => {
                self.generation = old_generation;
                Err(seek_other("stream decoder seek timed out"))
            }
        }
    }
}

fn seek_other(message: impl Into<String>) -> SeekError {
    SeekError::Other(Arc::new(std::io::Error::other(message.into())))
}

pub(crate) fn build_streaming_source<R>(
    reader: R,
    asset_io: Option<Arc<StreamingAssetIoStats>>,
    looping: bool,
    config: AudioStreamBufferConfig,
    start_position: Duration,
    worker_label: &str,
) -> Result<(StreamingPcmSource, Arc<StreamingStats>), String>
where
    R: Read + Seek + Send + Sync + 'static,
{
    let config = config.sanitized();
    let mut reader = reader;
    let original_position = reader
        .stream_position()
        .map_err(|error| format!("stream reader position query failed: {error}"))?;
    let byte_len = reader
        .seek(SeekFrom::End(0))
        .map_err(|error| format!("stream reader length query failed: {error}"))?;
    reader
        .seek(SeekFrom::Start(original_position))
        .map_err(|error| format!("stream reader restore failed: {error}"))?;
    let buffered = BufReader::with_capacity(
        (config.compressed_chunk_bytes as usize).min(256 * 1024),
        reader,
    );
    let builder = Decoder::<BufReader<R>>::builder()
        .with_data(buffered)
        .with_byte_len(byte_len);
    let mut decoder: Box<dyn Source<Item = f32> + Send> = if looping {
        Box::new(
            builder
                .build_looped()
                .map_err(|error| format!("stream loop decoder init failed: {error}"))?,
        )
    } else {
        Box::new(
            builder
                .build()
                .map_err(|error| format!("stream decoder init failed: {error}"))?,
        )
    };
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let total_duration = decoder.total_duration();
    if !start_position.is_zero() {
        decoder
            .try_seek(start_position)
            .map_err(|error| format!("stream initial seek failed: {error}"))?;
    }

    let channels_usize = usize::from(channels.get()).max(1);
    let sample_rate_usize = sample_rate.get() as usize;
    let capacity_frames = ((sample_rate_usize as u64 * config.capacity_ms as u64) / 1_000)
        .max(config.producer_chunk_frames as u64) as usize;
    let prefill_frames = ((sample_rate_usize as u64 * config.prefill_ms as u64) / 1_000)
        .min(capacity_frames as u64) as usize;
    let chunk_frames = config.producer_chunk_frames as usize;
    let chunk_samples = chunk_frames
        .saturating_mul(channels_usize)
        .max(channels_usize);
    let capacity_samples = capacity_frames.saturating_mul(channels_usize);
    let slot_count = (capacity_samples / chunk_samples).max(1);
    let actual_capacity_frames = slot_count.saturating_mul(chunk_frames);
    let (sender, receiver) = crossbeam_channel::bounded::<PcmChunk>(slot_count);
    let (command_tx, command_rx) = crossbeam_channel::bounded::<StreamCommand>(4);
    let stats = Arc::new(StreamingStats {
        channels: channels_usize,
        capacity_frames: actual_capacity_frames,
        queued_samples: AtomicUsize::new(0),
        underruns: AtomicU64::new(0),
        seek_operations: AtomicU64::new(0),
        finished: AtomicBool::new(false),
        asset_io,
    });

    let prefill_samples = prefill_frames.saturating_mul(channels_usize);
    while stats.queued_samples.load(Ordering::Relaxed) < prefill_samples {
        let (chunk, eof) = decode_chunk(&mut decoder, chunk_samples);
        if !chunk.is_empty() {
            let len = chunk.len();
            if sender
                .try_send(PcmChunk {
                    generation: 0,
                    samples: chunk,
                })
                .is_err()
            {
                break;
            }
            stats.queued_samples.fetch_add(len, Ordering::Relaxed);
        }
        if eof {
            stats.finished.store(true, Ordering::Release);
            break;
        }
    }

    let worker_stats = Arc::clone(&stats);
    let name = format!("engine.audio.stream.{worker_label}");
    thread::Builder::new()
        .name(name)
        .spawn(move || {
            stream_decode_worker(decoder, sender, command_rx, worker_stats, chunk_samples);
        })
        .map_err(|error| format!("spawn stream decode worker failed: {error}"))?;

    Ok((
        StreamingPcmSource {
            receiver,
            commands: command_tx,
            current: Vec::new(),
            current_index: 0,
            generation: 0,
            channels,
            sample_rate,
            total_duration,
            stats: Arc::clone(&stats),
            underrun_latched: false,
        },
        stats,
    ))
}

fn stream_decode_worker(
    mut decoder: Box<dyn Source<Item = f32> + Send>,
    sender: Sender<PcmChunk>,
    commands: Receiver<StreamCommand>,
    stats: Arc<StreamingStats>,
    chunk_samples: usize,
) {
    let mut generation = 0_u64;
    loop {
        while let Ok(command) = commands.try_recv() {
            handle_command(command, decoder.as_mut(), &stats, &mut generation);
        }

        if stats.finished() {
            match commands.recv() {
                Ok(command) => {
                    handle_command(command, decoder.as_mut(), &stats, &mut generation);
                    continue;
                }
                Err(_) => return,
            }
        }

        let (chunk, eof) = decode_chunk(decoder.as_mut(), chunk_samples);
        if !chunk.is_empty() {
            let len = chunk.len();
            let packet = PcmChunk {
                generation,
                samples: chunk,
            };
            stats.queued_samples.fetch_add(len, Ordering::Relaxed);
            crossbeam_channel::select_biased! {
                recv(commands) -> command => {
                    stats.subtract_queued(len);
                    if let Ok(command) = command {
                        handle_command(command, decoder.as_mut(), &stats, &mut generation);
                    } else {
                        return;
                    }
                }
                send(sender, packet) -> result => {
                    if result.is_err() {
                        stats.subtract_queued(len);
                        return;
                    }
                }
            }
        }
        if eof {
            stats.finished.store(true, Ordering::Release);
        }
    }
}

fn handle_command(
    command: StreamCommand,
    decoder: &mut dyn Source<Item = f32>,
    stats: &StreamingStats,
    generation: &mut u64,
) {
    match command {
        StreamCommand::Seek {
            position,
            generation: requested_generation,
            reply,
        } => {
            let result = decoder
                .try_seek(position)
                .map_err(|error| error.to_string());
            if result.is_ok() {
                *generation = requested_generation;
                stats.finished.store(false, Ordering::Release);
            }
            let _ = reply.send(result);
        }
    }
}

fn decode_chunk(decoder: &mut dyn Iterator<Item = f32>, max_samples: usize) -> (Vec<f32>, bool) {
    let mut chunk = Vec::with_capacity(max_samples);
    while chunk.len() < max_samples {
        match decoder.next() {
            Some(sample) => chunk.push(sample),
            None => return (chunk, true),
        }
    }
    (chunk, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn mono_pcm16_wav(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16_u32.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        out.extend_from_slice(&2_u16.to_le_bytes());
        out.extend_from_slice(&16_u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        out
    }

    fn test_config() -> AudioStreamBufferConfig {
        AudioStreamBufferConfig {
            capacity_ms: 250,
            prefill_ms: 100,
            producer_chunk_frames: 1_024,
            ..AudioStreamBufferConfig::default()
        }
    }

    #[test]
    fn streaming_source_decodes_pcm_into_bounded_ring_without_device() {
        let samples = (0..4_800)
            .map(|index| if index % 32 < 16 { 16_000 } else { -16_000 })
            .collect::<Vec<i16>>();
        let bytes = mono_pcm16_wav(48_000, &samples);
        let (mut source, stats) = build_streaming_source(
            Cursor::new(bytes),
            None,
            false,
            test_config(),
            Duration::ZERO,
            "test",
        )
        .expect("stream source");
        assert_eq!(source.channels().get(), 1);
        assert_eq!(source.sample_rate().get(), 48_000);
        assert!(stats.capacity_frames() <= 12_000);
        assert!(stats.buffered_frames() <= stats.capacity_frames());

        let decoded = source.by_ref().take(2_048).collect::<Vec<_>>();
        assert_eq!(decoded.len(), 2_048);
        assert!(decoded.iter().all(|sample| sample.is_finite()));
        assert!(decoded.iter().any(|sample| sample.abs() > 0.1));
        assert!(stats.buffered_frames() <= stats.capacity_frames());
    }

    #[test]
    fn streaming_source_supports_live_seek_and_refills_new_generation() {
        let samples = (0..48_000)
            .map(|index| if index < 24_000 { 4_000 } else { 20_000 })
            .collect::<Vec<i16>>();
        let bytes = mono_pcm16_wav(48_000, &samples);
        let (mut source, stats) = build_streaming_source(
            Cursor::new(bytes),
            None,
            false,
            test_config(),
            Duration::ZERO,
            "seek-test",
        )
        .expect("stream source");
        source
            .try_seek(Duration::from_millis(750))
            .expect("live seek");
        assert_eq!(stats.seek_operations(), 1);
        let samples = source.by_ref().take(256).collect::<Vec<_>>();
        assert_eq!(samples.len(), 256);
        assert!(samples.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn empty_ring_underrun_zero_fills_without_blocking_consumer() {
        let (_tx, rx) = crossbeam_channel::bounded::<PcmChunk>(1);
        let (command_tx, _command_rx) = crossbeam_channel::bounded::<StreamCommand>(1);
        let stats = Arc::new(StreamingStats {
            channels: 1,
            capacity_frames: 128,
            queued_samples: AtomicUsize::new(0),
            underruns: AtomicU64::new(0),
            seek_operations: AtomicU64::new(0),
            finished: AtomicBool::new(false),
            asset_io: None,
        });
        let mut source = StreamingPcmSource {
            receiver: rx,
            commands: command_tx,
            current: Vec::new(),
            current_index: 0,
            generation: 0,
            channels: ChannelCount::new(1).expect("mono"),
            sample_rate: SampleRate::new(48_000).expect("sample rate"),
            total_duration: None,
            stats: Arc::clone(&stats),
            underrun_latched: false,
        };
        assert_eq!(source.next(), Some(0.0));
        assert_eq!(source.next(), Some(0.0));
        assert_eq!(stats.underruns(), 1);
    }

    #[test]
    fn ring_stats_report_frames_not_interleaved_samples() {
        let stats = StreamingStats {
            channels: 2,
            capacity_frames: 4_800,
            queued_samples: AtomicUsize::new(960),
            underruns: AtomicU64::new(3),
            seek_operations: AtomicU64::new(2),
            finished: AtomicBool::new(false),
            asset_io: None,
        };
        assert_eq!(stats.buffered_frames(), 480);
        assert_eq!(stats.capacity_frames(), 4_800);
        assert_eq!(stats.underruns(), 3);
        assert_eq!(stats.seek_operations(), 2);
    }
}
