//! A headless bench of the LIVE decode path: the one the app uses while the
//! user is still speaking.
//!
//! The batch path (`--transcribe-file`) measures a run over a finished clip.
//! That is not the latency the user perceives: in the app the audio reaches
//! the decoder while it is being recorded, and only what is left when the
//! user lets go of the shortcut is decoded afterwards.
//!
//! This bench drives the real live path — the same `start_stream` worker, the
//! same router, the same `transcribe_recording` — feeding the audio at the
//! speed of a recording. It then reports:
//!
//! * how many fragments were decoded **before** the release, and how much
//!   audio they covered;
//! * how much audio was **still pending** when the user let go;
//! * how long the last fragment took;
//! * the whole post-release cost, stage by stage.
//!
//! Nothing here pastes anything into an application: the paste lives in the
//! app's action pipeline, not in the transcription manager, so this bench
//! cannot disturb what the user has focused.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::latency;
use crate::managers::transcription::TranscriptionManager;

/// How much audio is handed over at a time, in samples. 100 ms, the same
/// frame the recorder delivers.
const FEED_SAMPLES: usize = 1600;

/// A pause between runs, so the live worker of the previous run has released
/// its slot before the next one opens a new route.
const RUN_GAP: Duration = Duration::from_millis(250);

/// Runs the live bench. Returns a process exit code (0 ok, 1 runtime failure,
/// 2 bad input).
pub fn run(
    app: &AppHandle,
    wav: &Path,
    model_id: &str,
    device_index: Option<usize>,
    repeat: usize,
    realtime: bool,
    cancel_after_ms: Option<u64>,
) -> i32 {
    // The same format the app records: 16 kHz mono 16-bit PCM. Anything else
    // would be timed and decoded as garbage.
    match hound::WavReader::open(wav) {
        Ok(reader) => {
            let spec = reader.spec();
            if spec.sample_rate != 16_000
                || spec.channels != 1
                || spec.bits_per_sample != 16
                || spec.sample_format != hound::SampleFormat::Int
            {
                eprintln!(
                    "error: expected 16 kHz mono 16-bit PCM WAV, got {} Hz / {} ch / {}-bit {:?}",
                    spec.sample_rate, spec.channels, spec.bits_per_sample, spec.sample_format
                );
                return 2;
            }
        }
        Err(e) => {
            eprintln!("error: cannot open {}: {}", wav.display(), e);
            return 2;
        }
    }

    let samples = match crate::audio_toolkit::read_wav_samples(wav) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to read {}: {}", wav.display(), e);
            return 2;
        }
    };
    let audio_secs = samples.len() as f64 / 16_000.0;

    let tm = app.state::<Arc<TranscriptionManager>>();

    let load_start = Instant::now();
    if let Err(e) = tm.load_model_with_device(model_id, device_index) {
        eprintln!("error: load_model('{}') failed: {}", model_id, e);
        return 1;
    }
    let load_ms = load_start.elapsed().as_millis() as u64;
    let bound_backend = tm.current_backend();

    let mut runs = Vec::new();
    let mut text = String::new();

    for run_index in 0..repeat.max(1) {
        latency::reset_chunks();

        // The live worker, exactly as the app starts it when the user presses
        // the shortcut.
        tm.start_stream();
        let router = tm.stream_router();

        // Feed the audio at the speed of a recording: a fragment only counts
        // as "decoded while the user spoke" if the decoder had it in hand
        // before the release.
        let feed_started = Instant::now();
        for (index, slice) in samples.chunks(FEED_SAMPLES).enumerate() {
            router.feed(slice);
            if realtime {
                let due = feed_started
                    + Duration::from_secs_f64((index + 1) as f64 * FEED_SAMPLES as f64 / 16_000.0);
                let now = Instant::now();
                if due > now {
                    std::thread::sleep(due - now);
                }
            }
        }

        // The release: the moment the user lets go of the shortcut.
        let release_at = Instant::now();
        latency::begin_at(release_at);

        // The explicit cancellation, the way the user's cancel shortcut does
        // it: N ms after the release the engine is stopped. It measures what
        // a cancellation with pending audio costs, and that it is prompt
        // instead of waiting for the whole pending audio to be decoded.
        //
        // The flag is cleared as soon as the transcription ends, so a cancel
        // that arrives late does not stop the NEXT run's worker.
        let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_handle = cancel_after_ms.map(|ms| {
            let tm = Arc::clone(&tm);
            let finished = Arc::clone(&finished);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(ms));
                if finished.load(std::sync::atomic::Ordering::Acquire) {
                    return;
                }
                tm.cancel_transcription();
            })
        });

        let finalize_started = Instant::now();
        let mut cancelled = false;
        match tm.transcribe_recording(samples.clone()) {
            Ok(out) => text = out,
            Err(e) => {
                // A cancellation is not a failure: it is the scenario being
                // measured. Anything else is a real error.
                let is_cancel = e
                    .downcast_ref::<crate::engine_supervisor::EngineError>()
                    .is_some_and(|err| matches!(err, crate::engine_supervisor::EngineError::Cancelled));
                if !is_cancel {
                    eprintln!("error: live transcription failed: {}", e);
                    return 1;
                }
                cancelled = true;
            }
        }
        finished.store(true, std::sync::atomic::Ordering::Release);
        if let Some(handle) = cancel_handle {
            let _ = handle.join();
        }
        let post_release_ms = finalize_started.elapsed().as_millis() as u64;
        // How far behind the recognition was when the user let go: the audio
        // that was captured and not yet recognized. The manager publishes it
        // when the stream is finalized (or abandoned), so it is the measure
        // of the queue the app had to drain.
        let backlog_ms = tm.stream_backlog_ms();

        let stages = latency::snapshot().map(|trace| trace.to_json());
        let chunks = latency::chunks();

        // A fragment decoded before the release is one the user did not wait
        // for. The audio they cover is what the release freed them of.
        let mut decoded_while_recording = 0usize;
        let mut decoded_secs = 0.0f64;
        let mut chunk_reports = Vec::new();
        let mut last_chunk_ms = 0u64;
        for chunk in &chunks {
            let ended_vs_release_ms = match chunk.ended_at.checked_duration_since(release_at) {
                Some(after) => after.as_millis() as i64,
                None => -(release_at.duration_since(chunk.ended_at).as_millis() as i64),
            };
            if ended_vs_release_ms < 0 {
                decoded_while_recording += 1;
                decoded_secs += chunk.audio_secs;
            }
            last_chunk_ms = chunk.decode_ms;
            chunk_reports.push(json!({
                "index": chunk.index,
                "audio_secs": (chunk.audio_secs * 1000.0).round() / 1000.0,
                "decode_ms": chunk.decode_ms,
                "ended_vs_release_ms": ended_vs_release_ms,
            }));
        }

        // `asr` only gets marked when the live path produced nothing and the
        // recording fell back to a batch run: that is a different path and the
        // report has to say so.
        let path = stages
            .as_ref()
            .and_then(|s| s.get("stages"))
            .and_then(|s| s.get("asr"))
            .and_then(|v| v.as_u64())
            .filter(|ms| *ms > 0)
            .map(|_| "batch")
            .unwrap_or("live");

        runs.push(json!({
            "run": run_index + 1,
            "path": path,
            "post_release_ms": post_release_ms,
            "stages": stages,
            "chunks": chunk_reports,
            "chunks_decoded_while_recording": decoded_while_recording,
            "decoded_secs_while_recording": (decoded_secs * 1000.0).round() / 1000.0,
            "pending_secs_at_release": ((audio_secs - decoded_secs) * 1000.0).round() / 1000.0,
            "last_chunk_ms": last_chunk_ms,
            "cancelled": cancelled,
            "cancel_after_ms": cancel_after_ms,
            "backlog_ms_at_release": backlog_ms,
        }));

        // A cancelled run leaves the engine without a worker: the next one
        // starts fresh. Stopping here keeps the cancellation from leaking
        // into the following run.
        if cancelled {
            break;
        }

        std::thread::sleep(RUN_GAP);
    }

    println!(
        "{}",
        json!({
            "model": model_id,
            "bound_backend": bound_backend,
            "audio_secs": audio_secs,
            "load_ms": load_ms,
            "realtime_feed": realtime,
            "runs": runs,
            "text": text,
        })
    );
    0
}
