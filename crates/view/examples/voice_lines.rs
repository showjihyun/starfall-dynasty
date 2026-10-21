//! Prints, as JSON, every line the opening and the comms calls speak: `[{"voice", "text"}, …]`.
//! `python tools/voice.py` reads it to know what to generate (the RON files stay the only source).

fn main() {
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/../client/assets/cinematics");
    let read = |f: &str| std::fs::read_to_string(format!("{assets}/{f}")).unwrap_or_else(|e| panic!("{f}: {e}"));
    let opening = view::cinematic::Cinematic::parse(&read("opening.ron")).expect("opening.ron");
    let comms = view::comms::Comms::parse(&read("comms.ron")).expect("comms.ron");
    println!("{}", serde_json::to_string_pretty(&view::voice::wanted(&opening, &comms)).unwrap());
}
