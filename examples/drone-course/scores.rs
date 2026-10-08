// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore USERPROFILE

//! The leaderboard: the ten fastest races by average speed, since every course is different.
//! It's kept in a file in the home directory, or in a browser's local storage.

const KEPT: usize = 10;

#[derive(Clone)]
pub struct Score {
    pub initials: String,
    /// Average speed over the race, in km/h.
    pub speed: f32,
    /// The race time, in seconds.
    pub time: f32,
}

pub struct Scores(Vec<Score>);

impl Scores {
    pub fn load() -> Self {
        let mut scores: Vec<Score> = read()
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                Some(Score {
                    initials: fields.next()?.to_string(),
                    speed: fields.next()?.parse().ok()?,
                    time: fields.next()?.parse().ok()?,
                })
            })
            .collect();
        scores.sort_by(|a, b| b.speed.total_cmp(&a.speed));
        scores.truncate(KEPT);
        Self(scores)
    }

    pub fn list(&self) -> &[Score] {
        &self.0
    }

    pub fn qualifies(&self, speed: f32) -> bool {
        self.0.len() < KEPT || self.0.last().is_some_and(|last| speed > last.speed)
    }

    /// Adds `score`, saves the list, and returns its place, from 0.
    pub fn insert(&mut self, score: Score) -> usize {
        let place = self.0.partition_point(|other| other.speed >= score.speed);
        self.0.insert(place, score);
        self.0.truncate(KEPT);
        let text: String = self
            .0
            .iter()
            .map(|score| format!("{} {:.2} {:.2}\n", score.initials, score.speed, score.time))
            .collect();
        write(&text);
        place
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(std::path::Path::new(&home).join(".slint-drone-course-scores"))
}

#[cfg(not(target_arch = "wasm32"))]
fn read() -> Option<String> {
    std::fs::read_to_string(path()?).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn write(text: &str) {
    if let Some(path) = path()
        && let Err(error) = std::fs::write(&path, text)
    {
        eprintln!("Couldn't save the leaderboard to {}: {error}", path.display());
    }
}

#[cfg(target_arch = "wasm32")]
const STORAGE_KEY: &str = "slint-drone-course-scores";

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
fn read() -> Option<String> {
    storage()?.get_item(STORAGE_KEY).ok()?
}

#[cfg(target_arch = "wasm32")]
fn write(text: &str) {
    if let Some(storage) = storage() {
        // A browser can refuse storage, such as in private browsing.
        storage.set_item(STORAGE_KEY, text).ok();
    }
}
