//! Large-file wall time: JS CLI spawn vs in-process Rust (no pass/fail on speed).
//! SPDX-License-Identifier: MIT OR Apache-2.0
//!
//!   cargo test -p venus-sidecar --test from_doc_bench -- --ignored --nocapture

#[allow(dead_code)]
mod common;

use std::process::Command;
use std::time::Instant;

use venus_sidecar::from_doc;

use crate::common::{convert_cfg, load_fixture, LARGE_PARAGRAPH_COUNT, MIN_LARGE_MARKDOWN};

const WARMUP: usize = 3;
const TIMED: usize = 10;

fn percentile(sorted_ms: &[f64], p: f64) -> f64 {
    if sorted_ms.is_empty() {
        return 0.0;
    }
    let idx = ((p / 100.0) * (sorted_ms.len() as f64 - 1.0)).round() as usize;
    sorted_ms[idx.min(sorted_ms.len() - 1)]
}

fn summarize(mut samples: Vec<f64>) -> (f64, f64, f64) {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = percentile(&samples, 50.0);
    let p95 = percentile(&samples, 95.0);
    let max = *samples.last().unwrap();
    (median, p95, max)
}

fn machine_line() -> String {
    let arch = Command::new("uname")
        .arg("-m")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| std::env::consts::ARCH.to_string());
    let os = std::env::consts::OS;
    let cpu = if cfg!(target_os = "macos") {
        Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|t| {
            t.lines()
                .find(|l| l.starts_with("model name"))
                .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
        })
    };
    match cpu {
        Some(cpu) => format!("{os}/{arch} ({cpu})"),
        None => format!("{os}/{arch}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "wall-clock bench; run with --ignored --nocapture"]
async fn large_file_wall_time_js_and_rust() {
    let pin = load_fixture("large-home.yjs");
    let rust0 = from_doc::from_pinned_bytes(&pin).expect("rust large");
    assert!(
        rust0.markdown.len() >= MIN_LARGE_MARKDOWN,
        "bench pin markdown too small: {}",
        rust0.markdown.len()
    );
    assert_eq!(rust0.sidecar.blocks.len(), LARGE_PARAGRAPH_COUNT + 1);

    let cfg = convert_cfg();
    for _ in 0..WARMUP {
        let _ = venus_sidecar::convert::from_pinned_bytes(&pin, &cfg)
            .await
            .expect("JS warmup");
        let _ = from_doc::from_pinned_bytes(&pin).expect("rust warmup");
    }

    let mut js_ms = Vec::with_capacity(TIMED);
    for _ in 0..TIMED {
        let t0 = Instant::now();
        let js = venus_sidecar::convert::from_pinned_bytes(&pin, &cfg)
            .await
            .expect("JS timed");
        js_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(js.markdown, rust0.markdown);
    }

    let mut rust_ms = Vec::with_capacity(TIMED);
    for _ in 0..TIMED {
        let t0 = Instant::now();
        let rust = from_doc::from_pinned_bytes(&pin).expect("rust timed");
        rust_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(rust.markdown, rust0.markdown);
    }

    assert_eq!(js_ms.len(), TIMED);
    assert_eq!(rust_ms.len(), TIMED);
    let (js_med, js_p95, js_max) = summarize(js_ms);
    let (rs_med, rs_p95, rs_max) = summarize(rust_ms);
    let machine = machine_line();

    println!(
        "\nconvert-bench (warmup {WARMUP}, timed {TIMED})\n\
         machine: {machine}\n\
         pin bytes: {}\n\
         markdown bytes: {}\n\
         blocks: {}\n\
         JS-from-Rust (cold spawn counted): median={js_med:.1} p95={js_p95:.1} max={js_max:.1} ms\n\
         Pure Rust: median={rs_med:.1} p95={rs_p95:.1} max={rs_max:.1} ms\n",
        pin.len(),
        rust0.markdown.len(),
        rust0.sidecar.blocks.len()
    );
}
