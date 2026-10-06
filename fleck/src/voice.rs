//! Dictation: record from the microphone, then turn the recording into text
//! with Whisper, entirely on this machine - nothing is sent anywhere.
//!
//! The recording is held in memory as the samples Whisper wants, so there is
//! no temporary file, no recorder process to stop, and no WAV header to
//! distrust - see `audio` for how it reaches the sound server.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// The model Fleck dictates with, and where it comes from. `base` is the
/// middle of Whisper's range: good enough for dictating notes, and quick.
pub const MODEL_FILE: &str = "ggml-base.bin";
pub const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";

/// A recording in progress: the samples so far, and the flag that stops the
/// thread filling them.
pub struct Recording {
    samples: Arc<Mutex<Vec<f32>>>,
    stop: Arc<AtomicBool>,
    worker: std::thread::JoinHandle<()>,
}

impl Recording {
    /// Starts recording. Fails here, before anything is drawn as recording, if
    /// the microphone cannot be opened at all.
    pub fn start() -> Result<Self, String> {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        // The thread reports whether the microphone opened, so a missing or
        // busy device is an error from `start` rather than a silent recording.
        let (opened, open_result) = std::sync::mpsc::channel();
        let worker = std::thread::spawn({
            let (samples, stop) = (Arc::clone(&samples), Arc::clone(&stop));
            move || {
                let mut announced = Some(opened);
                let recorded = crate::audio::record(&stop, |chunk| {
                    if let Some(opened) = announced.take() {
                        let _ = opened.send(Ok(()));
                    }
                    if let Ok(mut samples) = samples.lock() {
                        samples.extend_from_slice(chunk);
                    }
                });
                if let (Some(opened), Err(error)) = (announced, &recorded) {
                    let _ = opened.send(Err(error.clone()));
                }
                if let Err(error) = recorded {
                    tracing::error!(%error, "recording stopped early");
                }
            }
        });
        // The first chunk is a quarter of a second away; a device that is going
        // to refuse does it sooner than that.
        match open_result.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(Err(error)) => {
                stop.store(true, Ordering::Relaxed);
                let _ = worker.join();
                Err(error)
            }
            // Timed out: the microphone is open but quiet, which is fine.
            Ok(Ok(())) | Err(_) => Ok(Self {
                samples,
                stop,
                worker,
            }),
        }
    }

    /// Everything said so far, for the live preview. Cheap enough to call every
    /// few seconds: it copies the tail, not the whole recording.
    pub fn tail(&self, window: std::time::Duration) -> Vec<f32> {
        let Ok(samples) = self.samples.lock() else {
            return Vec::new();
        };
        let window = window.as_secs() as usize * crate::audio::SAMPLE_RATE as usize;
        let from = samples.len().saturating_sub(window);
        samples[from..].to_vec()
    }

    /// Stops recording and hands back everything that was said.
    pub fn stop(self) -> Vec<f32> {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.worker.join();
        match self.samples.lock() {
            Ok(mut samples) => std::mem::take(&mut samples),
            Err(_) => Vec::new(),
        }
    }
}

/// Downloads the speech model, about 142 MB, to where `model_path` says it
/// goes. Blocking, and minutes on a slow line, so callers run it off the UI
/// thread.
///
/// Written beside its destination and renamed when it is whole, so an
/// interrupted download never leaves a half model that Whisper would choke on.
pub fn download_model(model: &Path) -> Result<(), String> {
    download(MODEL_URL, model)
}

/// Fetches `url` into `to`, through a partial file so an interrupted download
/// leaves nothing behind that looks finished.
fn download(url: &str, to: &Path) -> Result<(), String> {
    let model = to;
    if let Some(dir) = model.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("couldn't make room for the speech model: {error}"))?;
    }
    let partial = model.with_extension("part");
    let mut response = ureq::get(url)
        .call()
        .map_err(|error| format!("couldn't reach the speech model: {error}"))?;
    let mut file = std::fs::File::create(&partial)
        .map_err(|error| format!("couldn't write the speech model: {error}"))?;
    let written = std::io::copy(&mut response.body_mut().as_reader(), &mut file)
        .map_err(|error| format!("the speech model download stopped: {error}"))?;
    drop(file);
    std::fs::rename(&partial, model)
        .map_err(|error| format!("couldn't put the speech model in place: {error}"))?;
    tracing::info!(bytes = written, model = %model.display(), "downloaded the speech model");
    Ok(())
}

/// Where the speech model lives.
#[must_use]
pub fn model_path(data_dir: &Path) -> PathBuf {
    data_dir.join("models").join(MODEL_FILE)
}

/// How much of a still-running recording the live preview looks at. The whole
/// recording would cost more with every pass; the tail keeps it flat, and the
/// preview only has to show what was just said.
pub const PREVIEW_WINDOW: std::time::Duration = std::time::Duration::from_secs(15);

/// The model, loaded once and kept: it is about 140 MB, and the live preview
/// would otherwise reload it every few seconds.
static MODEL: std::sync::OnceLock<WhisperContext> = std::sync::OnceLock::new();

/// Turns a recording into text. Blocking and slow - seconds for a long note -
/// so callers run it off the UI thread.
pub fn transcribe(samples: &[f32], model: &Path) -> Result<String, String> {
    if samples.is_empty() {
        return Ok(String::new());
    }
    if !model.is_file() {
        return Err(format!(
            "the speech model is missing: download {MODEL_URL} to {}",
            model.display()
        ));
    }

    let context = if let Some(context) = MODEL.get() {
        context
    } else {
        let loaded = WhisperContext::new_with_params(
            model.to_string_lossy().as_ref(),
            WhisperContextParameters::default(),
        )
        .map_err(|error| format!("couldn't load the speech model: {error}"))?;
        // Another thread may have won the race; either context will do.
        let _ = MODEL.set(loaded);
        MODEL.get().expect("the model was just set")
    };
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
        .full(params, samples)
        .map_err(|error| format!("transcribing failed: {error}"))?;

    let mut text = String::new();
    for segment in state.as_iter() {
        if let Ok(segment) = segment.to_str_lossy() {
            text.push_str(segment.as_ref());
        }
    }
    Ok(text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silent_recording_transcribes_to_nothing() {
        // No samples: no model is loaded, so this says nothing about Whisper,
        // only that an empty recording is handled before any of that.
        assert_eq!(
            transcribe(&[], Path::new("/nonexistent/model.bin")),
            Ok(String::new())
        );
    }

    #[test]
    fn transcribing_without_the_model_says_where_to_get_it() {
        let error = transcribe(&[0.1, 0.2], Path::new("/nonexistent/model.bin"))
            .expect_err("no model, no transcription");

        assert!(error.contains(MODEL_URL), "{error}");
    }

    /// Needs the network, so it is not part of `just check`:
    /// `cargo test -- --ignored download`.
    #[test]
    #[ignore = "needs the network"]
    fn a_download_lands_whole_or_not_at_all() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("models").join("downloaded");

        download("https://example.com", &file).expect("a small download");

        assert!(std::fs::metadata(&file).unwrap().len() > 0);
        assert!(
            !file.with_extension("part").exists(),
            "the partial file is renamed, not left behind"
        );
    }

    #[test]
    fn the_model_lives_under_the_data_directory() {
        assert_eq!(
            model_path(Path::new("/home/someone/.local/share/fleck")),
            Path::new("/home/someone/.local/share/fleck/models/ggml-base.bin")
        );
    }
}
