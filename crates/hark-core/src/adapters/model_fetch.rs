//! Whisper model download shared by `hark setup` and the app's onboarding:
//! curl (present on every macOS) streams the file, progress is the size of
//! the partial file against a known total, and the sha256 is verified
//! before the model is adopted.
//!
//! Progress deliberately does NOT read curl's progress bar. That bar is
//! drawn for humans: it redraws with carriage returns, restarts after a
//! redirect, and a pipe read can cut a frame in half — ".3%" parses as
//! zero, which is how the wizard ended up flickering 0 → 41 → 0 → 42.

use sha2::Digest;
use std::io::Read;
use std::path::{Path, PathBuf};

/// A known-good whisper model the product offers.
pub struct WhisperModel {
    pub key: &'static str,
    pub filename: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    /// Human label for pickers ("~488 MB").
    pub size_label: &'static str,
    /// Exact size, so progress is arithmetic instead of screen-scraping.
    /// Pinned like the checksum: if upstream changes the bytes, the
    /// checksum fails anyway.
    pub size_bytes: u64,
}

const MODELS: &[WhisperModel] = &[
    WhisperModel {
        key: "small",
        filename: "ggml-small.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
        sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
        size_label: "~488 MB",
        size_bytes: 487_601_967,
    },
    WhisperModel {
        key: "large-v3-turbo",
        filename: "ggml-large-v3-turbo.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
        sha256: "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69",
        size_label: "~1.6 GB",
        size_bytes: 1_624_555_275,
    },
];

pub fn whisper_model(key: &str) -> Option<&'static WhisperModel> {
    MODELS.iter().find(|m| m.key == key)
}

pub fn whisper_models() -> &'static [WhisperModel] {
    MODELS
}

/// How far along a download is: bytes on disk over the expected total,
/// capped at 99. Reaching the last byte is not success — the checksum is,
/// and only the caller that verified it reports 100.
pub fn download_pct(done: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    let pct = done.saturating_mul(100) / total;
    pct.min(99) as u8
}

/// Stream-hash a file and compare against the expected sha256 (lowercase hex).
pub fn verify_sha256(path: &Path, expected: &str) -> anyhow::Result<bool> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()) == expected.to_lowercase())
}

/// Download a model into `dest_dir/<filename>`, reporting progress (0-100).
/// Resumes a partial download once on failure; the file only lands under its
/// final name after the checksum matches.
pub fn download_model(
    model: &WhisperModel,
    dest_dir: &Path,
    mut on_progress: impl FnMut(u8),
) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(dest_dir)?;
    let final_path = dest_dir.join(model.filename);
    if final_path.exists() && verify_sha256(&final_path, model.sha256)? {
        on_progress(100);
        return Ok(final_path);
    }
    let part = dest_dir.join(format!("{}.part", model.filename));

    let mut attempts = 0;
    loop {
        attempts += 1;
        let failure = run_curl(&part, model.url, model.size_bytes, &mut on_progress)?;
        let why = match failure {
            None if verify_sha256(&part, model.sha256)? => {
                std::fs::rename(&part, &final_path)?;
                on_progress(100);
                return Ok(final_path);
            }
            None => "checksum não confere".to_string(),
            Some(err) => err,
        };
        if attempts >= 2 {
            let _ = std::fs::remove_file(&part);
            anyhow::bail!("download do modelo falhou: {why}");
        }
    }
}

/// Run curl and report progress from the growing file. Returns None on
/// success, or curl's own complaint on failure.
fn run_curl(
    part: &Path,
    url: &str,
    total: u64,
    on_progress: &mut impl FnMut(u8),
) -> anyhow::Result<Option<String>> {
    let mut child = std::process::Command::new("curl")
        // No progress meter: stderr carries only the error, if any.
        .args(["-fSL", "--no-progress-meter", "-C", "-", "-o"])
        .arg(part)
        .arg(url)
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()?;

    let size = || std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    let mut last = 255u8;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        let pct = download_pct(size(), total);
        if pct != last {
            last = pct;
            on_progress(pct);
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    };

    let mut err = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_string(&mut err);
    }
    if status.success() {
        on_progress(download_pct(size(), total));
        return Ok(None);
    }
    let err = err.trim();
    Ok(Some(match err.is_empty() {
        true => format!("curl saiu com {status}"),
        false => err.chars().take(200).collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_the_share_of_bytes_already_on_disk() {
        let total = 487_601_967;
        assert_eq!(download_pct(0, total), 0);
        assert_eq!(download_pct(50, 100), 50);
        assert_eq!(download_pct(1, 3), 33);
        // Truncation, never rounding up: 99.9% is not "done".
        assert_eq!(download_pct(total - 1, total), 99);
        // 100 is reserved for "the checksum matched": the byte count
        // reaching the end does not mean the file is good yet.
        assert_eq!(download_pct(total, total), 99);
        assert_eq!(download_pct(total * 2, total), 99);
        // An unknown total cannot produce a percentage.
        assert_eq!(download_pct(1_000, 0), 0);
    }

    #[test]
    fn the_table_carries_exact_sizes_so_progress_needs_no_parsing() {
        assert_eq!(whisper_model("small").unwrap().size_bytes, 487_601_967);
        assert_eq!(
            whisper_model("large-v3-turbo").unwrap().size_bytes,
            1_624_555_275
        );
    }

    #[test]
    fn sha256_verifies_against_known_vectors() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty.bin");
        std::fs::write(&empty, b"").unwrap();
        // sha256 of the empty string.
        assert!(verify_sha256(
            &empty,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        )
        .unwrap());
        assert!(!verify_sha256(&empty, "deadbeef").unwrap());
    }

    #[test]
    fn model_table_knows_small_and_turbo() {
        assert_eq!(whisper_model("small").unwrap().filename, "ggml-small.bin");
        assert_eq!(
            whisper_model("large-v3-turbo").unwrap().size_label,
            "~1.6 GB"
        );
        assert!(whisper_model("mystery").is_none());
        assert_eq!(whisper_models().len(), 2);
    }
}
