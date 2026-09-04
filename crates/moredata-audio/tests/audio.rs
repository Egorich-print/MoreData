use moredata_audio::{AudioBackend, NullBackend, render_wav};
use moredata_core::{CompileOptions, CompiledGraph, Project};
use moredata_runtime::Runtime;

const PROJECT: &str = include_str!("../../../tests/fixtures/sine.mdproject");

#[test]
fn null_backend_status() {
    let b = NullBackend;
    let s = b.status();
    assert_eq!(b.name(), "null");
    assert_eq!(s.backend, "null");
    assert!(!s.pipewire);
}

#[test]
fn wav_render_roundtrip() {
    let g = Project::from_json(PROJECT).unwrap().to_graph().unwrap();
    let sr = g.sample_rate;
    let (cg, st) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    let mut rt = Runtime::new(cg, st, "wav");

    let dir = std::env::temp_dir();
    let path = dir.join("moredata_wav_roundtrip_test.wav");
    let frames = render_wav(&mut rt, &path, 0.1, sr).unwrap();
    assert_eq!(frames, (0.1 * sr as f32) as u64);

    let mut reader = hound::WavReader::open(&path).unwrap();
    let spec = reader.spec();
    assert_eq!(spec.sample_rate, sr);
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.bits_per_sample, 16);

    let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
    assert_eq!(samples.len(), frames as usize);
    let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
    // i16 full scale is 32767; expect the sine peak (~0.2–0.26 pre-quantization)
    assert!(
        peak > (0.19 * 32767.0) as u16 && peak < (0.27 * 32767.0) as u16,
        "peak={peak}"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn wav_render_zero_seconds() {
    let g = Project::from_json(PROJECT).unwrap().to_graph().unwrap();
    let (cg, st) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    let mut rt = Runtime::new(cg, st, "wav");
    let dir = std::env::temp_dir();
    let path = dir.join("moredata_wav_zero_test.wav");
    let frames = render_wav(&mut rt, &path, 0.0, 48000).unwrap();
    assert_eq!(frames, 0);
    std::fs::remove_file(&path).ok();
}
