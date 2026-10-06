//! Recording and playback, in this process.
//!
//! Both go through PulseAudio's simple API, which PipeWire serves on every
//! COSMIC desktop. Fleck used to shell out to `pw-record` and `pw-play`; those
//! binaries do not exist inside a Flatpak sandbox, and asking the sound server
//! directly for the format we want is no more code than spawning them was.
//!
//! The server resamples for us, so a recording arrives as 16 kHz mono however
//! the microphone is configured - exactly what Whisper expects.

use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;

/// What Whisper expects of a recording.
pub const SAMPLE_RATE: u32 = 16_000;
/// How much audio a single read asks for: a quarter of a second, so stopping
/// never waits long on the read in flight.
const CHUNK: usize = SAMPLE_RATE as usize / 4;

/// A stream's format: 16-bit samples, native byte order, one or more channels.
fn spec(rate: u32, channels: u8) -> Spec {
    Spec {
        format: Format::S16NE,
        rate,
        channels,
    }
}

/// Opens the microphone. The name shows up in COSMIC's sound settings next to
/// the application, so it says what Fleck is doing with it.
fn microphone() -> Result<Simple, String> {
    Simple::new(
        None,
        "Fleck",
        Direction::Record,
        None,
        "Dictation",
        &spec(SAMPLE_RATE, 1),
        None,
        None,
    )
    .map_err(|error| format!("couldn't open the microphone: {error}"))
}

/// Records until `stop` is set, handing every chunk to `collect`.
///
/// Runs on its own thread: each read blocks until the server has that much
/// audio. The caller sees the samples as they arrive, which is what the live
/// preview reads.
pub fn record(
    stop: &std::sync::atomic::AtomicBool,
    mut collect: impl FnMut(&[f32]),
) -> Result<(), String> {
    let stream = microphone()?;
    let mut buffer = vec![0i16; CHUNK];
    let mut samples = vec![0f32; CHUNK];
    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        if let Err(error) = stream.read(as_bytes_mut(&mut buffer)) {
            return Err(format!("the microphone stopped: {error}"));
        }
        for (sample, raw) in samples.iter_mut().zip(&buffer) {
            *sample = f32::from(*raw) / f32::from(i16::MAX);
        }
        collect(&samples);
    }
    Ok(())
}

/// Plays 16-bit PCM and waits for it to finish.
pub fn play(pcm: &[u8], rate: u32, channels: u8) -> Result<(), String> {
    let stream = Simple::new(
        None,
        "Fleck",
        Direction::Playback,
        None,
        "Reminder",
        &spec(rate, channels),
        None,
        None,
    )
    .map_err(|error| format!("couldn't open the speakers: {error}"))?;
    stream
        .write(pcm)
        .map_err(|error| format!("couldn't play the sound: {error}"))?;
    stream
        .drain()
        .map_err(|error| format!("the sound was cut off: {error}"))
}

/// The samples as the bytes the sound server reads and writes.
fn as_bytes_mut(samples: &mut [i16]) -> &mut [u8] {
    // Sound is bytes on the wire; i16 has no padding, so this is the same
    // buffer seen as what the C API takes.
    unsafe {
        std::slice::from_raw_parts_mut(
            samples.as_mut_ptr().cast::<u8>(),
            std::mem::size_of_val(samples),
        )
    }
}

/// The channel count, sample rate and raw sample bytes of a 16-bit PCM WAV
/// file. A `data` chunk whose length is zero or overlong falls back to the rest
/// of the file, so a half-written recording still plays.
pub fn wav_pcm(bytes: &[u8]) -> Result<(u8, u32, &[u8]), String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("that isn't a WAV file".to_string());
    }
    let mut channels = 1;
    let mut rate = SAMPLE_RATE;
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
            b"fmt " if body + 8 <= bytes.len() => {
                channels = u16::from_le_bytes([bytes[body + 2], bytes[body + 3]]).max(1) as u8;
                rate = u32::from_le_bytes([
                    bytes[body + 4],
                    bytes[body + 5],
                    bytes[body + 6],
                    bytes[body + 7],
                ]);
            }
            b"data" => {
                let available = bytes.len() - body;
                let length = if size == 0 || size > available {
                    available
                } else {
                    size
                };
                return Ok((channels, rate, &bytes[body..body + length]));
            }
            _ => {}
        }
        // Chunks are padded to an even length.
        offset = body + size + usize::from(!size.is_multiple_of(2));
    }
    Err("that WAV file holds no audio".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WAV header, as a recorder or a converter writes one.
    fn wav(channels: u16, rate: u32, samples: &[i16], claimed_length: Option<u32>) -> Vec<u8> {
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&0u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        wav.extend_from_slice(&(channels * 2).to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        let length = claimed_length.unwrap_or((samples.len() * 2) as u32);
        wav.extend_from_slice(&length.to_le_bytes());
        for sample in samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        wav
    }

    #[test]
    fn reads_the_format_and_the_samples() {
        let file = wav(2, 44_100, &[1, -1, 2, -2], None);

        let (channels, rate, pcm) = wav_pcm(&file).expect("a readable WAV file");

        assert_eq!((channels, rate), (2, 44_100));
        assert_eq!(pcm.len(), 8);
    }

    #[test]
    fn a_wav_file_that_says_it_is_empty_is_still_read() {
        // What an interrupted writer leaves behind: a zero length, with the
        // audio after it.
        let file = wav(1, SAMPLE_RATE, &[100, -100, 300], Some(0));

        let (_, _, pcm) = wav_pcm(&file).expect("a readable WAV file");

        assert_eq!(pcm.len(), 6, "all three samples, despite the header");
    }

    #[test]
    fn anything_else_is_not_a_wav_file() {
        assert!(wav_pcm(b"not audio at all").is_err());
    }

    /// The one check that needs a sound server and a microphone, so it is not
    /// part of `just check`: `cargo test -- --ignored microphone`.
    #[test]
    #[ignore = "needs a microphone and a sound server"]
    fn the_microphone_records_at_the_rate_whisper_wants() {
        let stop = std::sync::atomic::AtomicBool::new(false);
        let mut recorded = 0;

        record(&stop, |chunk| {
            recorded += chunk.len();
            // Half a second is enough to know the stream is running.
            if recorded >= SAMPLE_RATE as usize / 2 {
                stop.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        })
        .expect("the microphone opens");

        assert!(recorded >= SAMPLE_RATE as usize / 2, "{recorded} samples");
    }
}
