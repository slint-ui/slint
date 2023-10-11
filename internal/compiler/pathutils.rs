// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! Reimplement some Path handling code: The one in `std` is not available
//! when running in WASM!
//!
//! This is not helped by us using URLs in place of paths *sometimes*.

use std::path::{Path, PathBuf};

/// Check whether a `Path` is actually an URL.
pub fn is_url(path: &Path) -> bool {
    let Some(path) = path.to_str() else {
        // URLs can always convert to string in Rust
        return false;
    };

    to_url(path).is_some()
}

/// Convert a `Path` to an `url::Url` if possible
fn to_url(path: &str) -> Option<url::Url> {
    let Ok(url) = url::Url::parse(path) else {
        return None;
    };
    if url.scheme().len() == 1 {
        // "c:/" is a path, not a URL
        None
    } else {
        Some(url)
    }
}

#[test]
fn test_to_url() {
    #[track_caller]
    fn th(input: &str, expected: bool) {
        assert_eq!(to_url(&input).is_some(), expected);
    }

    th("https://foo.bar/", true);
    th("builtin:/foo/bar/", true);
    th("user://foo/bar.rs", true);
    th("/foo/bar/", false);
    th("../foo/bar", false);
    th("foo/bar", false);
    th("./foo/bar", false);
    // Windows style paths:
    th("C:\\Documents\\Newsletters\\Summer2018.pdf", false);
    th("\\Program Files\\Custom Utilities\\StringFinder.exe", false);
    th("2018\\January.xlsx", false);
    th("..\\Publications\\TravelBrochure.pdf", false);
    th("C:\\Projects\\library\\library.sln", false);
    th("C:Projects\\library\\library.sln", false);
    th("\\\\system07\\C$\\", false);
    th("\\\\Server2\\Share\\Test\\Foo.txt", false);
    th("\\\\.\\C:\\Test\\Foo.txt", false);
    th("\\\\?\\C:\\Test\\Foo.txt", false);
    th("\\\\.\\Volume{b75e2c83-0000-0000-0000-602f00000000}\\Test\\Foo.txt", false);
    th("\\\\?\\Volume{b75e2c83-0000-0000-0000-602f00000000}\\Test\\Foo.txt", false);
    // Windows style paths - some programs will helpfully convert the backslashes:-(:
    th("C:/Documents/Newsletters/Summer2018.pdf", false);
    th("/Program Files/Custom Utilities/StringFinder.exe", false);
    th("2018/January.xlsx", false);
    th("../Publications/TravelBrochure.pdf", false);
    th("C:/Projects/library/library.sln", false);
    th("C:Projects/library/library.sln", false);
    th("//system07/C$/", false);
    th("//Server2/Share/Test/Foo.txt", false);
    th("//./C:/Test/Foo.txt", false);
    th("//?/C:/Test/Foo.txt", false);
    th("//./Volume{b75e2c83-0000-0000-0000-602f00000000}/Test/Foo.txt", false);
    th("//?/Volume{b75e2c83-0000-0000-0000-602f00000000}/Test/Foo.txt", false);
    // Some corner case:
    th("C:///Documents/Newsletters/Summer2018.pdf", false);
    th("/http://foo/bar/", false);
    th("../http://foo/bar", false);
    th("foo/http://foo/bar", false);
    th("./http://foo/bar", false);
    th("", false);
}

// Check whether a path is absolute.
//
// This returns true is the Path contains a `url::Url` or starts with anything
// that is a root prefix (e.g. `/` on Unix or `C:\` on Windows).
pub fn is_absolute(path: &Path) -> bool {
    let Some(path) = path.to_str() else {
        // URLs can always convert to string in Rust
        return false;
    };

    if to_url(path).is_some() {
        return true;
    }

    matches!(components(path, 0, &None), Some((PathComponent::RootComponent(_), _, _)))
}

#[test]
fn test_is_absolute() {
    #[track_caller]
    fn th(input: &str, expected: bool) {
        let path = PathBuf::from(input);
        assert_eq!(is_absolute(&path), expected);
    }

    th("https://foo.bar/", true);
    th("builtin:/foo/bar/", true);
    th("user://foo/bar.rs", true);
    th("/foo/bar/", true);
    th("../foo/bar", false);
    th("foo/bar", false);
    th("./foo/bar", false);
    // Windows style paths:
    th("C:\\Documents\\Newsletters\\Summer2018.pdf", true);
    // Windows actually considers this to be a relative path:
    th("\\Program Files\\Custom Utilities\\StringFinder.exe", true);
    th("2018\\January.xlsx", false);
    th("..\\Publications\\TravelBrochure.pdf", false);
    th("C:\\Projects\\library\\library.sln", true);
    th("C:Projects\\library\\library.sln", false);
    th("\\\\system07\\C$\\", true);
    th("\\\\Server2\\Share\\Test\\Foo.txt", true);
    th("\\\\.\\C:\\Test\\Foo.txt", true);
    th("\\\\?\\C:\\Test\\Foo.txt", true);
    th("\\\\.\\Volume{b75e2c83-0000-0000-0000-602f00000000}\\Test\\Foo.txt", true);
    th("\\\\?\\Volume{b75e2c83-0000-0000-0000-602f00000000}\\Test\\Foo.txt", true);
    // Windows style paths - some programs will helpfully convert the backslashes:-(:
    th("C:/Documents/Newsletters/Summer2018.pdf", true);
    // Windows actually considers this to be a relative path:
    th("/Program Files/Custom Utilities/StringFinder.exe", true);
    th("2018/January.xlsx", false);
    th("../Publications/TravelBrochure.pdf", false);
    th("C:/Projects/library/library.sln", true);
    th("C:Projects/library/library.sln", false);
    // These are true, but only because '/' is root on Unix!
    th("//system07/C$/", true);
    th("//Server2/Share/Test/Foo.txt", true);
    th("//./C:/Test/Foo.txt", true);
    th("//?/C:/Test/Foo.txt", true);
    th("//./Volume{b75e2c83-0000-0000-0000-602f00000000}/Test/Foo.txt", true);
    th("//?/Volume{b75e2c83-0000-0000-0000-602f00000000}/Test/Foo.txt", true);
    // Some corner case:
    th("C:///Documents/Newsletters/Summer2018.pdf", true);
    th("C:", false);
    th("C:\\", true);
    th("C:/", true);
    th("", false);
}

#[derive(Debug, PartialEq)]
enum PathComponent<'a> {
    RootComponent(&'a str),
    EmptyComponent,
    SameDirectoryComponent(&'a str),
    ParentDirectoryComponent(&'a str),
    DirectoryComponent(&'a str),
    FileComponent(&'a str),
}

/// Find which kind of path separator is used in the `str`
fn find_path_separator(path: &str) -> char {
    for c in path.chars() {
        if c == '/' || c == '\\' {
            return c;
        }
    }
    '/'
}

/// Look at the individual parts of a `path`.
///
/// This will not work well with URLs, check whether something is an URL first!
fn components<'a>(
    path: &'a str,
    offset: usize,
    separator: &Option<char>,
) -> Option<(PathComponent<'a>, usize, char)> {
    use PathComponent as PC;

    if offset >= path.len() {
        return None;
    }

    let b = path.as_bytes();

    if offset == 0 {
        if b.len() >= 3 {
            if ((b'a'..=b'z').contains(&b[0]) || (b'A'..=b'Z').contains(&b[0]))
                && b[1] == b':'
                && (b[2] == b'\\' || b[2] == b'/')
            {
                return Some((PC::RootComponent(&path[0..3]), 3, b[2] as char));
            }
        }
        if b.len() >= 2 {
            if b[0] == b'\\' && b[1] == b'\\' {
                let second_bs = path[2..]
                    .find('\\')
                    .map(|pos| pos + 2 + 1)
                    .and_then(|pos1| path[pos1..].find('\\').map(|pos2| pos2 + pos1));
                if let Some(end_offset) = second_bs {
                    return Some((PC::RootComponent(&path[0..=end_offset]), end_offset + 1, '\\'));
                }
            }
        }
        if b[0] == b'/' || b[0] == b'\\' {
            return Some((PC::RootComponent(&path[0..1]), 1, b[0] as char));
        }
    }

    let separator = separator.unwrap_or_else(|| find_path_separator(path));

    let next_component = path[offset..].find(separator).map(|p| p + offset).unwrap_or(path.len());
    if &path[offset..next_component] == "." {
        return Some((
            PC::SameDirectoryComponent(&path[offset..next_component]),
            next_component + 1,
            separator,
        ));
    }
    if &path[offset..next_component] == ".." {
        return Some((
            PC::ParentDirectoryComponent(&path[offset..next_component]),
            next_component + 1,
            separator,
        ));
    }

    if next_component == path.len() {
        Some((PC::FileComponent(&path[offset..next_component]), next_component, separator))
    } else {
        if next_component == offset {
            Some((PC::EmptyComponent, next_component + 1, separator))
        } else {
            Some((
                PC::DirectoryComponent(&path[offset..next_component]),
                next_component + 1,
                separator,
            ))
        }
    }
}

#[test]
fn test_components() {
    use PathComponent as PC;

    #[track_caller]
    fn th(input: &str, expected: Option<(PathComponent, usize, char)>) {
        assert_eq!(components(input, 0, &None), expected);
    }

    th("/foo/bar/", Some((PC::RootComponent("/"), 1, '/')));
    th("../foo/bar", Some((PC::ParentDirectoryComponent(".."), 3, '/')));
    th("foo/bar", Some((PC::DirectoryComponent("foo"), 4, '/')));
    th("./foo/bar", Some((PC::SameDirectoryComponent("."), 2, '/')));
    // Windows style paths:
    th("C:\\Documents\\Newsletters\\Summer2018.pdf", Some((PC::RootComponent("C:\\"), 3, '\\')));
    // Windows actually considers this to be a relative path:
    th(
        "\\Program Files\\Custom Utilities\\StringFinder.exe",
        Some((PC::RootComponent("\\"), 1, '\\')),
    );
    th("2018\\January.xlsx", Some((PC::DirectoryComponent("2018"), 5, '\\')));
    th("..\\Publications\\TravelBrochure.pdf", Some((PC::ParentDirectoryComponent(".."), 3, '\\')));
    // TODO: This is wrong, but we are unlikely to need it:-)
    th("C:Projects\\library\\library.sln", Some((PC::DirectoryComponent("C:Projects"), 11, '\\')));
    th("\\\\system07\\C$\\", Some((PC::RootComponent("\\\\system07\\C$\\"), 14, '\\')));
    th(
        "\\\\Server2\\Share\\Test\\Foo.txt",
        Some((PC::RootComponent("\\\\Server2\\Share\\"), 16, '\\')),
    );
    th("\\\\.\\C:\\Test\\Foo.txt", Some((PC::RootComponent("\\\\.\\C:\\"), 7, '\\')));
    th("\\\\?\\C:\\Test\\Foo.txt", Some((PC::RootComponent("\\\\?\\C:\\"), 7, '\\')));
    th(
        "\\\\.\\Volume{b75e2c83-0000-0000-0000-602f00000000}\\Test\\Foo.txt",
        Some((
            PC::RootComponent("\\\\.\\Volume{b75e2c83-0000-0000-0000-602f00000000}\\"),
            49,
            '\\',
        )),
    );
    th(
        "\\\\?\\Volume{b75e2c83-0000-0000-0000-602f00000000}\\Test\\Foo.txt",
        Some((
            PC::RootComponent("\\\\?\\Volume{b75e2c83-0000-0000-0000-602f00000000}\\"),
            49,
            '\\',
        )),
    );
    // Windows style paths - some programs will helpfully convert the backslashes:-(:
    th("C:/Documents/Newsletters/Summer2018.pdf", Some((PC::RootComponent("C:/"), 3, '/')));
    // TODO: All the following are wrong, but unlikely to bother us!
    th("/Program Files/Custom Utilities/StringFinder.exe", Some((PC::RootComponent("/"), 1, '/')));
    th("//system07/C$/", Some((PC::RootComponent("/"), 1, '/')));
    th("//Server2/Share/Test/Foo.txt", Some((PC::RootComponent("/"), 1, '/')));
    th("//./C:/Test/Foo.txt", Some((PC::RootComponent("/"), 1, '/')));
    th("//?/C:/Test/Foo.txt", Some((PC::RootComponent("/"), 1, '/')));
    th(
        "//./Volume{b75e2c83-0000-0000-0000-602f00000000}/Test/Foo.txt",
        Some((PC::RootComponent("/"), 1, '/')),
    );
    th(
        "//?/Volume{b75e2c83-0000-0000-0000-602f00000000}/Test/Foo.txt",
        Some((PC::RootComponent("/"), 1, '/')),
    );
    // // Some corner case:
    // th("C:///Documents/Newsletters/Summer2018.pdf", true);
    // TODO: This is wrong, but unlikely to be needed
    th("C:", Some((PC::FileComponent("C:"), 2, '/')));
    th("foo", Some((PC::FileComponent("foo"), 3, '/')));
    th("foo/", Some((PC::DirectoryComponent("foo"), 4, '/')));
    th("foo\\", Some((PC::DirectoryComponent("foo"), 4, '\\')));
    th("", None);
}

struct Components<'a> {
    path: &'a str,
    offset: usize,
    separator: Option<char>,
}

fn component_iter<'a>(path: &'a str) -> Components<'a> {
    Components { path, offset: 0, separator: None }
}

impl<'a> Iterator for Components<'a> {
    type Item = PathComponent<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let Some((result, new_offset, separator)) =
            components(&self.path, self.offset, &self.separator)
        else {
            return None;
        };
        self.offset = new_offset;
        self.separator = Some(separator);

        Some(result)
    }
}

fn clean_path_string(path: &str) -> String {
    use PathComponent as PC;

    let separator = find_path_separator(path);
    let path = if separator == '\\' {
        path.replace('/', &format!("{separator}"))
    } else {
        path.replace('\\', "/")
    };

    let mut clean_components = Vec::new();
    let mut iter = component_iter(&path);

    while let Some(component) = iter.next() {
        match component {
            PC::RootComponent(v) => {
                clean_components = vec![PC::RootComponent(v)];
            }
            PC::EmptyComponent | PC::SameDirectoryComponent(_) => { /* nothing to do */ }
            PC::ParentDirectoryComponent(v) => {
                match clean_components.last() {
                    Some(PC::DirectoryComponent(_)) => {
                        clean_components.pop();
                    }
                    Some(PC::FileComponent(_)) => unreachable!("Must be the last component"),
                    Some(PC::SameDirectoryComponent(_) | PC::EmptyComponent) => {
                        unreachable!("Will never be in a the vector")
                    }
                    Some(PC::ParentDirectoryComponent(_)) => {
                        clean_components.push(PC::ParentDirectoryComponent(v));
                    }
                    Some(PC::RootComponent(_)) => { /* do nothing */ }
                    None => {
                        clean_components.push(PC::ParentDirectoryComponent(v));
                    }
                };
            }
            PC::DirectoryComponent(v) => clean_components.push(PC::DirectoryComponent(v)),
            PC::FileComponent(v) => clean_components.push(PC::FileComponent(v)),
        }
    }
    if clean_components.is_empty() {
        ".".to_string()
    } else {
        let mut result = String::new();
        for c in clean_components {
            match c {
                PC::RootComponent(v) => {
                    result = v.to_string();
                }
                PC::EmptyComponent | PC::SameDirectoryComponent(_) => {
                    unreachable!("Never in the vector!")
                }
                PC::ParentDirectoryComponent(v) => {
                    result += &format!("{v}{separator}");
                }
                PC::DirectoryComponent(v) => result += &format!("{v}{separator}"),
                PC::FileComponent(v) => {
                    result += v;
                }
            }
        }
        result
    }
}

#[test]
fn test_clean_path_string() {
    #[track_caller]
    fn th(input: &str, expected: &str) {
        let result = clean_path_string(input);
        assert_eq!(result, expected);
    }

    th("../../ab/.././hello.txt", "../../hello.txt");
    th("/../../ab/.././hello.txt", "/hello.txt");
    th("ab/.././cb/././///./..", ".");
    th("ab/.././cb/.\\.\\\\\\\\./..", ".");
    th("ab\\..\\.\\cb\\././///./..", ".");
}

/// Return a clean up path without unnecessary `.` and `..` directories in it.
///
/// This will *not* look at the file system, so symlinks will not get resolved.
pub fn clean_path(path: &Path) -> PathBuf {
    let Some(path_str) = path.to_str() else {
        return path.to_owned();
    };

    if let Some(url) = to_url(path_str) {
        // URL is cleaned up while parsing!
        PathBuf::from(url.to_string())
    } else {
        PathBuf::from(clean_path_string(path_str))
    }
}

fn dirname_string(path: &str) -> String {
    let separator = find_path_separator(path);
    let mut iter = component_iter(path);
    let mut result = String::new();

    while let Some(component) = iter.next() {
        match component {
            PathComponent::RootComponent(v) => result = v.to_string(),
            PathComponent::EmptyComponent => result.push(separator),
            PathComponent::SameDirectoryComponent(v)
            | PathComponent::ParentDirectoryComponent(v)
            | PathComponent::DirectoryComponent(v) => result += &format!("{v}{separator}"),
            PathComponent::FileComponent(_) => { /* nothing to do */ }
        };
    }

    if result.is_empty() {
        String::from(".")
    } else {
        result
    }
}

#[test]
fn test_dirname() {
    #[track_caller]
    fn th(input: &str, expected: &str) {
        let result = dirname_string(&input);
        assert_eq!(result, expected);
    }

    th("/../../ab/.././", "/../../ab/.././");
    th("ab/.././cb/./././..", "ab/.././cb/./././../");
    th("hello.txt", ".");
    th("../hello.txt", "../");
    th("/hello.txt", "/");
}

/// Return the part of `Path` before the last path separator.
pub fn dirname(path: &Path) -> PathBuf {
    let Some(path_str) = path.to_str() else {
        return path.to_owned();
    };

    PathBuf::from(dirname_string(path_str))
}

/// Join a `path` to a `base_path`, handling URLs in both, matching up
/// path separators, etc.
///
/// The result will be a `clean_path(...)`.
pub fn join(base: &Path, path: &Path) -> Option<PathBuf> {
    if is_absolute(path) {
        return Some(path.to_owned());
    }

    let Some(base_str) = base.to_str() else {
        return Some(path.to_owned());
    };
    let Some(path_str) = path.to_str() else {
        return Some(path.to_owned());
    };

    let path_separator = find_path_separator(path_str);

    if let Some(mut base_url) = to_url(base_str) {
        let path_str = if path_separator != '/' {
            path_str.replace(path_separator, "/")
        } else {
            path_str.to_string()
        };

        let base_path = base_url.path();
        if !base_path.is_empty() && !base_path.ends_with('/') {
            base_url.set_path(&format!("{base_path}/"));
        }

        Some(PathBuf::from(base_url.join(&path_str).ok()?.to_string()))
    } else {
        let base_separator = find_path_separator(base_str);
        let path_str = if path_separator != base_separator {
            path_str.replace(path_separator, &base_separator.to_string())
        } else {
            path_str.to_string()
        };
        let joined = clean_path_string(&format!("{base_str}{base_separator}{path_str}"));
        Some(PathBuf::from(joined))
    }
}

#[test]
fn test_join() {
    #[track_caller]
    fn th(base: &str, path: &str, expected: Option<&str>) {
        let base = PathBuf::from(base);
        let path = PathBuf::from(path);
        let expected = expected.map(|e| PathBuf::from(e));

        let result = join(&base, &path);
        assert_eq!(result, expected);
    }

    th("https://slint.dev/", "/hello.txt", Some("/hello.txt"));
    th("https://slint.dev/", "../../hello.txt", Some("https://slint.dev/hello.txt"));
    th("/../../ab/.././", "hello.txt", Some("/hello.txt"));
    th("ab/.././cb/./././..", "../.././hello.txt", Some("../../hello.txt"));
    th("builtin:/foo", "..\\bar.slint", Some("builtin:/bar.slint"));
    th("builtin:/", "..\\bar.slint", Some("builtin:/bar.slint"));
    th("builtin:/foo/baz", "..\\bar.slint", Some("builtin:/foo/bar.slint"));
    th("builtin:/foo", "bar.slint", Some("builtin:/foo/bar.slint"));
    th("builtin:/foo/", "bar.slint", Some("builtin:/foo/bar.slint"));
    th("builtin:/", "..\\bar.slint", Some("builtin:/bar.slint"));
}
