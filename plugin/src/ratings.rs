//! Verdicts from audition mode, by factory preset name. The plugin writes
//! them; the preset generator reads them.

use crate::presets::Preset;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

pub const FORMAT: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Keep,
    Tweak,
    Cut,
}

impl Verdict {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "keep" => Some(Self::Keep),
            "tweak" => Some(Self::Tweak),
            "cut" => Some(Self::Cut),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Tweak => "tweak",
            Self::Cut => "cut",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rating {
    pub verdict: Verdict,
    /// The edited sound, when a tweak was rated after changing knobs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit: Option<Preset>,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ratings {
    pub format: u32,
    #[serde(default)]
    pub ratings: BTreeMap<String, Rating>,
}

impl Ratings {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(&Ratings { format: FORMAT, ratings: self.ratings.clone() })
            .map_err(io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json + "\n")?;
        std::fs::rename(&tmp, path)
    }

    pub fn get(&self, name: &str) -> Option<&Rating> {
        self.ratings.get(name)
    }

    pub fn set(&mut self, name: &str, rating: Option<Rating>) {
        match rating {
            Some(r) => self.ratings.insert(name.to_string(), r),
            None => self.ratings.remove(name),
        };
    }

    /// Adds `newer`'s verdicts, replacing ours for the same preset.
    pub fn merge(&mut self, newer: Ratings) {
        self.ratings.extend(newer.ratings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::PlainValues;

    #[test]
    fn round_trips_through_a_file() {
        let dir = std::env::temp_dir().join(format!("patina-ratings-{}", std::process::id()));
        let path = dir.join("ratings.json");
        let mut r = Ratings::default();
        r.set("Acid Bass", Some(Rating { verdict: Verdict::Cut, edit: None }));
        let edit = Preset::capture("Glass Pad", "Pad", &PlainValues::default(), &[]);
        r.set("Glass Pad", Some(Rating { verdict: Verdict::Tweak, edit: Some(edit) }));
        r.save(&path).unwrap();
        let back = Ratings::load(&path);
        assert_eq!(back.ratings, r.ratings);
        assert_eq!(back.format, FORMAT);
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(json.contains("\"verdict\": \"cut\""));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn newer_ratings_win_a_merge() {
        let mut older = Ratings::default();
        older.set("A", Some(Rating { verdict: Verdict::Keep, edit: None }));
        older.set("B", Some(Rating { verdict: Verdict::Keep, edit: None }));
        let mut newer = Ratings::default();
        newer.set("B", Some(Rating { verdict: Verdict::Cut, edit: None }));
        older.merge(newer);
        assert_eq!(older.get("A").unwrap().verdict, Verdict::Keep);
        assert_eq!(older.get("B").unwrap().verdict, Verdict::Cut);
    }
}
