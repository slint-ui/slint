// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore Bubuntu

//! The location of a .slint file or an asset.

use smol_str::SmolStr;
use std::path::{Component, Path, PathBuf};

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
        match path.to_str().and_then(parse_url) {
            Some(url) => url.into(),
            None => Self::File(clean_path(path)),
        }
    }

    /// Clones `url` only for a `Url` result; `From<url::Url>` moves it instead.
    pub fn from_url(url: &url::Url) -> Self {
        Self::from_file_url(url).unwrap_or_else(|| Self::Url(ForeignUrl(url.clone())))
    }

    /// The `File` or `Builtin` that `url` names, if any.
    fn from_file_url(url: &url::Url) -> Option<Self> {
        match url.scheme() {
            "builtin" => Some(Self::Builtin(url.path().trim_start_matches('/').into())),
            #[cfg(not(target_arch = "wasm32"))]
            "file" => Some(Self::File(clean_path(&url.to_file_path().ok()?))),
            _ => None,
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

    /// The inverse of [`Self::new`], with a URL stored as its string.
    #[deprecated(note = "A URL isn't a path: only for public APIs that take a `Path`")]
    pub fn to_legacy_path(&self) -> PathBuf {
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

    /// Reads a `File` or a `Builtin`; a `Url` is `NotFound`.
    pub fn read(&self) -> std::io::Result<std::borrow::Cow<'static, [u8]>> {
        match self {
            Self::File(path) => std::fs::read(path).map(Into::into),
            Self::Builtin(path) => crate::fileaccess::builtin_contents(path)
                .map(Into::into)
                .ok_or_else(|| std::io::ErrorKind::NotFound.into()),
            Self::Url(_) => Err(std::io::ErrorKind::NotFound.into()),
        }
    }

    /// See [`Self::read`].
    pub fn read_to_string(&self) -> std::io::Result<String> {
        String::from_utf8(self.read()?.into_owned())
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
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

    pub fn parent(&self) -> Self {
        match self {
            Self::File(path) => Self::File(path.parent().unwrap_or(path).to_owned()),
            _ => self.to_url().and_then(|url| url.join(".").ok()).map_or(self.clone(), Self::from),
        }
    }

    /// Resolves `relative` against `self` taken as a directory.
    /// An absolute path or URL in `relative` replaces `self`.
    ///
    /// `relative` follows the import syntax, where `\` separates directories on every host.
    pub fn join(&self, relative: &str) -> Option<Self> {
        let relative = relative.replace('\\', "/");
        if is_absolute(&relative) {
            return Some(Self::new(relative));
        }
        if let Self::File(dir) = self {
            return Some(Self::File(clean_path(&dir.join(relative))));
        }
        let mut url = self.to_url()?;
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        Some(url.join(&relative).ok()?.into())
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

/// Whether `path` is a URL or a path from the root on any host, such as `/a`, `\\a` or `C:/a`.
pub fn is_absolute(path: &str) -> bool {
    let windows_root = match path.as_bytes() {
        [b'\\', ..] => true,
        [drive, b':', b'/' | b'\\', ..] => drive.is_ascii_alphabetic(),
        _ => false,
    };
    windows_root || parse_url(path).is_some() || Path::new(path).has_root()
}

/// A single-character scheme is a Windows drive letter (`c:\\...`), i.e. a path rather than a URL.
fn parse_url(path: &str) -> Option<url::Url> {
    url::Url::parse(path).ok().filter(|url| url.scheme().len() > 1)
}

/// Removes the `.` and `..` components of `path` without looking at the file system.
pub fn clean_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match result.components().next_back() {
                Some(Component::Normal(_)) => {
                    result.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => result.push(".."),
            },
            component => result.push(component),
        }
    }
    result
}

impl From<url::Url> for SourcePath {
    fn from(url: url::Url) -> Self {
        Self::from_file_url(&url).unwrap_or(Self::Url(ForeignUrl(url)))
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
    fn join(base: &str, relative: &str) -> SourcePath {
        SourcePath::new(base).parent().join(relative).unwrap()
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
            SourcePath::new("builtin:/common/x.svg")
        );
        assert_eq!(
            join("https://slint.dev/ui/main.slint", "img/a b.png"),
            SourcePath::new("https://slint.dev/ui/img/a%20b.png")
        );
        assert_eq!(
            join("https://slint.dev/ui/main.slint", "..\\x.slint"),
            SourcePath::new("https://slint.dev/x.slint")
        );
        assert_eq!(
            join("https://slint.dev/ui/main.slint", "/abs/x.slint"),
            SourcePath::new("/abs/x.slint")
        );
        assert_eq!(join("ui/main.slint", "../assets/logo.png"), SourcePath::new("assets/logo.png"));
        assert_eq!(join("main.slint", "x.slint"), SourcePath::new("x.slint"));
        let dir = SourcePath::new("https://slint.dev/lib");
        assert_eq!(dir.join("a.slint").unwrap().to_string(), "https://slint.dev/lib/a.slint");
    }

    #[test]
    fn classification() {
        for (path, absolute) in [
            ("https://foo.bar/", true),
            ("builtin:/foo", true),
            ("/foo/bar", true),
            ("C:/Documents", true),
            ("\\Program Files", true),
            ("C:Documents", false),
            ("foo/bar", false),
            ("./http://foo/bar", false),
        ] {
            assert_eq!(is_absolute(path), absolute, "{path}");
        }
        assert!(matches!(SourcePath::new("C:/ui/main.slint"), SourcePath::File(_)));
    }

    #[test]
    fn clean_paths() {
        for (path, clean) in [
            ("ab/.././cb/./././..", ""),
            ("../../ab/../cd", "../../cd"),
            ("/../ab/./cd//ef", "/ab/cd/ef"),
            ("a\\b", "a\\b"),
        ] {
            assert_eq!(clean_path(Path::new(path)), Path::new(clean), "{path}");
        }
        assert_eq!(join("ui/main.slint", "sub\\x.slint"), SourcePath::new("ui/sub/x.slint"));
        assert_eq!(join("/ui/main.slint", "\\abs\\x.slint"), SourcePath::new("/abs/x.slint"));
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
            let path = SourcePath::from_url(&url);
            assert_eq!(path.to_url(), Some(url.clone()));
            let image = path.parent().join("images/logo.png").unwrap();
            assert_eq!(image.to_url(), url.join("images/logo.png").ok(), "{s}");
        }
    }
}
