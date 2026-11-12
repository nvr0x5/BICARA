//! Feed a WAV through the live pipeline in 20 ms chunks (as if it were live audio).
//! cargo run --release -p bicara-stt --example transcribe -- <model.bin> <file.wav>
use std::sync::mpsc::channel;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut r = hound::WavReader::open(&a[2]).unwrap();
    let spec = r.spec();
    let ch = spec.channels as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => r
            .samples::<i32>()
            .map(|s| s.unwrap() as f32 / (1i64 << (spec.bits_per_sample - 1)) as f32)
            .collect(),
    };
    let mono: Vec<f32> = raw
        .chunks(ch)
        .map(|f| f.iter().sum::<f32>() / ch as f32)
        .collect();
    let (tx, rx) = channel();
    let t0 = std::time::Instant::now();
    let stt =
        bicara_stt::LiveTranscriber::new(&a[1], Default::default(), move |u| tx.send(u).unwrap())
            .unwrap();
    eprintln!("model loaded in {} ms", t0.elapsed().as_millis());
    let mut f = stt.feeder("app");
    for c in mono.chunks((spec.sample_rate / 50) as usize) {
        f.push(c, spec.sample_rate);
    }
    f.finish();
    drop(f);
    drop(stt); // waits for the queue to drain
    for u in rx.iter() {
        println!(
            "[{:6.2}-{:6.2}] ({} ms) {}",
            u.start, u.end, u.latency_ms, u.text
        );
    }
}
