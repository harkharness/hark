//! Diagnostic: capture 4s from the default input device and report the
//! peak — distinguishes "mic delivers audio" from "mic delivers silence"
//! (macOS permission denied keeps the callbacks alive with zeros).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc;

fn main() {
    let host = cpal::default_host();
    let device = host.default_input_device().expect("no input device");
    println!("device: {}", device.name().unwrap_or_default());
    let config = device.default_input_config().expect("config");
    println!("rate: {} ch: {}", config.sample_rate().0, config.channels());

    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let _ = tx.send(data.to_vec());
            },
            |err| eprintln!("audio input error: {err}"),
            None,
        )
        .expect("stream");
    stream.play().expect("play");

    let mut peak = 0.0f32;
    let mut rms_acc = 0.0f64;
    let mut n = 0usize;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    while std::time::Instant::now() < deadline {
        let Ok(chunk) = rx.recv_timeout(std::time::Duration::from_secs(2)) else {
            println!("RESULT: no audio callbacks in 2s (permission denied?)");
            return;
        };
        for s in &chunk {
            peak = peak.max(s.abs());
            rms_acc += (*s as f64) * (*s as f64);
        }
        n += chunk.len();
    }
    let rms = (rms_acc / n.max(1) as f64).sqrt();
    println!("RESULT: peak={peak:.6} rms={rms:.6} samples={n}");
    println!(
        "verdict: {}",
        if peak < 1e-4 { "SILENCE (H2)" } else { "audio OK" }
    );
}
