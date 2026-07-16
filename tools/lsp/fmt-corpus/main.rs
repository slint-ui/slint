// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Corpus-testing harness for the query-based formatter prototype
//! (`tools/lsp/fmt/`). Not part of the LSP: a dev-only tool for finding
//! formatter bugs and measuring how much it changes real `.slint` files.
//!
//! ```sh
//! cargo run -p slint-lsp --example fmt-corpus -- diff --corpus fast
//! cargo run -p slint-lsp --example fmt-corpus -- canonicalize --corpus full
//! ```
//!
//! This whole directory can be deleted (along with its `[[example]]` entry in
//! `Cargo.toml`) without affecting the formatter or the LSP.

// This harness only calls `format_document_query`, not the rest of fmt's
// surface (some of which is only reached from `fmt/tool.rs`, excluded
// below), so parts of the included modules are otherwise unused here.
#![allow(dead_code)]

// `#[path]` on an inline module sets the base directory its unattributed
// children resolve in, so this reaches the real `tools/lsp/fmt/*.rs` files
// without copying them. Only the modules the formatter's entry point
// actually needs are included; `fmt::fmt` (the old imperative formatter) and
// `fmt::tool` (the LSP's CLI glue) are intentionally left out.
#[path = "../fmt"]
mod fmt {
    pub mod atoms;
    pub mod engine;
    pub mod render;
    pub mod rules;
    pub mod writer;
}

mod canonicalize;
mod diff;

use clap::{Parser, Subcommand, ValueEnum};
use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::parser::syntax_nodes;
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(about = "Corpus-testing harness for the query-based formatter prototype")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Report whole-file line churn between original and formatted output.
    Diff(CorpusArgs),
    /// Check whether indentation-noisy multiline blocks converge to the same
    /// formatted output.
    Canonicalize(CorpusArgs),
}

#[derive(clap::Args, Debug)]
struct CorpusArgs {
    /// Use a built-in corpus preset instead of explicit paths.
    #[arg(long, value_enum)]
    corpus: Option<CorpusPreset>,

    /// Files or directories to analyze.
    paths: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CorpusPreset {
    /// Recursively traverse the current directory.
    Full,
    /// Traverse a faster subset: examples/ and tests/cases/.
    Fast,
}

struct ResolvedCorpus {
    label: String,
    files: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match &cli.command {
        Command::Diff(args) => {
            resolve_corpus(args).and_then(|corpus| diff::run(&corpus.label, &corpus.files))
        }
        Command::Canonicalize(args) => {
            resolve_corpus(args).and_then(|corpus| canonicalize::run(&corpus.label, &corpus.files))
        }
    };

    match result {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// Format `source` with the query-based formatter, or `None` if it doesn't
/// parse cleanly. The formatter itself never fails: unrecognized constructs
/// are just kept verbatim.
fn format_source(source: &str) -> Option<String> {
    let mut diagnostics = BuildDiagnostics::default();
    let node = i_slint_compiler::parser::parse(source.to_owned(), None, &mut diagnostics);
    if diagnostics.has_errors() {
        return None;
    }

    let document = syntax_nodes::Document::new(node)?;
    let mut output = Vec::new();
    fmt::rules::format_document_query(document, &mut fmt::writer::FileWriter { file: &mut output })
        .expect("writing to a Vec<u8> cannot fail");
    Some(String::from_utf8(output).expect("formatter output is valid UTF-8"))
}

fn resolve_corpus(args: &CorpusArgs) -> io::Result<ResolvedCorpus> {
    if args.corpus.is_some() && !args.paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "use either --corpus or explicit paths, not both",
        ));
    }

    let (label, roots) = if !args.paths.is_empty() {
        (format!("explicit paths ({})", display_paths(&args.paths)), args.paths.clone())
    } else {
        match args.corpus.unwrap_or(CorpusPreset::Full) {
            CorpusPreset::Full => {
                ("full corpus (current directory traversal)".into(), vec![std::env::current_dir()?])
            }
            CorpusPreset::Fast => {
                let roots = ["examples", "tests/cases"]
                    .into_iter()
                    .map(PathBuf::from)
                    .filter(|path| path.exists())
                    .collect::<Vec<_>>();
                ("fast corpus (examples/ + tests/cases/)".into(), roots)
            }
        }
    };

    if roots.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no corpus roots found for the selected input",
        ));
    }

    let files = collect_standalone_slint_files(&roots)?;
    if files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no standalone .slint files found in {}", display_paths(&roots)),
        ));
    }

    Ok(ResolvedCorpus { label, files })
}

fn collect_standalone_slint_files(paths: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut files = BTreeSet::new();
    for root in paths {
        collect_path(root, &mut files)?;
    }
    Ok(files.into_iter().collect())
}

fn collect_path(path: &Path, files: &mut BTreeSet<PathBuf>) -> io::Result<()> {
    if path.is_dir() {
        let mut entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_path(&entry.path(), files)?;
        }
    } else if path.extension().and_then(|extension| extension.to_str()) == Some("slint") {
        files.insert(path.to_owned());
    }
    Ok(())
}

fn display_paths(paths: &[PathBuf]) -> String {
    paths.iter().map(|path| path.display().to_string()).collect::<Vec<_>>().join(", ")
}
