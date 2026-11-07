//! Writes ui/dev/mock.json so ui/index.html runs in a plain browser with the
//! real parameter table, pages and factory bank.

use serde_json::json;
use patina::bridge::{describe_pages, describe_params};
use patina::library::Library;
use patina::params::Pid;
use patina::presets::factory;

fn main() {
    let empty = std::env::temp_dir().join("patina-ui-mock");
    let order = Library::at(empty.join("User"), empty.join("state.json")).keys();
    let mut bank: Vec<_> = factory().iter().collect();
    bank.sort_by_key(|p| order.iter().position(|k| *k == format!("factory/{}", p.name)));
    let presets: Vec<_> = bank
        .into_iter()
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
