use crate::presets::{self, Preset, Session, Source};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

/// Factory and user presets, addressed by stable keys (`factory/<name>`,
/// `user/<file stem>`), plus favorites.
pub struct Library {
    factory: &'static [Preset],
    user: Vec<(PathBuf, Preset)>,
    user_dir: PathBuf,
    state_path: PathBuf,
    state: State,
}

#[derive(Default, Serialize, Deserialize)]
struct State {
    #[serde(default)]
    favorites: BTreeSet<String>,
    #[serde(default)]
    knob_mode: Option<u8>,
}

/// Browser order for the factory categories; others follow alphabetically.
const CATEGORY_ORDER: &[&str] = &["Bass", "Lead", "Pad", "Keys", "Pluck", "Bell", "Brass", "Texture"];

fn category_rank(category: &str) -> usize {
    CATEGORY_ORDER.iter().position(|c| *c == category).unwrap_or(CATEGORY_ORDER.len())
}

impl Library {
    pub fn open() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        Self::at(presets::user_dir(), home.join("Library/Application Support/1000 Synths/state.json"))
    }

    pub fn at(user_dir: PathBuf, state_path: PathBuf) -> Self {
        let state = std::fs::read_to_string(&state_path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        let user = presets::load_dir(&user_dir);
        Self { factory: presets::factory(), user, user_dir, state_path, state }
    }

    pub fn reload(&mut self) {
        self.user = presets::load_dir(&self.user_dir);
    }

    fn user_key(path: &Path) -> String {
        format!("user/{}", path.file_stem().unwrap_or_default().to_string_lossy())
    }

    fn entries(&self) -> impl Iterator<Item = (String, &Preset, Source)> {
        let factory = self.factory.iter().map(|p| (format!("factory/{}", p.name), p, Source::Factory));
        let user = self.user.iter().map(|(path, p)| (Self::user_key(path), p, Source::User));
        factory.chain(user)
    }

    /// Every preset in browser order: by category, then name.
    fn sorted(&self) -> Vec<(String, &Preset, Source)> {
        let mut all: Vec<_> = self.entries().collect();
        all.sort_by_cached_key(|(_, p, source)| {
            (category_rank(&p.category), p.category.clone(), p.name.to_lowercase(), *source == Source::User)
        });
        all
    }

    /// Keys in browser order.
    pub fn keys(&self) -> Vec<String> {
        self.sorted().into_iter().map(|(key, ..)| key).collect()
    }

    pub fn knob_mode(&self) -> Option<u8> {
        self.state.knob_mode
    }

    pub fn set_knob_mode(&mut self, mode: u8) -> io::Result<()> {
        self.state.knob_mode = Some(mode);
        self.write_state()
    }

    pub fn get(&self, key: &str) -> Option<(&Preset, Source)> {
        self.entries().find(|(k, ..)| k == key).map(|(_, p, s)| (p, s))
    }

    /// The key of the preset a session was loaded from, if it still exists.
    pub fn key_of(&self, session: &Session) -> Option<String> {
        let key = match session.source {
            Source::Factory => format!("factory/{}", session.name),
            Source::User => format!("user/{}", presets::file_stem(&session.name)),
            Source::New => return None,
        };
        self.get(&key).is_some().then_some(key)
    }

    pub fn listing(&self) -> Vec<Value> {
        self.sorted()
            .into_iter()
            .map(|(key, p, source)| {
                json!({
                    "favorite": self.state.favorites.contains(&key),
                    "key": key,
                    "name": p.name,
                    "category": p.category,
                    "tags": p.tags,
                    "user": source == Source::User,
                })
            })
            .collect()
    }

    pub fn set_favorite(&mut self, key: &str, on: bool) -> io::Result<()> {
        if on {
            self.state.favorites.insert(key.to_string());
        } else {
            self.state.favorites.remove(key);
        }
        self.write_state()
    }

    /// Saves a user preset and returns its key.
    pub fn save(&mut self, preset: &Preset) -> io::Result<String> {
        let path = presets::save_to(&self.user_dir, preset)?;
        self.reload();
        Ok(Self::user_key(&path))
    }

    /// Deletes a user preset. Factory presets can't be deleted.
    pub fn delete(&mut self, key: &str) -> io::Result<()> {
        let Some((path, _)) = self.user.iter().find(|(path, _)| Self::user_key(path) == key) else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "no such user preset"));
        };
        std::fs::remove_file(path)?;
        self.reload();
        if self.state.favorites.remove(key) {
            self.write_state()?;
        }
        Ok(())
    }

    fn write_state(&self) -> io::Result<()> {
        if let Some(dir) = self.state_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(&self.state).map_err(io::Error::other)?;
        let tmp = self.state_path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &self.state_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::PlainValues;

    fn temp_library(tag: &str) -> (PathBuf, Library) {
        let root = std::env::temp_dir().join(format!("1000-synths-library-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let lib = Library::at(root.join("User"), root.join("state.json"));
        (root, lib)
    }

    #[test]
    fn lists_factory_presets_by_name() {
        let (root, lib) = temp_library("list");
        let first = &presets::factory()[0];
        let key = format!("factory/{}", first.name);
        assert_eq!(lib.get(&key).map(|(p, s)| (p.name.as_str(), s)), Some((first.name.as_str(), Source::Factory)));
        assert_eq!(lib.listing().len(), presets::factory().len());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn saves_finds_and_deletes_user_presets() {
        let (root, mut lib) = temp_library("save");
        let preset = Preset::capture("Glass/Choir", "Pad", &PlainValues::default(), &[]);
        let key = lib.save(&preset).unwrap();
        assert_eq!(key, "user/Glass_Choir");
        assert_eq!(lib.get(&key).map(|(p, s)| (p, s)), Some((&preset, Source::User)));
        let session = Session::from_preset(&preset, Source::User);
        assert_eq!(lib.key_of(&session).as_deref(), Some(key.as_str()));

        lib.set_favorite(&key, true).unwrap();
        assert!(lib.delete("factory/anything").is_err());
        lib.delete(&key).unwrap();
        assert!(lib.get(&key).is_none());
        assert!(lib.key_of(&session).is_none());
        assert!(!lib.state.favorites.contains(&key));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn lists_in_category_order() {
        let (root, lib) = temp_library("order");
        let categories: Vec<_> = lib.listing().iter().map(|e| e["category"].as_str().unwrap().to_string()).collect();
        let ranks: Vec<_> = categories.iter().map(|c| category_rank(c)).collect();
        assert!(ranks.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(lib.keys().len(), categories.len());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn favorites_persist() {
        let (root, mut lib) = temp_library("fav");
        let key = format!("factory/{}", presets::factory()[3].name);
        lib.set_favorite(&key, true).unwrap();
        let again = Library::at(root.join("User"), root.join("state.json"));
        let listed = again.listing().into_iter().find(|e| e["key"] == key.as_str()).unwrap();
        assert_eq!(listed["favorite"], true);
        let _ = std::fs::remove_dir_all(root);
    }
}
