//! Dictation: record from the microphone, then turn the recording into text
//! with Whisper, entirely on this machine - nothing is sent anywhere.
//!
//! Recording shells out to PipeWire's or PulseAudio's own recorder rather than
//! opening the audio device itself: both ship with every COSMIC desktop, and
//! it keeps Fleck out of the business of audio devices entirely.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Recorders to try, in order.
const RECORDERS: [&str; 2] = ["pw-record", "parecord"];
/// What Whisper expects: 16 kHz, one channel, 16-bit samples.
const SAMPLE_RATE: &str = "16000";
const CHANNELS: &str = "1";
const FORMAT: &str = "s16";
/// The model Fleck dictates with, and where it comes from. `base` is the
/// middle of Whisper's range: good enough for dictating notes, and quick.
pub const MODEL_FILE: &str = "ggml-base.bin";
pub const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";

/// A recording in progress: the recorder, and the file it is writing.
pub struct Recording {
    recorder: Child,
    pub path: PathBuf,
}

impl Recording {
    /// Starts recording to a new file in the cache folder.
    pub fn start() -> Result<Self, String> {
        let path =
            std::env::temp_dir().join(format!("fleck-dictation-{}.wav", uuid::Uuid::new_v4()));
        for recorder in RECORDERS {
            let started = Command::new(recorder)
                .args([
                    "--rate",
                    SAMPLE_RATE,
                    "--channels",
                    CHANNELS,
                    "--format",
                    FORMAT,
                ])
                .arg(&path)
                .spawn();
            match started {
                Ok(recorder) => return Ok(Self { recorder, path }),
                Err(error) => tracing::debug!(?error, recorder, "recorder not available"),
            }
        }
        Err("no recorder found: install pipewire-bin or pulseaudio-utils".to_string())
    }

    /// Stops the recorder and hands back the file it wrote.
    pub fn stop(mut self) -> PathBuf {
        // The recorders write their header as they go and finish cleanly on a
        // kill, which is how they are meant to be stopped.
        let _ = self.recorder.kill();
        let _ = self.recorder.wait();
        self.path
    }
}

/// Where the speech model lives.
#[must_use]
pub fn model_path(data_dir: &Path) -> PathBuf {
    data_dir.join("models").join(MODEL_FILE)
}

/// Turns a recording into text. Blocking and slow - seconds for a long note -
/// so callers run it off the UI thread.
pub fn transcribe(audio: &Path, model: &Path) -> Result<String, String> {
    let samples = read_samples(audio)?;
    if samples.is_empty() {
        return Ok(String::new());
    }
    if !model.is_file() {
        return Err(format!(
            "the speech model is missing: download {MODEL_URL} to {}",
            model.display()
        ));
    }

    let context = WhisperContext::new_with_params(
        model.to_string_lossy().as_ref(),
        WhisperContextParameters::default(),
    )
    .map_err(|error| format!("couldn't load the speech model: {error}"))?;
    let mut state = context
        .create_state()
        .map_err(|error| format!("couldn't start transcribing: {error}"))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    // Whisper prints its own progress to stdout unless told not to.
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    state
        .full(params, &samples)
        .map_err(|error| format!("transcribing failed: {error}"))?;

    let mut text = String::new();
    for segment in state.as_iter() {
        if let Ok(segment) = segment.to_str_lossy() {
            text.push_str(segment.as_ref());
        }
    }
    Ok(text.trim().to_string())
}

/// The recording as the mono 32-bit samples Whisper wants.
///
/// Reads the file rather than trusting its header: a recorder that is stopped
/// mid-flight can leave the length field at zero while the audio itself is all
/// there, and a reader that believes the header hands back silence.
fn read_samples(audio: &Path) -> Result<Vec<f32>, String> {
    let bytes =
        std::fs::read(audio).map_err(|error| format!("couldn't read the recording: {error}"))?;
    let (channels, pcm) = wav_pcm(&bytes)?;
    let samples: Vec<f32> = pcm
        .as_chunks::<2>()
        .0
        .iter()
        .map(|sample| f32::from(i16::from_le_bytes(*sample)) / f32::from(i16::MAX))
        .collect();
    tracing::debug!(
        file = %audio.display(),
        bytes = bytes.len(),
        channels,
        samples = samples.len(),
        "read a recording"
    );
    if channels <= 1 {
        return Ok(samples);
    }
    // Whisper takes one channel; anything else is averaged down.
    let channels = usize::from(channels);
    Ok(samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect())
}

/// The channel count and the raw sample bytes of a 16-bit PCM WAV file. A
/// `data` chunk whose length is zero or overlong (an interrupted recording)
/// falls back to the rest of the file.
fn wav_pcm(bytes: &[u8]) -> Result<(u16, &[u8]), String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("that recording isn't a WAV file".to_string());
    }
    let mut channels = 1;
    let mut offset = 12;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        let body = offset + 8;
        match id {
            b"fmt " if body + 4 <= bytes.len() => {
                channels = u16::from_le_bytes([bytes[body + 2], bytes[body + 3]]).max(1);
            }
            b"data" => {
                let available = bytes.len() - body;
                let length = if size == 0 || size > available {
                    available
                } else {
                    size
                };
                return Ok((channels, &bytes[body..body + length]));
            }
            _ => {}
        }
        // Chunks are padded to an even length.
        offset = body + size + usize::from(!size.is_multiple_of(2));
    }
    Err("that recording holds no audio".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silent_recording_transcribes_to_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("silence.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let writer = hound::WavWriter::create(&path, spec).unwrap();
        writer.finalize().unwrap();

        // No samples: no model is loaded, so this says nothing about Whisper,
        // only that an empty recording is handled before any of that.
        assert_eq!(
            transcribe(&path, Path::new("/nonexistent/model.bin")),
            Ok(String::new())
        );
    }

    #[test]
    fn a_recording_whose_header_says_it_is_empty_is_still_read() {
        // What an interrupted recorder leaves behind: a zero length, with the
        // audio after it.
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&0u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // one channel
        wav.extend_from_slice(&16_000u32.to_le_bytes());
        wav.extend_from_slice(&32_000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&0u32.to_le_bytes());
        for sample in [100i16, -100, 300] {
            wav.extend_from_slice(&sample.to_le_bytes());
        }

        let (channels, pcm) = wav_pcm(&wav).expect("a readable recording");

        assert_eq!(channels, 1);
        assert_eq!(pcm.len(), 6, "all three samples, despite the header");
    }

    #[test]
    fn samples_are_mixed_down_to_one_channel() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stereo.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        // Two frames, each with the channels at opposite ends.
        for _ in 0..2 {
            writer.write_sample(i16::MAX).unwrap();
            writer.write_sample(-i16::MAX).unwrap();
        }
        writer.finalize().unwrap();

        let samples = read_samples(&path).unwrap();

        assert_eq!(samples.len(), 2, "one sample per frame");
        assert!(samples.iter().all(|sample| sample.abs() < f32::EPSILON));
    }

    #[test]
    fn the_model_lives_under_the_data_directory() {
        assert_eq!(
            model_path(Path::new("/home/someone/.local/share/fleck")),
            Path::new("/home/someone/.local/share/fleck/models/ggml-base.bin")
        );
    }
}
