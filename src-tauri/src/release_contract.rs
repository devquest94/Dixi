//! Build contract: the invariants a release build must satisfy.
//!
//! These are checks on the build *configuration* rather than on the running
//! program, so they belong to the crate that ships the binary. They exist
//! because the failure they guard against is silent: a release binary built
//! without `tauri/custom-protocol` starts, occupies the tray and shows a
//! blank window, because its webview looks for a Vite dev server that is not
//! running.
//!
//! The behavioural half of the same check (does the produced binary really
//! embed the frontend?) lives in `scripts/verify-release-protocol.ps1`,
//! because it needs the built artifact.

#[cfg(test)]
use std::path::PathBuf;

/// The crate root, as Cargo reports it.
#[cfg(test)]
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Reads `Cargo.toml` from the crate root.
#[cfg(test)]
fn cargo_toml() -> String {
    let path = crate_root().join("Cargo.toml");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The production build must be able to embed the frontend.
    ///
    /// `tauri/custom-protocol` is what makes Tauri serve the bundled assets
    /// instead of the dev server. Dixi exposes it under its own feature name
    /// so a plain `cargo build --release --features custom-protocol` produces
    /// a shippable binary.
    #[test]
    fn custom_protocol_feature_is_declared_and_forwarded() {
        let manifest = cargo_toml();
        assert!(
            manifest.contains("tauri/custom-protocol"),
            "Cargo.toml must forward the `custom-protocol` feature to `tauri/custom-protocol`: \
             without it a release binary shows a blank window",
        );

        let declares_feature = manifest
            .lines()
            .any(|line| {
                let line = line.trim();
                line.starts_with("custom-protocol")
                    && line.contains("tauri/custom-protocol")
            });
        assert!(
            declares_feature,
            "Cargo.toml must declare a `custom-protocol` feature that enables `tauri/custom-protocol`",
        );
    }

    /// The feature must not be silently disabled for the release profile.
    ///
    /// A `[profile.release]` that lists features cannot remove `custom-protocol`
    /// (cargo only adds features there), but a `default = []` on the package
    /// would leave the plain `cargo build --release` without it. The feature
    /// has to stay reachable by name.
    #[test]
    fn custom_protocol_is_reachable_by_name() {
        let manifest = cargo_toml();
        assert!(
            manifest.contains("custom-protocol"),
            "the `custom-protocol` feature must be requestable as `--features custom-protocol`",
        );
    }

    /// The frontend build output the release embeds must be declared.
    ///
    /// Tauri reads `frontendDist` from `tauri.conf.json`; if it points at a
    /// directory that no longer exists, the release build silently produces a
    /// binary with no frontend at all.
    #[test]
    fn tauri_config_declares_a_frontend_dist() {
        let path = crate_root().join("tauri.conf.json");
        let config = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));

        assert!(
            config.contains("frontendDist"),
            "tauri.conf.json must declare `frontendDist`, otherwise the release binary ships no frontend",
        );
    }
}
