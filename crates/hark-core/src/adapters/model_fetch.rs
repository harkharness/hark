//! Whisper model download shared by `hark setup` and the app's onboarding:
//! curl (present on every macOS) streams the file while we parse its
//! progress bar, then the sha256 is verified before the model is adopted.

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
}

const MODELS: &[WhisperModel] = &[
    WhisperModel {
        key: "small",
        filename: "ggml-small.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
        sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
        size_label: "~488 MB",
    },
    WhisperModel {
        key: "large-v3-turbo",
        filename: "ggml-large-v3-turbo.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
        sha256: "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69",
        size_label: "~1.6 GB",
    },
];

pub fn whisper_model(key: &str) -> Option<&'static WhisperModel> {
    MODELS.iter().find(|m| m.key == key)
}

pub fn whisper_models() -> &'static [WhisperModel] {
    MODELS
}

/// Extract the latest percentage from a chunk of curl `--progress-bar`
/// stderr output (carriage-return separated `###  45.3%` frames).
pub fn parse_curl_progress(chunk: &str) -> Option<u8> {
    chunk
        .rsplit(['\r', '\n'])
        .find_map(|frame| {
            let pct = frame.trim().strip_suffix('%')?;
            let value: f64 = pct.rsplit(' ').next()?.trim().parse().ok()?;
            Some(value.clamp(0.0, 100.0) as u8)
        })
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
        let status = run_curl(&part, model.url, &mut on_progress)?;
        if status && verify_sha256(&part, model.sha256)? {
            std::fs::rename(&part, &final_path)?;
            on_progress(100);
            return Ok(final_path);
        }
        if attempts >= 2 {
            let _ = std::fs::remove_file(&part);
            anyhow::bail!("download do modelo falhou (rede ou checksum); tente de novo");
        }
    }
}

fn run_curl(
    part: &Path,
    url: &str,
    on_progress: &mut impl FnMut(u8),
) -> anyhow::Result<bool> {
    let mut child = std::process::Command::new("curl")
        .args(["-fSL", "--progress-bar", "-C", "-", "-o"])
        .arg(part)
        .arg(url)
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()?;

    if let Some(mut stderr) = child.stderr.take() {
        let mut buf = [0u8; 4096];
        while let Ok(n) = stderr.read(&mut buf) {
            if n == 0 {
                break;
            }
            if let Some(pct) = parse_curl_progress(&String::from_utf8_lossy(&buf[..n])) {
                on_progress(pct);
            }
        }
    }
    Ok(child.wait()?.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_parses_the_latest_frame_and_ignores_noise() {
        assert_eq!(parse_curl_progress("###   12.4%\r#####  45.3%"), Some(45));
        assert_eq!(parse_curl_progress("######################## 100.0%"), Some(100));
        assert_eq!(parse_curl_progress("curl: (56) recv failure"), None);
        assert_eq!(parse_curl_progress(""), None);
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
