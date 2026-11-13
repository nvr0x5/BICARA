//! cargo run -p bicara-llm --example probe -- [haiku|sonnet|opus]
use bicara_llm::*;

#[tokio::main]
async fn main() {
    let llm = Llm::new();
    for m in std::env::args().skip(1) {
        let cfg = ProviderConfig::ClaudeCli { model: m.clone(), binary: None };
        match llm.test(&cfg, None).await {
            Ok(r) => println!("{m:7} -> {:?}  reply={:?}  {} ms", r.model, r.reply, r.ms),
            Err(e) => println!("{m:7} -> error: {e}"),
        }
    }
}
