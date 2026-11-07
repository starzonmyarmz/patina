//! Writes ui/dev/mock.json so ui/index.html runs in a plain browser with the
//! real parameter table, pages and factory bank.

use serde_json::json;
use thousand_synths::bridge::{describe_pages, describe_params};
use thousand_synths::params::Pid;
use thousand_synths::presets::factory;

fn main() {
    let presets: Vec<_> = factory()
        .iter()
        .map(|p| {
            let values = p.values();
            let normalized: Vec<f32> = Pid::ALL.iter().map(|&pid| pid.spec().normalize(values.0[pid as usize])).collect();
            let macros: Vec<&str> = p.macros.iter().map(|m| m.name.as_str()).collect();
            json!({
                "key": format!("factory/{}", p.name), "name": p.name, "category": p.category,
                "tags": p.tags, "user": false, "favorite": false, "values": normalized, "macros": macros,
            })
        })
        .collect();
    let mock = json!({ "params": describe_params(), "pages": describe_pages(), "presets": presets });
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/dev");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("mock.json"), mock.to_string()).unwrap();
    println!("wrote {}", dir.join("mock.json").display());
}
