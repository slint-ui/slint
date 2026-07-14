// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A process-global cache of the `slint!` macro's generated output.
//!
//! rust-analyzer re-expands `slint!` very frequently in a long-lived process,
//! usually with an unchanged body, and a full expansion costs tens of milliseconds
//! (it compiles the builtins, the style and any imported widgets). Caching the
//! output turns an unchanged re-expansion into a lookup plus a re-parse.
//!
//! The cached value is a `String`, not a `TokenStream`: it is shared across the
//! thread rust-analyzer spawns per expansion, and a `TokenStream` is not `Send`.
//!
//! On by default under rust-analyzer (where the live-preview output is small and
//! re-parsing is cheap); opt in elsewhere with `SLINT_MACRO_CACHE=1`.

use std::collections::VecDeque;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use i_slint_compiler::CompilerConfiguration;
use i_slint_compiler::parser::Token;

/// Maximum number of entries; editing a body creates a new key each time, so the
/// cache must be bounded.
const CAPACITY: usize = 64;

/// Whether the macro is running under rust-analyzer, detected via its unstable
/// internal environment variable. `lib.rs` uses it to select the live-preview
/// generator.
pub fn is_rust_analyzer() -> bool {
    std::env::var_os("RUST_ANALYZER_INTERNALS_DO_NOT_USE").is_some()
}

/// Whether the output cache should be used: on under rust-analyzer, opt-in
/// elsewhere with `SLINT_MACRO_CACHE=1` (off by default).
pub fn enabled() -> bool {
    is_rust_analyzer()
        || matches!(std::env::var("SLINT_MACRO_CACHE").as_deref(), Ok("1") | Ok("true"))
}

struct Entry {
    /// The full key, compared verbatim on lookup so a hash near-collision can't
    /// serve a wrong entry.
    key: String,
    /// The serialized generated token stream.
    output: String,
    /// Loaded external files with a content hash of each; the entry is invalidated
    /// once any of them no longer matches.
    deps: Vec<(PathBuf, u64)>,
}

/// Bounded, insertion-ordered set of entries (FIFO eviction). Lookup is a linear
/// scan, which is fine at this capacity.
#[derive(Default)]
struct Cache {
    entries: VecDeque<Entry>,
}

impl Cache {
    /// The cached output and deps for `key`; deps are validated by the caller after
    /// the lock is released (see `lookup`).
    fn candidate(&self, key: &str) -> Option<(String, Vec<(PathBuf, u64)>)> {
        self.entries.iter().find(|e| e.key == key).map(|e| (e.output.clone(), e.deps.clone()))
    }

    fn put(&mut self, key: String, output: String, deps: Vec<(PathBuf, u64)>) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.key == key) {
            existing.output = output;
            existing.deps = deps;
            return;
        }
        if self.entries.len() >= CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry { key, output, deps });
    }
}

/// Whether every dependency still hashes to its recorded value. An unreadable
/// file counts as changed, which invalidates the entry.
fn deps_match(deps: &[(PathBuf, u64)]) -> bool {
    deps.iter().all(|(path, expected)| content_hash(path) == Some(*expected))
}

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Cache::default()))
}

fn hash64<H: Hash>(value: &H) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// Content hash of `path`, or `None` if it can't be read (treated as changed).
fn content_hash(path: &Path) -> Option<u64> {
    std::fs::read(path).ok().map(|bytes| hash64(&bytes))
}

/// Build the cache key from everything that varies between expansions and affects
/// the output: the source path, the relevant compiler configuration, and the macro
/// body (token kind + text; spans are excluded so the same body at a different call
/// site still hits). Environment variables are constant for the life of the
/// process, so they don't belong in the key.
pub fn key_material(
    tokens: &[Token],
    config: &CompilerConfiguration,
    source_path: &Path,
) -> String {
    use std::fmt::Write;
    let mut key = String::new();
    let _ = write!(key, "src={source_path:?};");
    let _ = write!(key, "style={:?};", config.style);
    let _ = write!(key, "inc={:?};", config.include_paths);
    let mut libs: Vec<_> = config.library_paths.iter().collect();
    libs.sort_by(|a, b| a.0.cmp(b.0));
    let _ = write!(key, "lib={libs:?};");
    let _ = write!(key, "td={:?};", config.translation_domain);
    let _ = write!(key, "dtc={:?};", config.default_translation_context);
    let _ = write!(key, "exp={};", config.enable_experimental);
    let _ = write!(key, "inl={};", config.inline_all_elements);
    key.push_str("tokens=");
    for t in tokens {
        let _ = write!(key, "{:?}:{};", t.kind, t.text);
    }
    key
}

/// Test-only hit counter: a hit is otherwise indistinguishable from a fresh
/// expansion, so a test needs this to assert one actually happened.
#[cfg(test)]
static HIT_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[inline]
fn note_hit() {
    #[cfg(test)]
    HIT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Cached output for `key` if present and all its dependencies are unchanged.
/// Deps are hashed after the lock is released so concurrent expansions don't
/// serialize on disk reads.
pub fn lookup(key: &str) -> Option<String> {
    let (output, deps) = cache().lock().ok()?.candidate(key)?;
    if deps_match(&deps) {
        note_hit();
        Some(output)
    } else {
        None
    }
}

/// Cache `output` for `key`. `loaded_files` are the files the output depends on
/// (the set that also drives the `include_bytes!` reload markers); if any can't be
/// hashed the entry is dropped rather than stored without validation.
pub fn store(key: String, output: String, loaded_files: &[PathBuf]) {
    let mut deps = Vec::with_capacity(loaded_files.len());
    for path in loaded_files {
        match content_hash(path) {
            Some(h) => deps.push((path.clone(), h)),
            None => return,
        }
    }
    if let Ok(mut guard) = cache().lock() {
        guard.put(key, output, deps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit_count() -> u64 {
        HIT_COUNT.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// A real round-trip through the public `store`/`lookup` API: a stored key is
    /// served back (a hit), an absent key is not, and the hit is actually recorded.
    /// Keys are unique so the test is independent of the shared process-global cache.
    #[test]
    fn store_then_lookup_is_a_recorded_hit() {
        let key = "expansion_cache::store_then_lookup_is_a_recorded_hit";
        assert_eq!(lookup(key), None, "absent key misses");

        store(key.into(), "OUTPUT".into(), &[]);
        let before = hit_count();
        assert_eq!(lookup(key).as_deref(), Some("OUTPUT"), "stored key hits");
        assert!(hit_count() > before, "the hit is recorded");
    }

    /// A changed (or deleted) dependency must turn a would-be hit into a miss, which
    /// is what makes the cache safe when an imported `.slint` file is edited.
    #[test]
    fn changed_dependency_turns_a_hit_into_a_miss() {
        let key = "expansion_cache::changed_dependency_turns_a_hit_into_a_miss";
        let dir = std::env::temp_dir().join("slint_macro_cache_test_dep");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("dep.slint");
        std::fs::write(&file, b"export component A {}").unwrap();

        store(key.into(), "OUTPUT".into(), std::slice::from_ref(&file));
        assert_eq!(lookup(key).as_deref(), Some("OUTPUT"), "unchanged dep hits");

        std::fs::write(&file, b"export component A { width: 1px; }").unwrap();
        assert_eq!(lookup(key), None, "changed dep invalidates");

        std::fs::remove_file(&file).unwrap();
        assert_eq!(lookup(key), None, "missing dep invalidates");
    }

    #[test]
    fn put_replaces_an_existing_key() {
        let mut cache = Cache::default();
        cache.put("k".into(), "v1".into(), vec![]);
        cache.put("k".into(), "v2".into(), vec![]);
        assert_eq!(cache.candidate("k").map(|(o, _)| o).as_deref(), Some("v2"));
        assert_eq!(cache.entries.len(), 1);
    }

    #[test]
    fn put_evicts_the_oldest_when_full() {
        let mut cache = Cache::default();
        for i in 0..CAPACITY + 10 {
            cache.put(format!("k{i}"), format!("v{i}"), vec![]);
        }
        assert_eq!(cache.entries.len(), CAPACITY, "size is bounded");
        // The 10 oldest keys were evicted (FIFO); the most recent CAPACITY remain.
        assert!(cache.candidate("k0").is_none());
        assert!(cache.candidate("k9").is_none());
        assert_eq!(cache.candidate("k10").map(|(o, _)| o).as_deref(), Some("v10"));
    }
}
