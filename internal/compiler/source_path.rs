// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore Bubuntu

//! The location of a .slint file or an asset.

use crate::pathutils;
use smol_str::SmolStr;
use std::path::{Path, PathBuf};

/// Where the compiler loads a .slint file or an asset from.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SourcePath {
    /// A file this host opens directly, possibly relative to the working directory.
    File(PathBuf),
    /// A file of the built-in library, such as `fluent/button.slint`.
    Builtin(SmolStr),
    /// A location that only the file loader callback can fetch,
    /// such as `https:`, `vscode-remote:`, or a `file:` URL of another OS (#13674).
    Url(ForeignUrl),
}

/// A URL that is neither a native path nor a builtin file on this host.
///
/// It's never a `builtin:` URL, and it's a `file:` URL only when that isn't a native path here,
/// such as a Windows path on Linux, or any `file:` URL on wasm.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, derive_more::Deref)]
pub struct ForeignUrl(url::Url);

impl SourcePath {
    /// Parses a native path, or a URL such as `builtin:/…`, `https:…` or `file:…`.
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        match path.to_str().and_then(pathutils::to_url) {
            Some(url) => Self::from_url(url),
            None => Self::File(pathutils::clean_path(path)),
        }
    }

    pub fn from_url(url: url::Url) -> Self {
        match url.scheme() {
            "builtin" => Self::Builtin(url.path().trim_start_matches('/').into()),
            #[cfg(not(target_arch = "wasm32"))]
            "file" => match url.to_file_path() {
                Ok(path) => Self::File(pathutils::clean_path(&path)),
                Err(()) => Self::Url(ForeignUrl(url)),
            },
            _ => Self::Url(ForeignUrl(url)),
        }
    }

    /// Returns `None` for a relative `File`.
    pub fn to_url(&self) -> Option<url::Url> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::File(path) => url::Url::from_file_path(path).ok(),
            #[cfg(target_arch = "wasm32")]
            Self::File(_) => None,
            Self::Builtin(path) => url::Url::parse(&format!("builtin:/{path}")).ok(),
            Self::Url(url) => Some(url.0.clone()),
        }
    }

    /// The inverse of [`Self::new`].
    pub fn to_path_buf(&self) -> PathBuf {
        match self {
            Self::File(path) => path.clone(),
            _ => self.to_string().into(),
        }
    }

    pub fn exists(&self) -> bool {
        self.as_native_path().is_some_and(Path::exists)
    }

    pub fn is_builtin(&self) -> bool {
        matches!(self, Self::Builtin(_))
    }

    /// Reads a `File`; anything else is `NotFound`.
    pub fn read(&self) -> std::io::Result<Vec<u8>> {
        std::fs::read(self.as_native_path().ok_or(std::io::ErrorKind::NotFound)?)
    }

    /// See [`Self::read`].
    pub fn read_to_string(&self) -> std::io::Result<String> {
        std::fs::read_to_string(self.as_native_path().ok_or(std::io::ErrorKind::NotFound)?)
    }

    pub fn as_native_path(&self) -> Option<&Path> {
        match self {
            Self::File(path) => Some(path),
            _ => None,
        }
    }

    pub fn into_native_path(self) -> Option<PathBuf> {
        match self {
            Self::File(path) => Some(path),
            _ => None,
        }
    }

    /// The directory containing `self`.
    pub fn parent(&self) -> Self {
        match self {
            Self::File(path) => Self::File(pathutils::dirname(path)),
            _ => self
                .to_url()
                .and_then(|url| url.join(".").ok())
                .map_or(self.clone(), Self::from_url),
        }
    }

    /// Resolves `relative` against `self` taken as a directory.
    /// An absolute path or URL in `relative` replaces `self`.
    pub fn join(&self, relative: &str) -> Option<Self> {
        if pathutils::is_absolute(Path::new(relative)) {
            return Some(Self::new(relative));
        }
        let mut url = match self {
            Self::File(dir) => return Some(Self::File(pathutils::join(dir, Path::new(relative)))),
            _ => self.to_url()?,
        };
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        Some(Self::from_url(url.join(&relative.replace('\\', "/")).ok()?))
    }

    pub fn file_name(&self) -> Option<&str> {
        let name = match self {
            Self::File(path) => return path.file_name()?.to_str(),
            Self::Builtin(path) => path.as_str(),
            Self::Url(url) => url.path(),
        };
        name.rsplit('/').next().filter(|name| !name.is_empty())
    }

    pub fn extension(&self) -> Option<&str> {
        let (stem, extension) = self.file_name()?.rsplit_once('.')?;
        (!stem.is_empty()).then_some(extension)
    }
}

impl Default for SourcePath {
    fn default() -> Self {
        Self::File(PathBuf::new())
    }
}

impl std::fmt::Display for SourcePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File(path) => path.display().fmt(f),
            Self::Builtin(path) => write!(f, "builtin:/{path}"),
            Self::Url(url) => url.as_str().fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn join(base: &str, relative: &str) -> String {
        SourcePath::new(base).parent().join(relative).unwrap().to_string()
    }

    #[test]
    fn kinds() {
        let builtin = SourcePath::new("builtin:///fluent/./button.slint");
        assert_eq!(builtin, SourcePath::Builtin("fluent/button.slint".into()));
        assert_eq!(builtin.to_url().unwrap().as_str(), "builtin:/fluent/button.slint");
        assert_eq!(
            SourcePath::new("C:\\ui\\main.slint"),
            SourcePath::File("C:\\ui\\main.slint".into())
        );
        for url in ["https://slint.dev/a%20b.slint", "vscode-remote://wsl%2Bubuntu/a.slint"] {
            assert_eq!(SourcePath::new(url).to_string(), url);
        }
        assert_eq!(SourcePath::new("ui/./w/../main.slint").extension(), Some("slint"));
        assert_eq!(SourcePath::new("/a/.hidden").extension(), None);
    }

    #[test]
    fn joins() {
        assert_eq!(
            join("builtin:/fluent/button.slint", "../common/x.svg"),
            "builtin:/common/x.svg"
        );
        assert_eq!(
            join("https://slint.dev/ui/main.slint", "img/a b.png"),
            "https://slint.dev/ui/img/a%20b.png"
        );
        assert_eq!(
            join("https://slint.dev/ui/main.slint", "..\\x.slint"),
            "https://slint.dev/x.slint"
        );
        assert_eq!(join("https://slint.dev/ui/main.slint", "/abs/x.slint"), "/abs/x.slint");
        assert_eq!(join("ui/main.slint", "../assets/logo.png"), "assets/logo.png");
        assert_eq!(join("main.slint", "x.slint"), "x.slint");
        assert_eq!(join("C:\\ui\\main.slint", "img/a.png"), "C:\\ui\\img\\a.png");
        let dir = SourcePath::new("https://slint.dev/lib");
        assert_eq!(dir.join("a.slint").unwrap().to_string(), "https://slint.dev/lib/a.slint");
    }

    /// The remote preview asks the editor for imports by the URL it derives back (#13674).
    #[test]
    fn file_urls_of_any_os_round_trip() {
        for s in [
            "file:///mnt/remote%20project/ui/main.slint",
            "file:///C:/Users/me/project/main.slint",
            "file://server/share/main.slint",
        ] {
            let url = url::Url::parse(s).unwrap();
            let path = SourcePath::from_url(url.clone());
            assert_eq!(path.to_url(), Some(url.clone()));
            let image = path.parent().join("images/logo.png").unwrap();
            assert_eq!(image.to_url(), url.join("images/logo.png").ok(), "{s}");
        }
    }
}
