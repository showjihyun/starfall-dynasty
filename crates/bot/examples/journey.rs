//! Player journey QA (`bot::journey`): landing → captain → first guidance → opening → chapters →
//! closing calls → next chapter, each stage PASS/FAIL, then the first-session timeline.
//!
//!   cargo run -p bot --release --example journey [-- --no-campaign] [--hours 10] [--hull Talon --species Human]
//!
//! Exit code 1 when a stage failed.

fn parse<T: serde::de::DeserializeOwned>(name: &str) -> T {
    serde_json::from_str(&format!("\"{name}\"")).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let mut o = bot::journey::Options { campaign: !args.iter().any(|a| a == "--no-campaign"), ..Default::default() };
    if let Some(h) = arg("--hours").and_then(|v| v.parse().ok()) {
        o.max_hours = h;
    }
    if let Some(h) = arg("--hull") {
        o.hull = parse(&h);
    }
    if let Some(s) = arg("--species") {
        o.species = parse(&s);
    }
    let r = bot::journey::run(&o);
    print!("{}", r.text());
    std::process::exit(if r.failed() > 0 { 1 } else { 0 });
}
