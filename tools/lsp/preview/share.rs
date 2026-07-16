// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Share the previewed design to SlintPad.
//!
//! The current design (all the `.slint` sources the preview resolved) is uploaded to a
//! secret GitHub gist, and SlintPad is opened on it. SlintPad already knows how to render a
//! multi-file design from a gist, so nothing needs to change on the SlintPad side. See
//! `tools/slintpad/src/github.ts` for the reading side.
//!
//! Sharing reads the design off the local filesystem and uploads it over the network, neither
//! of which the WASM preview can do, so the whole module is native only (see `mod share` in
//! `preview.rs`) and needs no per-item `#[cfg]`.
//!
//! This is a prototype: it authenticates with a hand-entered GitHub token (entered on every
//! share, not persisted), uploads the `.slint` sources and embeds every referenced asset as a
//! `data:` URL. The gist upload is isolated in the `create_gist` functions, so a future
//! SlintAccount backend can replace it without touching the collection, rebasing, or UI code.

use super::PREVIEW_STATE;
use i_slint_live_preview::make_data_url;
use slint::{ModelRc, SharedString, VecModel};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// Where the resulting link points once uploaded.
const SLINTPAD_URL: &str = "https://slintpad.com";

/// The design collected from the preview, ready to be uploaded.
struct CollectedShare {
    /// Relative path of the entry file (`/`-separated), e.g. `main.slint`.
    entry: String,
    /// Source files keyed by relative path (`/`-separated) with their contents.
    sources: BTreeMap<String, String>,
    /// Assets keyed by relative path, pointing at their file on disk. They are read and inlined
    /// as `data:` URLs only at upload time, so opening the dialog stays cheap.
    assets: BTreeMap<String, PathBuf>,
}

impl CollectedShare {
    /// The design uses subdirectories, which gists can not represent directly.
    fn is_nested(&self) -> bool {
        self.sources.keys().any(|p| p.contains('/'))
    }

    /// The list shown in the consent dialog: everything that leaves the machine.
    fn consent_list(&self) -> Vec<SharedString> {
        self.sources.keys().chain(self.assets.keys()).map(SharedString::from).collect()
    }
}

/// Read an asset and inline it as a `data:` URL, or `None` if it can not be read.
fn asset_data_url(path: &Path) -> Option<String> {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    Some(make_data_url(extension, &std::fs::read(path).ok()?))
}

/// Resolve symlinks and normalize; fall back to the input when the file is not on disk yet
/// (an unsaved buffer), so freshly-created files still get a stable absolute path.
fn real_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Longest common ancestor *directory* of the given absolute file paths.
///
/// Returns `None` when the files share no directory at all — different Windows drives, or a
/// set so spread out that only the filesystem root is common (both a privacy problem and a
/// broken share layout).
fn common_ancestor(paths: &[PathBuf]) -> Option<PathBuf> {
    let mut common: Option<Vec<Component>> = None;
    for path in paths {
        let dir: Vec<Component> = path.parent()?.components().collect();
        common = Some(match common {
            None => dir,
            Some(prev) => {
                let n = prev.iter().zip(&dir).take_while(|(a, b)| a == b).count();
                prev[..n].to_vec()
            }
        });
        if common.as_ref().is_some_and(|c| c.is_empty()) {
            return None;
        }
    }
    let common = common?;
    let mut root = PathBuf::new();
    root.extend(common.iter().map(|c| c.as_os_str()));
    // A root with no parent is a filesystem root (`/`, `C:\`): too broad to share from.
    root.parent()?;
    Some(root)
}

/// Path of `file` relative to `root`, using `/` separators for transport.
fn relative(root: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(root).ok()?;
    let parts: Option<Vec<&str>> = rel
        .components()
        .map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();
    Some(parts?.join("/"))
}

/// Gather the previewed design from the preview state and rebase every path onto a common
/// root. Fails loudly (returning a message naming the offending file) rather than silently
/// rewriting a path or uploading from the filesystem root.
fn collect(preview_state: &super::PreviewState) -> Result<CollectedShare, String> {
    let entry = preview_state
        .current_component()
        .ok_or_else(|| "No component is currently previewed.".to_string())?;

    let entry_path = entry
        .url
        .to_file_path()
        .map(|p| real_path(&p))
        .map_err(|_| "The previewed component is not a local file.".to_string())?;

    // Sources: the `.slint` files the preview actually resolved (skip `builtin:` etc.).
    let mut sources: Vec<(PathBuf, String)> = Vec::new();
    for dep in &preview_state.dependencies {
        if dep.scheme() != "file" {
            continue;
        }
        let path = dep.to_file_path().map_err(|_| format!("Not a local file: {dep}"))?;
        let code = preview_state
            .source_code
            .get(dep)
            .map(|e| e.code.clone())
            .or_else(|| std::fs::read_to_string(&path).ok())
            .ok_or_else(|| format!("Could not read {}", path.display()))?;
        sources.push((real_path(&path), code));
    }
    if sources.iter().all(|(p, _)| p != &entry_path) {
        return Err("The previewed file is not among the resolved sources.".to_string());
    }

    let assets: Vec<PathBuf> = preview_state
        .resources
        .iter()
        .filter(|u| u.scheme() == "file")
        .filter_map(|u| u.to_file_path().ok())
        .map(|p| real_path(&p))
        .collect();

    // Rebase onto the common ancestor of every file, sources and assets alike.
    let all_paths: Vec<PathBuf> =
        sources.iter().map(|(p, _)| p.clone()).chain(assets.iter().cloned()).collect();
    let root = common_ancestor(&all_paths).ok_or_else(|| {
        "The files span unrelated locations (no shared directory); refusing to share.".to_string()
    })?;

    let rebase = |p: &Path| -> Result<String, String> {
        relative(&root, p).ok_or_else(|| {
            format!("{} lies outside the shared root {}", p.display(), root.display())
        })
    };

    let entry_rel = rebase(&entry_path)?;
    let mut source_map: BTreeMap<String, String> = BTreeMap::new();
    for (path, code) in &sources {
        source_map.insert(rebase(path)?, code.clone());
    }

    // Case / normalization collisions become distinct keys here but collide on a
    // case-insensitive server filesystem. Detect and refuse rather than lose a file.
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for key in source_map.keys() {
        if let Some(other) = seen.insert(key.to_lowercase(), key.clone()) {
            return Err(format!("Files '{other}' and '{key}' collide when case is ignored."));
        }
    }

    // Every asset is shared; it is read and inlined as a `data:` URL at upload time.
    let mut asset_map: BTreeMap<String, PathBuf> = BTreeMap::new();
    for path in &assets {
        if let Ok(rel) = rebase(path) {
            asset_map.insert(rel, path.clone());
        }
    }

    tracing::debug!("Share: rebased design onto {}", root.display());

    Ok(CollectedShare { entry: entry_rel, sources: source_map, assets: asset_map })
}

/// Callback for `Api.share-open`: gather the file list and reveal the dialog.
pub fn share_open() {
    PREVIEW_STATE.with_borrow(|preview_state| {
        let Some(api) = preview_state.api.upgrade() else {
            return;
        };
        match collect(preview_state) {
            Ok(share) => {
                let files = ModelRc::new(VecModel::from(share.consent_list()));
                api.set_share_files(files);
                api.set_share_status(SharedString::new());
            }
            Err(err) => {
                api.set_share_files(ModelRc::new(VecModel::from(Vec::<SharedString>::new())));
                api.set_share_status(err.into());
            }
        }
        api.set_share_result_url(SharedString::new());
        api.set_share_in_progress(false);
        api.set_share_dialog_visible(true);
    });
}

/// Callback for `Api.share-perform`: upload the design and open SlintPad on it.
///
/// `token` is the GitHub token the user typed in the dialog. This prototype does not persist
/// it, so it is entered on every share.
pub fn share_perform(token: SharedString) {
    let share = match PREVIEW_STATE.with_borrow(collect) {
        Ok(share) => share,
        Err(err) => return set_status(err),
    };

    let token = token.trim().to_string();
    if token.is_empty() {
        return set_status("Enter a GitHub token with the 'gist' scope.".to_string());
    }

    upload(share, token);
}

/// Run `f` against the preview `Api` on the event loop thread, from anywhere.
fn update_ui(f: impl FnOnce(&super::ui::Api<'static>) + Send + 'static) {
    let _ = i_slint_core::api::invoke_from_event_loop(move || {
        PREVIEW_STATE.with_borrow(|preview_state| {
            if let Some(api) = preview_state.api.upgrade() {
                f(&api);
            }
        });
    });
}

/// Show a final status message and stop the progress indicator.
fn set_status(text: String) {
    update_ui(move |api| {
        api.set_share_in_progress(false);
        api.set_share_status(text.into());
    });
}

fn upload(share: CollectedShare, token: String) {
    update_ui(|api| {
        api.set_share_in_progress(true);
        api.set_share_status("Uploading to GitHub…".into());
    });

    std::thread::spawn(move || match create_gist(&token, &share) {
        Ok(gist_url) => {
            let load_url = slintpad_url(&gist_url);
            update_ui(move |api| {
                api.set_share_in_progress(false);
                api.set_share_status("Shared.".into());
                api.set_share_result_url(load_url.into());
            });
        }
        Err(GistError::Unauthorized) => {
            set_status("The GitHub token was rejected. Enter a valid token.".to_string());
        }
        Err(err) => set_status(format!("Sharing failed: {err}")),
    });
}

/// The SlintPad URL that renders the gist, with `gist_url` correctly encoded as a query value.
fn slintpad_url(gist_url: &str) -> String {
    let mut url = lsp_types::Url::parse(SLINTPAD_URL).expect("SLINTPAD_URL is a valid URL");
    url.query_pairs_mut().append_pair("load_url", gist_url);
    url.to_string()
}

enum GistError {
    Unauthorized,
    Http(u16, String),
    Network(String),
    NoUrl,
}

impl std::fmt::Display for GistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GistError::Unauthorized => write!(f, "the GitHub token was rejected"),
            GistError::Http(code, msg) => write!(f, "GitHub returned {code}: {msg}"),
            GistError::Network(msg) => write!(f, "{msg}"),
            GistError::NoUrl => write!(f, "GitHub did not return a gist URL"),
        }
    }
}

const GISTS_URL: &str = "https://api.github.com/gists";

/// A reqwest client for the gist API. Built once and reused across a share's requests.
fn gist_client() -> Result<reqwest::blocking::Client, GistError> {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("slint-lsp/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| GistError::Network(e.to_string()))
}

/// Body of a "create gist" request: the files plus a `slint.json` naming the entry and its
/// `mappings`.
fn create_body(
    entry: &str,
    files: serde_json::Map<String, serde_json::Value>,
    mappings: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut files = files;
    let manifest = serde_json::json!({
        "main": entry,
        "mappings": mappings,
        "slint_version": env!("CARGO_PKG_VERSION"),
    });
    files.insert("slint.json".into(), serde_json::json!({ "content": manifest.to_string() }));
    serde_json::json!({
        "description": format!("Slint design shared from the editor (main file is: \"{entry}\")"),
        "public": false,
        "files": files,
    })
}

/// The shareable assets read and inlined as `data:` URL mappings.
fn asset_mappings(share: &CollectedShare) -> serde_json::Map<String, serde_json::Value> {
    share
        .assets
        .iter()
        .filter_map(|(rel, path)| Some((rel.clone(), asset_data_url(path)?.into())))
        .collect()
}

/// Create a secret gist for the design and return the gist's `html_url` (which SlintPad can
/// load via `?load_url=`). SlintPad's `_process_gist_url` reads `main`/`mappings` from
/// `slint.json` and resolves the rest of the files.
fn create_gist(token: &str, share: &CollectedShare) -> Result<String, GistError> {
    if share.is_nested() {
        return create_nested_gist(token, share);
    }

    // Flat: every file is a sibling, so gist filenames are the relative paths as-is and the
    // sibling `.slint` files map themselves. Only the assets need `mappings`. Single request.
    let files = share
        .sources
        .iter()
        .map(|(name, content)| (name.clone(), serde_json::json!({ "content": content })))
        .collect();
    let body = create_body(&share.entry, files, asset_mappings(share));
    html_url(&send_gist(&gist_client()?, reqwest::Method::POST, GISTS_URL, token, &body)?)
}

/// Gists can not hold subdirectories, so store each nested file under a flat, mangled name and
/// add a `slint.json` `mappings` entry from the real relative path to that file's raw URL.
/// The raw URLs are only known after creation, so this is a create-then-`PATCH`.
fn create_nested_gist(token: &str, share: &CollectedShare) -> Result<String, GistError> {
    let client = gist_client()?;

    // rel path -> mangled gist filename (unique by construction).
    let mangled: BTreeMap<String, String> = share
        .sources
        .keys()
        .enumerate()
        .map(|(i, rel)| {
            let base = rel.split('/').next_back().unwrap_or(rel);
            (rel.clone(), format!("{i}~{base}"))
        })
        .collect();

    let files = share
        .sources
        .iter()
        .map(|(rel, content)| (mangled[rel].clone(), serde_json::json!({ "content": content })))
        .collect();

    let create = create_body(&share.entry, files, serde_json::Map::new());
    let created = send_gist(&client, reqwest::Method::POST, GISTS_URL, token, &create)?;

    // Map each real relative path to the raw URL GitHub assigned its mangled file, then let the
    // assets ride along as inline `data:` URLs.
    let created_files = created.get("files").and_then(|v| v.as_object()).ok_or(GistError::NoUrl)?;
    let mut mappings = asset_mappings(share);
    for (rel, name) in &mangled {
        let raw_url = created_files
            .get(name)
            .and_then(|f| f.get("raw_url"))
            .and_then(|v| v.as_str())
            .ok_or(GistError::NoUrl)?;
        mappings.insert(rel.clone(), serde_json::json!(raw_url));
    }

    let gist_id = created.get("id").and_then(|v| v.as_str()).ok_or(GistError::NoUrl)?;
    let manifest = serde_json::json!({
        "main": share.entry,
        "mappings": mappings,
        "slint_version": env!("CARGO_PKG_VERSION"),
    });
    let patch = serde_json::json!({
        "files": { "slint.json": { "content": manifest.to_string() } },
    });
    let url = format!("{GISTS_URL}/{gist_id}");
    send_gist(&client, reqwest::Method::PATCH, &url, token, &patch)?;

    html_url(&created)
}

/// Send an authenticated request to the gist API and return the parsed JSON response.
fn send_gist(
    client: &reqwest::blocking::Client,
    method: reqwest::Method,
    url: &str,
    token: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, GistError> {
    let response = client
        .request(method, url)
        .header("Accept", "application/vnd.github+json")
        .bearer_auth(token)
        .json(body)
        .send()
        .map_err(|e| GistError::Network(e.to_string()))?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(GistError::Unauthorized);
    }
    if !status.is_success() {
        return Err(GistError::Http(status.as_u16(), response.text().unwrap_or_default()));
    }
    response.json().map_err(|e| GistError::Network(e.to_string()))
}

fn html_url(value: &serde_json::Value) -> Result<String, GistError> {
    value.get("html_url").and_then(|v| v.as_str()).map(str::to_string).ok_or(GistError::NoUrl)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn common_ancestor_single_file() {
        let root = common_ancestor(&[p("/home/u/proj/main.slint")]).unwrap();
        assert_eq!(root, p("/home/u/proj"));
    }

    #[test]
    fn common_ancestor_nested() {
        let root = common_ancestor(&[
            p("/home/u/proj/main.slint"),
            p("/home/u/proj/widgets/button.slint"),
        ])
        .unwrap();
        assert_eq!(root, p("/home/u/proj"));
    }

    #[test]
    fn common_ancestor_rejects_filesystem_root() {
        // Only `/` is common: too broad, must be refused.
        assert!(common_ancestor(&[p("/a/x.slint"), p("/b/y.slint")]).is_none());
    }

    #[test]
    fn relative_joins_with_forward_slash() {
        let root = p("/home/u/proj");
        assert_eq!(relative(&root, &p("/home/u/proj/main.slint")).unwrap(), "main.slint");
        assert_eq!(
            relative(&root, &p("/home/u/proj/widgets/button.slint")).unwrap(),
            "widgets/button.slint"
        );
    }

    #[test]
    fn slintpad_url_encodes_the_gist() {
        assert_eq!(
            slintpad_url("https://gist.github.com/u/abc"),
            "https://slintpad.com/?load_url=https%3A%2F%2Fgist.github.com%2Fu%2Fabc"
        );
    }
}
