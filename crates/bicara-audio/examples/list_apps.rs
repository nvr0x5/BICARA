fn main() {
    for a in bicara_audio::list_audio_apps().expect("list") {
        println!(
            "{:<8} {:<40} {:<30} pids={:?} icon={}",
            if a.playing { "PLAYING" } else { "-" },
            a.id,
            a.name,
            a.pids,
            a.icon_png_base64.is_some()
        );
    }
}
