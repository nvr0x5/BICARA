//! cargo run -p bicara-llm --example ask -- <claude|codex|ollama> "<question>" [model]
use bicara_llm::*;
use std::io::Write;

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    let model = a.get(3).cloned();
    let cfg = match a[1].as_str() {
        "claude" => ProviderConfig::ClaudeCli {
            model: model.unwrap_or("haiku".into()),
            binary: None,
        },
        "codex" => ProviderConfig::CodexCli {
            model,
            binary: None,
        },
        _ => ProviderConfig::OpenAiCompatible {
            label: "Ollama".into(),
            base_url: "http://localhost:11434/v1".into(),
            model: model.unwrap_or("llama3.2".into()),
        },
    };
    let llm = Llm::new();
    let req = ChatRequest {
        system: "Jawab singkat dalam bahasa Indonesia.".into(),
        user: a[2].clone(),
        max_tokens: 300,
    };
    llm.prewarm(&cfg, &req.system).await;
    tokio::time::sleep(std::time::Duration::from_secs(4)).await; // let the warm process boot
    for round in 0..2 {
        let t0 = std::time::Instant::now();
        let mut first: Option<u128> = None;
        let out = llm
            .complete(&cfg, None, &req, &mut |d| {
                first.get_or_insert(t0.elapsed().as_millis());
                print!("{d}");
                let _ = std::io::stdout().flush();
            })
            .await;
        println!(
            "\n[round {round}] first token {:?} ms, total {} ms, ok={}",
            first,
            t0.elapsed().as_millis(),
            out.is_ok()
        );
        if let Err(e) = out {
            println!("error: {e}");
        }
    }
}
