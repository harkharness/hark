//! Which whisper model this machine should be steered towards.
//!
//! whisper.cpp only gets Metal on Apple Silicon (see hark-core's Cargo.toml);
//! everywhere else it runs on CPU, where large-v3-turbo transcribes at ~3x
//! real time (measured on an Intel i5 MacBook: 14.7s for a 4.7s utterance)
//! while small stays under real time (0.6x). A "recommended" label that
//! ignores that difference steered Intel users into a mic that feels frozen.

/// The model key the onboarding should recommend for this os/arch pair.
pub fn recommended_stt_model(os: &str, arch: &str) -> &'static str {
    if os == "macos" && arch == "aarch64" {
        "large-v3-turbo"
    } else {
        "small"
    }
}

/// Why the recommendation differs from the "best" model, when it does.
/// None when the recommended model IS the strongest one (nothing to explain).
pub fn recommendation_reason(os: &str, arch: &str) -> Option<&'static str> {
    match recommended_stt_model(os, arch) {
        "small" => Some("no_metal_cpu"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_silicon_gets_the_strong_model() {
        // Metal makes large-v3-turbo fast there; accuracy wins.
        assert_eq!(recommended_stt_model("macos", "aarch64"), "large-v3-turbo");
    }

    #[test]
    fn intel_mac_gets_the_fast_model() {
        // No Metal on x86_64: turbo runs ~3x real time on CPU and the mic
        // feels frozen. small answers under real time.
        assert_eq!(recommended_stt_model("macos", "x86_64"), "small");
    }

    #[test]
    fn any_other_cpu_platform_gets_the_fast_model() {
        assert_eq!(recommended_stt_model("linux", "x86_64"), "small");
        // Linux aarch64 has no Metal either — the arch alone is not enough.
        assert_eq!(recommended_stt_model("linux", "aarch64"), "small");
    }

    #[test]
    fn only_the_downgrade_carries_a_reason() {
        assert!(recommendation_reason("macos", "aarch64").is_none());
        assert!(recommendation_reason("macos", "x86_64").is_some());
    }
}
