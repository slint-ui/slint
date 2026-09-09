// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::{self, Write};
use std::path::Path;

#[derive(Debug)]
pub(crate) struct WriteFailure {
    pub error: io::Error,
    pub may_have_changed: bool,
}

#[cfg(any(test, feature = "system-testing"))]
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WriteFault {
    BeforeOpen,
    AfterTruncate,
    AfterBytes(usize),
}

pub(crate) fn write_source(
    path: &Path,
    contents: &[u8],
    #[cfg(any(test, feature = "system-testing"))] fault: Option<WriteFault>,
) -> Result<(), WriteFailure> {
    let mut may_have_changed = false;
    let result = (|| {
        #[cfg(any(test, feature = "system-testing"))]
        if matches!(fault, Some(WriteFault::BeforeOpen)) {
            return Err(io::Error::other("injected failure before opening source"));
        }
        let mut file = match std::fs::OpenOptions::new().write(true).open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // Creation can affect the filesystem even if it reports an error.
                may_have_changed = true;
                std::fs::OpenOptions::new().write(true).create_new(true).open(path)?
            }
            Err(error) => return Err(error),
        };
        // A failed truncation may already have changed the file.
        may_have_changed = true;
        file.set_len(0)?;
        #[cfg(any(test, feature = "system-testing"))]
        match fault {
            Some(WriteFault::AfterTruncate) => {
                return Err(io::Error::other("injected failure after truncating source"));
            }
            Some(WriteFault::AfterBytes(count)) => {
                file.write_all(&contents[..count.min(contents.len())])?;
                return Err(io::Error::other("injected failure after partial source write"));
            }
            _ => {}
        }
        file.write_all(contents)
    })();
    result.map_err(|error| WriteFailure { error, may_have_changed })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_preserve_mutation_evidence() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.slint");
        for (fault, expected, mutated) in [
            (WriteFault::BeforeOpen, &b"original"[..], false),
            (WriteFault::AfterTruncate, &b""[..], true),
            (WriteFault::AfterBytes(3), &b"rep"[..], true),
        ] {
            std::fs::write(&path, b"original").unwrap();
            let failure = write_source(&path, b"replacement", Some(fault)).unwrap_err();
            assert_eq!(failure.may_have_changed, mutated);
            assert_eq!(std::fs::read(&path).unwrap(), expected);
        }
    }

    #[test]
    fn failed_open_does_not_claim_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let failure = write_source(directory.path(), b"replacement", None).unwrap_err();
        assert!(!failure.may_have_changed);
    }

    #[test]
    fn failed_creation_is_conservatively_a_possible_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("missing/source.slint");
        let failure = write_source(&path, b"replacement", None).unwrap_err();
        assert!(failure.may_have_changed);
        assert!(!path.exists());
    }

    #[test]
    fn successful_write_truncates_and_can_create() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.slint");
        write_source(&path, b"original", None).unwrap();
        write_source(&path, b"new", None).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
    }
}
