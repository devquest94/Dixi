//! The latency trace of one dictation.
//!
//! A dictation goes through many stages before the text reaches the
//! application the user was typing in. Without a measurement, "Dixi feels
//! slow" has no addressable cause: the only way to find out whether the
//! delay is the model load, the inference, the dictionary or the paste is to
//! time each stage separately.
//!
//! What this module guarantees:
//!
//! * **Monotonic.** Every duration comes from [`Instant`], never from the
//!   wall clock, so a clock adjustment cannot produce a negative latency.
//! * **Private by construction.** Only stage names and durations are
//!   recorded. No audio, no transcribed text, no tokens, no file paths: the
//!   trace cannot leak what was said.
//! * **Cheap.** Recording a stage is one mutex and one map insert. The
//!   trace is off the critical path by construction.
//!
//! The trace of the dictation in flight lives in a process-wide slot. The
//! coordinator serialises dictations, so there is only ever one in flight,
//! and a slot avoids threading a handle through every signature of the
//! pipeline (which crosses a process boundary to the ASR worker).

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The stages of one dictation, in the order they happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// From the moment the user released the shortcut to the moment the
    /// pipeline was told to stop: the release grace and the dispatch.
    Dispatch,
    /// Closing the audio buffer and getting the samples.
    Audio,
    /// The model was not ready and the pipeline waited for it.
    ///
    /// It is the *wait*, not the duration of the load: the load is kicked
    /// off when the recording starts, so it runs while the user speaks and
    /// only the leftover wait shows up in the post-release latency.
    ModelLoad,
    /// The engine run in batch: feature preparation, inference and decoding,
    /// including the round trip to the worker process.
    ///
    /// In the app's default path the transcription is decoded *live* while
    /// the user speaks, so this stage measures almost nothing: the real work
    /// happened before the release and is not part of the post-release
    /// latency. The final queue decode is [`Stage::Finalize`].
    Asr,
    /// The decode of the last chunk, after the release.
    ///
    /// This is the part of the recognition that the user actually waits
    /// for, and it is what separates the release from the text.
    Finalize,
    /// The dictionary, the snippets and the writing profile.
    Rules,
    /// The post-processing by the language model.
    PostProcess,
    /// Writing the history entry.
    History,
    /// Sending the text to the destination application.
    Insertion,
}

impl Stage {
    /// The stable name used in the log and in the JSON summary.
    pub fn name(self) -> &'static str {
        match self {
            Stage::Dispatch => "dispatch",
            Stage::Audio => "audio",
            Stage::ModelLoad => "model_load",
            Stage::Asr => "asr",
            Stage::Finalize => "finalize",
            Stage::Rules => "rules",
            Stage::PostProcess => "post",
            Stage::History => "history",
            Stage::Insertion => "insert",
        }
    }
}

/// The measured stages of one dictation.
#[derive(Debug, Clone)]
pub struct LatencyTrace {
    /// The moment the user stopped talking.
    origin: Instant,
    /// How long each stage took.
    stages: BTreeMap<Stage, Duration>,
    /// The moment the text reached the destination application.
    text_ready_at: Option<Instant>,
    /// The moment the dictation reached its end.
    finished_at: Instant,
}

impl LatencyTrace {
    /// The time from the user stopping to the end of the trace.
    pub fn total(&self) -> Duration {
        self.stages
            .values()
            .copied()
            .sum::<Duration>()
            .max(Duration::ZERO)
    }

    /// The time from the user stopping to the moment the trace was finished.
    pub fn wall(&self) -> Duration {
        self.finished_at
            .saturating_duration_since(self.origin)
    }

    /// The time from the user stopping to the moment the text reached the
    /// destination application.
    ///
    /// This is **the** latency the user perceives: from the release of the
    /// shortcut to the text appearing where they are typing. It is not the
    /// sum of the stages, because the stages are disjoint slices of the
    /// work and the sum leaves out whatever nobody attributed.
    pub fn text_latency(&self) -> Option<Duration> {
        self.text_ready_at
            .map(|at| at.saturating_duration_since(self.origin))
    }

    /// The one-line summary for the log.
    ///
    /// The format is stable on purpose: the benchmark harness parses it, and
    /// a log that a human can read and a script can parse is worth more than
    /// either alone.
    ///
    /// Three totals are printed, and they are not the same thing:
    ///
    /// * `text` is from the release to the text in the destination: the
    ///   latency the user perceives.
    /// * `total` is the sum of the stages: what the pipeline spent.
    /// * `wall` is from the release to the end of the task, which includes
    ///   the bookkeeping that happens after the text is already delivered.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        for stage in [
            Stage::Dispatch,
            Stage::Audio,
            Stage::ModelLoad,
            Stage::Asr,
            Stage::Finalize,
            Stage::Rules,
            Stage::PostProcess,
            Stage::History,
            Stage::Insertion,
        ] {
            let took = self.stages.get(&stage).copied().unwrap_or_default();
            parts.push(format!("{}={}ms", stage.name(), took.as_millis()));
        }
        parts.push(format!("total={}ms", self.total().as_millis()));
        if let Some(text) = self.text_latency() {
            parts.push(format!("text={}ms", text.as_millis()));
        }
        parts.push(format!("wall={}ms", self.wall().as_millis()));
        parts.join(" ")
    }

    /// The same summary as JSON, for the benchmark harness.
    pub fn to_json(&self) -> serde_json::Value {
        let mut stages = serde_json::Map::new();
        for (stage, took) in &self.stages {
            stages.insert(
                stage.name().to_string(),
                serde_json::Value::from(took.as_millis() as u64),
            );
        }
        serde_json::json!({
            "stages": serde_json::Value::Object(stages),
            "total_ms": self.total().as_millis() as u64,
            "text_ms": self.text_latency().map(|d| d.as_millis() as u64),
            "wall_ms": self.wall().as_millis() as u64,
        })
    }
}

/// The dictation in flight.
static CURRENT: Mutex<Option<LatencyTrace>> = Mutex::new(None);

/// Starts the trace of a new dictation.
///
/// `origin` is the moment the user stopped talking. It is not necessarily
/// `Instant::now()`: the release is deferred by a grace window so a key
/// repeat cannot cancel the recording, and that window is part of the
/// latency the user perceives.
pub fn begin_at(origin: Instant) {
    if let Ok(mut slot) = CURRENT.lock() {
        *slot = Some(LatencyTrace {
            origin,
            stages: BTreeMap::new(),
            text_ready_at: None,
            finished_at: origin,
        });
    }
}
/// Records that the text reached the destination application.
///
/// From this instant on, the latency the user perceives is known: it is the
/// distance from the release to here.
pub fn mark_text_ready() {
    if let Ok(mut slot) = CURRENT.lock() {
        if let Some(trace) = slot.as_mut() {
            trace.text_ready_at = Some(Instant::now());
        }
    }
}

/// Records how long a stage took.
pub fn mark(stage: Stage, took: Duration) {
    if let Ok(mut slot) = CURRENT.lock() {
        if let Some(trace) = slot.as_mut() {
            trace.stages.insert(stage, took);
        }
    }
}

/// Records a stage measured from a moment in the past.
pub fn mark_since(stage: Stage, since: Instant) {
    mark(stage, since.elapsed());
}

/// Records a stage measured from the origin of the current trace.
///
/// Used by the stages that do not have their own natural starting point.
pub fn mark_from_origin(stage: Stage) {
    let origin = {
        let Ok(slot) = CURRENT.lock() else { return };
        match slot.as_ref() {
            Some(trace) => trace.origin,
            None => return,
        }
    };
    mark(stage, origin.elapsed());
}

/// A copy of the trace in flight, without ending it.
///
/// Used by the benchmark harness, which runs the same file several times and
/// keeps the fastest run's stages.
pub fn snapshot() -> Option<LatencyTrace> {
    let slot = CURRENT.lock().ok()?;
    let mut trace = slot.as_ref().cloned()?;
    trace.finished_at = Instant::now();
    Some(trace)
}

/// Ends the trace and returns it.
///
/// The trace is removed from the slot, so a later stage of another
/// dictation cannot write into a finished one.
pub fn finish() -> Option<LatencyTrace> {
    let mut slot = CURRENT.lock().ok()?;
    let mut trace = slot.take()?;
    trace.finished_at = Instant::now();
    Some(trace)
}

/// Closes the trace and logs it when it goes out of scope.
///
/// The owner of the trace is the task that runs the pipeline, not the
/// clipboard: the paste happens on the main thread and can finish before
/// the bookkeeping, so closing the trace there would cut the accounting
/// stage short and lose the line on every failure. With the guard, every
/// outcome leaves a closed, logged trace behind.
pub struct TraceGuard;

impl TraceGuard {
    /// Takes ownership of the trace in flight.
    pub fn new() -> Self {
        TraceGuard
    }
}

impl Drop for TraceGuard {
    fn drop(&mut self) {
        if let Some(trace) = finish() {
            log::info!("[latency] {}", trace.summary());
        }
    }
}
