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
    /// Loading the ASR model. Zero when it was already resident.
    ModelLoad,
    /// The engine run: features, inference and decoding, including the round
    /// trip to the worker process.
    Asr,
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

    /// The one-line summary for the log.
    ///
    /// The format is stable on purpose: the benchmark harness parses it, and
    /// a log that a human can read and a script can parse is worth more than
    /// either alone.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        for stage in [
            Stage::Dispatch,
            Stage::Audio,
            Stage::ModelLoad,
            Stage::Asr,
            Stage::Rules,
            Stage::PostProcess,
            Stage::History,
            Stage::Insertion,
        ] {
            let took = self.stages.get(&stage).copied().unwrap_or_default();
            parts.push(format!("{}={}ms", stage.name(), took.as_millis()));
        }
        parts.push(format!("total={}ms", self.total().as_millis()));
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
            finished_at: origin,
        });
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
