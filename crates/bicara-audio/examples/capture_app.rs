//! cargo run --example capture_app -- <app-id> <seconds> <out.wav>
use std::sync::{Arc, Mutex};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (id, secs, out) = (&args[1], args[2].parse::<u64>().unwrap(), &args[3]);
    let writer: Arc<Mutex<Option<hound::WavWriter<_>>>> = Arc::new(Mutex::new(None));
    let w2 = writer.clone();
    let out2 = out.clone();
    let stats = Arc::new(Mutex::new((0usize, 0f32)));
    let s2 = stats.clone();
    let cap = bicara_audio::AppCapture::start(
        id,
        Box::new(move |samples, rate| {
            let mut w = w2.lock().unwrap();
            if w.is_none() {
                let spec = hound::WavSpec {
                    channels: 1,
                    sample_rate: rate,
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                };
                *w = Some(hound::WavWriter::create(&out2, spec).unwrap());
                eprintln!("sample rate {rate}");
            }
            let wr = w.as_mut().unwrap();
            for &s in samples {
                wr.write_sample(s).unwrap();
            }
            let (_, peak) = bicara_audio::levels(samples);
            let mut st = s2.lock().unwrap();
            st.0 += samples.len();
            st.1 = st.1.max(peak);
        }),
    )
    .expect("start capture");
    std::thread::sleep(std::time::Duration::from_secs(secs));
    cap.stop();
    if let Some(w) = writer.lock().unwrap().take() {
        w.finalize().unwrap();
    }
    let st = stats.lock().unwrap();
    println!("frames={} peak={:.4}", st.0, st.1);
}
