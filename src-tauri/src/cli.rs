use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone, Default)]
#[command(name = "dixi", about = "Dixi - dictado por voz local")]
pub struct CliArgs {
    /// Start with the main window hidden
    #[arg(long)]
    pub start_hidden: bool,

    /// Disable the system tray icon
    #[arg(long)]
    pub no_tray: bool,

    /// Toggle transcription on/off (sent to running instance)
    #[arg(long)]
    pub toggle_transcription: bool,

    /// Toggle transcription with post-processing on/off (sent to running instance)
    #[arg(long)]
    pub toggle_post_process: bool,

    /// Cancel the current operation (sent to running instance)
    #[arg(long)]
    pub cancel: bool,

    /// Enable debug mode with verbose logging
    #[arg(long)]
    pub debug: bool,

    /// Transcribe this WAV (16 kHz mono) headlessly and exit. Runs the same
    /// batch transcription path as the app — no mic, no silence removal, no
    /// download (the model must already be installed). Long audio is still
    /// chunked at pauses, as in the app.
    #[arg(short = 'f', long, value_name = "WAV")]
    pub transcribe_file: Option<PathBuf>,

    /// Transcribe this WAV through the LIVE path: the one the app uses while
    /// the user speaks. The audio is fed at the speed of a recording and the
    /// report says what was decoded before the release and how much was still
    /// pending when the user let go. No paste, no download.
    #[arg(long, value_name = "WAV")]
    pub live_file: Option<PathBuf>,

    /// Download a catalog model into Dixi's own models folder.
    ///
    /// Uses the app's own transport: resumable, HTTPS, a `.partial` of its
    /// own, the catalog size and SHA-256 as the trust anchor, and the file is
    /// published by rename only when it is complete and verified. The source
    /// is the Hugging Face resolve URL of the revision the catalog pins.
    ///
    /// It does NOT change the selected model.
    #[arg(long, value_name = "ID")]
    pub download_model: Option<String>,

    /// Feed the live bench faster than real time. Development only: the
    /// latency it reports is only meaningful at 1x.
    #[arg(long)]
    pub fast_feed: bool,

    /// Model id to load for --transcribe-file (default: the selected model).
    #[arg(long)]
    pub model: Option<String>,

    /// Hard-select the compute device for --transcribe-file by its registry
    /// index (see --list-devices). Omit to use the persisted accelerator
    /// setting. transcribe-cpp (whisper-family) models only.
    #[arg(long, value_name = "N")]
    pub device_index: Option<usize>,

    /// List the transcribe-cpp compute devices (with indices) and exit.
    #[arg(long)]
    pub list_devices: bool,

    /// List the available models (with ids) and exit. Pass an id to --model.
    /// Honors --json for machine-readable output.
    #[arg(long)]
    pub list_models: bool,

    /// Repeat the transcription N times (best_ms reports the fastest run).
    #[arg(long, value_name = "N")]
    pub repeat: Option<usize>,

    /// Emit --transcribe-file results as JSON.
    #[arg(long)]
    pub json: bool,

    /// Regenerate the TypeScript IPC bindings (`src/bindings.ts`) and exit.
    ///
    /// The bindings are the frontend's contract with the Rust command
    /// registry. Regenerating them from a release build keeps that contract
    /// verifiable without rebuilding the whole engine in debug mode, which is
    /// the expensive part of the build on a constrained machine.
    #[arg(long)]
    pub export_bindings: bool,
}
