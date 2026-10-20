//! Protection of the secrets stored on disk.
//!
//! The API keys of the post-processing providers live in the settings store.
//! The [`SecretMap`](crate::settings::SecretMap) type only redacts them when
//! the settings are printed to the log; the value written to disk used to be
//! **plain text**, which means any process running as the same user, any
//! backup of the data directory and any file synchronisation tool could read
//! the keys.
//!
//! On Windows they are protected with DPAPI (`CryptProtectData`), the
//! platform's own mechanism for exactly this: the blob can only be
//! unprotected by a process running as the same user on the same machine.
//!
//! The values are encrypted at the **storage boundary** (see
//! [`crate::settings::write_settings`] and [`crate::settings::get_settings`]),
//! so the rest of the program keeps working with plain text and nothing
//! else has to know about the protection.
//!
//! # Format
//!
//! A protected value is stored as `dpapi:v1:<hex>`, where the hex is the
//! DPAPI blob. The prefix makes the format recognisable, so a value written
//! by an older version (plain text) still loads and is protected the next
//! time the settings are written.
//!
//! # Portability
//!
//! DPAPI binds the blob to the user account **and** the machine. A settings
//! file copied to another computer, or opened by another user, cannot be
//! unprotected: the key is then treated as lost and has to be entered again.
//! That is the trade-off of protecting a secret with the platform's user
//! store, and the reason the portable mode is documented as not carrying
//! credentials between machines.

use std::fmt;

/// Marker that identifies a value protected with DPAPI.
const DPAPI_PREFIX: &str = "dpapi:v1:";

/// Prefix of the hexadecimal encoding of a protected blob.
const HEX_PREFIX: &str = "dpapi:v1:";

/// A secret that could not be protected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectionError {
    /// What was being attempted, in a form safe to log.
    pub context: String,
    /// The underlying failure, in a form safe to log.
    pub detail: String,
}

impl fmt::Display for ProtectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.context, self.detail)
    }
}

impl std::error::Error for ProtectionError {}

/// Whether a stored value is already protected.
pub fn is_protected(stored: &str) -> bool {
    stored.starts_with(DPAPI_PREFIX)
}

/// Protects a secret for storage.
///
/// Returns the value in the `dpapi:v1:<hex>` form. An empty secret stays
/// empty: there is nothing to protect and the providers treat an empty key
/// as "not configured".
#[cfg(target_os = "windows")]
pub fn protect(plaintext: &str) -> Result<String, ProtectionError> {
    if plaintext.is_empty() {
        return Ok(String::new());
    }

    let blob = windows_dpapi::protect(plaintext.as_bytes()).map_err(|detail| ProtectionError {
        context: "failed to protect the secret with DPAPI".to_string(),
        detail,
    })?;

    Ok(format!("{HEX_PREFIX}{}", to_hex(&blob)))
}

/// Unprotects a stored value.
///
/// A value without the marker is returned unchanged: that is a secret written
/// by an older version, and refusing to read it would throw away the user's
/// key. The value is protected the next time the settings are written.
#[cfg(target_os = "windows")]
pub fn unprotect(stored: &str) -> Result<String, ProtectionError> {
    if stored.is_empty() || !is_protected(stored) {
        return Ok(stored.to_string());
    }

    let hex = &stored[HEX_PREFIX.len()..];
    let blob = from_hex(hex).ok_or_else(|| ProtectionError {
        context: "the protected secret is corrupted".to_string(),
        detail: "the stored value is not valid hexadecimal".to_string(),
    })?;

    let plaintext = windows_dpapi::unprotect(&blob).map_err(|detail| ProtectionError {
        context: "failed to unprotect the secret with DPAPI".to_string(),
        detail,
    })?;

    String::from_utf8(plaintext).map_err(|_| ProtectionError {
        context: "the protected secret is corrupted".to_string(),
        detail: "the unprotected bytes are not valid UTF-8".to_string(),
    })
}

/// Without DPAPI the secret cannot be protected. The caller decides what to
/// do with it; the value itself is never logged.
#[cfg(not(target_os = "windows"))]
pub fn protect(plaintext: &str) -> Result<String, ProtectionError> {
    if plaintext.is_empty() {
        return Ok(String::new());
    }
    Err(ProtectionError {
        context: "this platform has no DPAPI".to_string(),
        detail: "secrets can only be protected on Windows".to_string(),
    })
}

/// Without DPAPI a stored secret is returned unchanged.
#[cfg(not(target_os = "windows"))]
pub fn unprotect(stored: &str) -> Result<String, ProtectionError> {
    Ok(stored.to_string())
}

/// The Windows DPAPI calls, kept apart from the platform-independent code.
///
/// The two functions are declared directly instead of through the `windows`
/// crate: enabling `Win32_Security_Cryptography` in that crate forces a
/// recompilation of a very large generated library (which does not fit in
/// memory on a modest machine), for two functions whose ABI is stable and
/// tiny.
#[cfg(target_os = "windows")]
mod windows_dpapi {
    use core::ffi::c_void;

    /// `DATA_BLOB` from `wincrypt.h`: a length and a pointer to the bytes.
    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    /// `CRYPTPROTECT_UI_FORBIDDEN`: never show a UI, fail instead.
    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

    /// `BOOL`: a Win32 boolean, 32 bits wide.
    type Bool = i32;

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            p_data_in: *const DataBlob,
            sz_data_descr: *const u16,
            p_optional_entropy: *const DataBlob,
            p_reserved: *const c_void,
            p_prompt_struct: *const c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> Bool;

        fn CryptUnprotectData(
            p_data_in: *const DataBlob,
            ppsz_data_descr: *mut *mut u16,
            p_optional_entropy: *const DataBlob,
            p_reserved: *const c_void,
            p_prompt_struct: *const c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> Bool;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(mem: *mut c_void) -> *mut c_void;
    }

    /// Protects `data` with the current user's DPAPI key.
    pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
        // SAFETY: the input blob points at `data`, which outlives the call,
        // and the output blob is freed before returning.
        unsafe {
            let input = DataBlob {
                cb_data: data.len() as u32,
                pb_data: data.as_ptr() as *mut u8,
            };
            let mut output = DataBlob {
                cb_data: 0,
                pb_data: core::ptr::null_mut(),
            };

            if CryptProtectData(
                &input,
                core::ptr::null(),
                core::ptr::null(),
                core::ptr::null(),
                core::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            ) == 0
            {
                return Err("CryptProtectData failed".to_string());
            }

            let bytes = copy_blob(&output);
            free_blob(&mut output);
            Ok(bytes)
        }
    }

    /// Unprotects a blob produced by [`protect`].
    pub fn unprotect(blob: &[u8]) -> Result<Vec<u8>, String> {
        // SAFETY: the input blob points at `blob`, which outlives the call,
        // and the output blob is freed before returning.
        unsafe {
            let input = DataBlob {
                cb_data: blob.len() as u32,
                pb_data: blob.as_ptr() as *mut u8,
            };
            let mut output = DataBlob {
                cb_data: 0,
                pb_data: core::ptr::null_mut(),
            };

            if CryptUnprotectData(
                &input,
                core::ptr::null_mut(),
                core::ptr::null(),
                core::ptr::null(),
                core::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            ) == 0
            {
                return Err("CryptUnprotectData failed".to_string());
            }

            let bytes = copy_blob(&output);
            free_blob(&mut output);
            Ok(bytes)
        }
    }

    /// Copies the bytes a DPAPI output blob points at.
    unsafe fn copy_blob(blob: &DataBlob) -> Vec<u8> {
        if blob.pb_data.is_null() || blob.cb_data == 0 {
            return Vec::new();
        }
        core::slice::from_raw_parts(blob.pb_data, blob.cb_data as usize).to_vec()
    }

    /// Releases the memory DPAPI allocated for an output blob.
    unsafe fn free_blob(blob: &mut DataBlob) {
        if !blob.pb_data.is_null() {
            let _ = LocalFree(blob.pb_data as *mut c_void);
            blob.pb_data = core::ptr::null_mut();
            blob.cb_data = 0;
        }
    }
}

/// Encodes bytes as lowercase hexadecimal.
fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit((byte >> 4) as u32, 16).unwrap_or('0'));
        out.push(char::from_digit((byte & 0x0f) as u32, 16).unwrap_or('0'));
    }
    out
}

/// Decodes lowercase or uppercase hexadecimal.
fn from_hex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks(2) {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        out.push((high * 16 + low) as u8);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hexadecimal_round_trip() {
        let bytes = vec![0x00, 0x0f, 0xff, 0xa5, 0x5a];
        let hex = to_hex(&bytes);
        assert_eq!(hex, "000fffa55a");
        assert_eq!(from_hex(&hex), Some(bytes));
    }

    #[test]
    fn hexadecimal_rejects_odd_length_and_bad_digits() {
        assert_eq!(from_hex("abc"), None);
        assert_eq!(from_hex("zz"), None);
    }

    #[test]
    fn a_plain_value_is_not_reported_as_protected() {
        assert!(!is_protected("sk-plain"));
        assert!(!is_protected(""));
        assert!(is_protected("dpapi:v1:00ff"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn protects_and_unprotects_a_secret() {
        let secret = "sk-minimax-secret-value-12345";
        let protected = protect(secret).expect("protection must succeed on Windows");

        assert!(is_protected(&protected));
        assert!(
            !protected.contains(secret),
            "the protected value must not contain the plain secret"
        );
        assert_eq!(unprotect(&protected).expect("unprotection must succeed"), secret);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn an_empty_secret_stays_empty() {
        assert_eq!(protect("").expect("an empty secret is not protected"), "");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn a_plain_stored_value_is_returned_unchanged() {
        // A value written by an older version has no marker: it must still
        // load, and it is protected the next time the settings are written.
        assert_eq!(
            unprotect("sk-legacy-plain").expect("a plain value must load"),
            "sk-legacy-plain"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn a_corrupted_protected_value_fails_loudly() {
        let error = unprotect("dpapi:v1:zzzz").expect_err("corrupted data must fail");
        assert!(error.detail.contains("hexadecimal"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn two_secrets_produce_different_blobs() {
        // DPAPI uses a random salt, so the same secret never encrypts to the
        // same bytes twice.
        let first = protect("same-secret").expect("protection must succeed");
        let second = protect("same-secret").expect("protection must succeed");
        assert_ne!(first, second);
    }
}
