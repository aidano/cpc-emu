// Audio output backends. The cpal backend (feature "audio") drives the
// default output device — WASAPI on Windows, CoreAudio on macOS, ALSA on
// Linux. Without the feature the starter is a stub so callers need no cfg.

use crate::audio::AudioConsumer;

#[cfg(feature = "audio")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Live output handle; dropping it closes the stream.
pub struct AudioOutput {
    #[cfg(feature = "audio")]
    _stream: cpal::Stream,
}

/// Opens the default output device and starts a callback draining
/// `consumer`. Returns the handle plus the actual sample rate, so the PSG
/// can render at the device's rate. Returns None (with a log) when no
/// usable device exists: the emulator then runs silent.
pub fn start(consumer: AudioConsumer) -> Option<(AudioOutput, u32)> {
    #[cfg(feature = "audio")]
    {
        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            log::warn!("no audio output device found; running without sound");
            return None;
        };

        // Prefer f32 output; among those, a range that includes 44.1kHz.
        let configs: Vec<_> = match device.supported_output_configs() {
            Ok(iter) => iter
                .filter(|range| range.sample_format() == cpal::SampleFormat::F32)
                .collect(),
            Err(_) => Vec::new(),
        };
        let Some(range) = configs.iter().min_by_key(|range| {
            let covers_44100 =
                range.min_sample_rate().0 <= 44_100 && 44_100 <= range.max_sample_rate().0;
            (std::cmp::Reverse(covers_44100), range.max_sample_rate().0)
        }) else {
            log::warn!("no f32 audio output configuration; running without sound");
            return None;
        };
        let range = range.clone();
        let target = cpal::SampleRate(44_100);
        let config: cpal::StreamConfig =
            if range.min_sample_rate() <= target && target <= range.max_sample_rate() {
                range.with_sample_rate(target).into()
            } else {
                range.with_max_sample_rate().into()
            };
        let rate = config.sample_rate.0;
        let channels = config.channels as usize;

        // The callback converts mono to the device's channel count and holds
        // the last value through underruns so gaps don't click. `mono` warms
        // up once and is then reused allocation-free. Once per second the
        // callback metering shows the stream's true drain rate, with any
        // underruns called out: they are the audible-artifact signal.
        let mut last_sample: f32 = 0.0;
        let mut mono: Vec<f32> = Vec::new();
        let mut diag_frames: u64 = 0;
        let mut diag_callbacks: u64 = 0;
        let mut diag_underruns: u64 = 0;
        let mut diag_missing: u64 = 0;
        let mut diag_last: Option<std::time::Instant> = None;
        let stream = match device.build_output_stream(
            &config,
            move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                let now = std::time::Instant::now();
                let frames = data.len() / channels.max(1);
                if mono.len() < frames {
                    mono.resize(frames, 0.0);
                }
                let got = consumer.pop(&mut mono[..frames]);
                for frame in 0..frames {
                    let value = if frame < got { mono[frame] } else { last_sample };
                    last_sample = value;
                    for slot in &mut data[frame * channels..frame * channels + channels] {
                        *slot = value;
                    }
                }
                diag_frames += frames as u64;
                diag_callbacks += 1;
                if got < frames {
                    diag_underruns += 1;
                    diag_missing += (frames - got) as u64;
                }
                let elapsed = now.duration_since(*diag_last.get_or_insert(now)).as_secs_f64();
                if elapsed >= 1.0 {
                    log::debug!(
                        "audio stream: {:.1} callbacks/s, {:.0} frames/s consumed (device rate {}), {} underrun callback(s), {} missing frame(s)",
                        diag_callbacks as f64 / elapsed,
                        diag_frames as f64 / elapsed,
                        rate,
                        diag_underruns,
                        diag_missing,
                    );
                    diag_last = Some(now);
                    diag_frames = 0;
                    diag_callbacks = 0;
                    diag_underruns = 0;
                    diag_missing = 0;
                }
            },
            |err| log::error!("audio stream error: {}", err),
            None,
        ) {
            Ok(stream) => stream,
            Err(err) => {
                log::warn!("audio output unavailable ({}); running without sound", err);
                return None;
            }
        };
        if let Err(err) = stream.play() {
            log::warn!("audio output failed to start ({}); running without sound", err);
            return None;
        }
        log::debug!("audio output: {} Hz, {} channel(s)", rate, channels);
        Some((AudioOutput { _stream: stream }, rate))
    }
    #[cfg(not(feature = "audio"))]
    {
        let _ = consumer;
        None
    }
}
