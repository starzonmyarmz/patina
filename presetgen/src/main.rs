mod archetypes;
mod check;

use archetypes::{descriptor, Archetype, Category, Vary, ARCHETYPES, NEUTRAL};
use std::collections::HashSet;
use std::path::PathBuf;
use thousand_synths::params::{PlainValues, Values};
use thousand_synths::presets::{matrix_from, Bank, MacroDef, MacroTarget, Preset, FORMAT};

const SEED: u64 = 0x1000_5EED;
const VARIATIONS: usize = 6;

struct Rng(u64);

impl Rng {
    fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

struct Candidate {
    preset: Preset,
    values: PlainValues,
    category: Category,
}

fn base_values(a: &Archetype) -> PlainValues {
    let mut v = PlainValues::default();
    for &(pid, x) in NEUTRAL.iter().chain(a.set) {
        v.set(pid, x);
    }
    v
}

/// Returns the varied values and naming words, strongest change first.
fn vary(a: &Archetype, base: &PlainValues, rng: &mut Rng) -> (PlainValues, Vec<&'static str>) {
    let mut v = base.clone();
    for (pid, spec) in a.vary {
        let s = pid.spec();
        let x = match spec {
            Vary::Range(lo, hi) => {
                let (nlo, nhi) = (s.normalize(*lo), s.normalize(*hi));
                s.denormalize(nlo + (nhi - nlo) * rng.unit())
            }
            Vary::Pick(choices) => choices[(rng.unit() * choices.len() as f32) as usize % choices.len()],
        };
        v.set(*pid, x);
    }
    let mut moves: Vec<(f32, &'static str)> = a
        .vary
        .iter()
        .filter_map(|(pid, _)| descriptor(*pid).map(|d| (*pid, d)))
        .map(|(pid, (low, high))| {
            let s = pid.spec();
            let delta = s.normalize(v.get(pid)) - s.normalize(base.get(pid));
            (delta.abs(), if delta > 0.0 { high } else { low })
        })
        .filter(|(delta, _)| *delta > 0.02)
        .collect();
    moves.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut words: Vec<&'static str> = Vec::new();
    for (_, w) in moves {
        if !words.contains(&w) {
            words.push(w);
        }
    }
    (v, words)
}

fn macros_for(category: Category) -> Vec<MacroDef> {
    category
        .macros()
        .iter()
        .map(|m| MacroDef {
            name: m.name.to_string(),
            targets: m.targets.iter().map(|&(pid, amount)| MacroTarget { param: pid.spec().id.into(), amount }).collect(),
        })
        .collect()
}

/// Tries the strongest word, then the two strongest, then numbers.
fn unique(archetype: &str, words: &[&str], taken: &mut HashSet<String>) -> String {
    let mut options = vec![match words.first() {
        Some(w) => format!("{w} {archetype}"),
        None => format!("{archetype} Alt"),
    }];
    if let [a, b, ..] = words {
        options.push(format!("{a} {b} {archetype}"));
    }
    let numbered = (2..).map(|n| format!("{} {n}", options[0]));
    for candidate in options.clone().into_iter().chain(numbered) {
        if taken.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!()
}

fn candidates() -> Vec<Candidate> {
    let mut taken = HashSet::new();
    let mut out = Vec::new();
    for (i, a) in ARCHETYPES.iter().enumerate() {
        let base = base_values(a);
        let mut rng = Rng(SEED ^ (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let macros = macros_for(a.category);
        let mut push = |name: String, values: PlainValues, tags: Vec<String>| {
            let mut preset = Preset::capture(&name, a.category.name(), &values, &macros);
            preset.tags = tags;
            preset.author = "factory".into();
            out.push(Candidate { preset, values, category: a.category });
        };
        taken.insert(a.name.to_string());
        push(a.name.to_string(), base.clone(), vec![a.category.name().to_lowercase()]);
        for _ in 0..VARIATIONS {
            let (values, words) = vary(a, &base, &mut rng);
            let name = unique(a.name, &words, &mut taken);
            let mut tags = vec![a.category.name().to_lowercase()];
            tags.extend(words.iter().take(2).map(|w| w.to_lowercase()));
            push(name, values, tags);
        }
    }
    out
}

fn main() {
    let mut all = candidates();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let per = all.len().div_ceil(threads);
    let results: Vec<Result<f32, String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = all
            .chunks_mut(per)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter_mut()
                        .map(|c| {
                            let hold = match c.category {
                                Category::Pad | Category::Texture => 4.0,
                                _ => 1.5,
                            };
                            let matrix = matrix_from(&c.preset.macros);
                            let result = check::normalize(&mut c.values, matrix, c.category.test_notes(), hold);
                            let name = c.preset.name.clone();
                            let macros = c.preset.macros.clone();
                            let category = c.preset.category.clone();
                            let tags = std::mem::take(&mut c.preset.tags);
                            c.preset = Preset { tags, author: "factory".into(), ..Preset::capture(&name, &category, &c.values, &macros) };
                            result
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });

    let mut kept = Vec::new();
    let mut rejected = 0;
    for (c, result) in all.into_iter().zip(results) {
        match result {
            Ok(_) => kept.push(c.preset),
            Err(why) => {
                rejected += 1;
                eprintln!("rejected {}: {why}", c.preset.name);
            }
        }
    }
    for c in [Category::Bass, Category::Lead, Category::Pad, Category::Pluck, Category::Keys, Category::Bell, Category::Brass, Category::Texture] {
        println!("{:8} {}", c.name(), kept.iter().filter(|p| p.category == c.name()).count());
    }
    println!("kept {} of {}, rejected {rejected}", kept.len(), kept.len() + rejected);

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../presets/factory.json");
    let json = serde_json::to_string_pretty(&Bank { format: FORMAT, presets: kept }).unwrap();
    std::fs::write(&path, json + "\n").unwrap();
    println!("wrote {}", path.display());
}
