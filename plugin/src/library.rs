use crate::presets::{self, Preset, Session, Source};
use crate::ratings::{Rating, Ratings};
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
    ratings: Ratings,
}

#[derive(Default, Serialize, Deserialize)]
struct State {
    #[serde(default)]
    favorites: BTreeSet<String>,
    #[serde(default)]
    knob_mode: Option<u8>,
}

/// Browser order for the factory categories; others follow alphabetically.
pub const CATEGORY_ORDER: &[&str] = &["Bass", "Lead", "Pad", "Keys", "Pluck", "Bell", "Brass", "Texture"];

#[cfg(not(windows))]
pub fn support_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join("Library/Application Support/Patina")
}

#[cfg(windows)]
pub fn support_dir() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default().join("Patina")
}

/// Where the plugin saves audition verdicts.
pub fn ratings_path() -> PathBuf {
    support_dir().join("ratings.json")
}

fn category_rank(category: &str) -> usize {
    CATEGORY_ORDER.iter().position(|c| *c == category).unwrap_or(CATEGORY_ORDER.len())
}

impl Library {
    pub fn open() -> Self {
        Self::at(presets::user_dir(), support_dir().join("state.json"))
    }

    pub fn at(user_dir: PathBuf, state_path: PathBuf) -> Self {
        let state = std::fs::read_to_string(&state_path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        let user = presets::load_dir(&user_dir);
        let ratings = Ratings::load(&state_path.with_file_name("ratings.json"));
        Self { factory: presets::factory(), user, user_dir, state_path, state, ratings }
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

    /// The browser's filters, in its order: all, favorites, user, unrated
    /// (in audition mode), then each category.
    pub fn filters(&self, audition: bool) -> Vec<String> {
        let mut out: Vec<String> = ["all", "favorites", "user"].map(String::from).into();
        if audition {
            out.push("unrated".into());
        }
        for (_, p, _) in self.sorted() {
            if !out[3..].contains(&p.category) {
                out.push(p.category.clone());
            }
        }
        out
    }

    /// The keys `filter` shows, in browser order.
    pub fn filtered(&self, filter: &str) -> Vec<String> {
        self.sorted()
            .into_iter()
            .filter(|(key, p, source)| match filter {
                "all" => true,
                "favorites" => self.state.favorites.contains(key),
                "user" => *source == Source::User,
                "unrated" => *source != Source::User && self.rating(key).is_none(),
                category => p.category == category,
            })
            .map(|(key, ..)| key)
            .collect()
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
                    "rating": self.rating(&key).map(|r| r.verdict.name()),
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

    pub fn ratings_path(&self) -> PathBuf {
        self.state_path.with_file_name("ratings.json")
    }

    /// Only factory presets are rated.
    pub fn rating(&self, key: &str) -> Option<&Rating> {
        self.ratings.get(key.strip_prefix("factory/")?)
    }

    pub fn rate(&mut self, key: &str, rating: Option<Rating>) -> io::Result<()> {
        let Some(name) = key.strip_prefix("factory/").filter(|_| self.get(key).is_some()) else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "only factory presets are rated"));
        };
        self.ratings.set(name, rating);
        self.ratings.save(&self.ratings_path())
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
        let root = std::env::temp_dir().join(format!("patina-library-{tag}-{}", std::process::id()));
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
    fn rates_factory_presets_only() {
        use crate::ratings::Verdict;
        let (root, mut lib) = temp_library("rate");
        let key = lib.keys()[0].clone();
        lib.rate(&key, Some(Rating { verdict: Verdict::Cut, edit: None })).unwrap();
        let again = Library::at(root.join("User"), root.join("state.json"));
        assert_eq!(again.rating(&key).map(|r| r.verdict), Some(Verdict::Cut));
        let listed = again.listing().into_iter().find(|e| e["key"] == key.as_str()).unwrap();
        assert_eq!(listed["rating"], "cut");
        assert!(lib.rate("user/Mine", None).is_err());
        assert!(lib.rate("factory/No such preset", None).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn filters_match_the_browser() {
        let (root, mut lib) = temp_library("filters");
        let filters = lib.filters(false);
        assert_eq!(filters[..4], ["all", "favorites", "user", "Bass"]);
        assert!(!filters.contains(&"unrated".to_string()));
        assert_eq!(lib.filters(true)[3], "unrated");
        assert_eq!(lib.filtered("all"), lib.keys());
        assert!(lib.filtered("favorites").is_empty());
        let key = lib.filtered("Pad")[2].clone();
        lib.set_favorite(&key, true).unwrap();
        assert_eq!(lib.filtered("favorites"), [key]);
        assert!(lib.filtered("Pad").iter().all(|k| lib.get(k).unwrap().0.category == "Pad"));
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
