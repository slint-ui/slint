#!/usr/bin/env -S cargo +nightly -Zscript -q
---
[package]
edition = "2024"

[dependencies]
clap = { version = "4.6", features = ["derive"] }
cargo_metadata = "0.23"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
---

use std::{collections::HashSet, fs::File};

use cargo_metadata::{CargoOpt, MetadataCommand, PackageName, semver::Version};
use clap::Parser;
use serde::{Deserialize, Serialize};

/// Top-level structure in `cargo-sources.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum CargoSource {
    #[serde(rename_all = "kebab-case")]
    Archive { archive_type: String, url: String, sha256: String, dest: String },
    #[serde(rename_all = "kebab-case")]
    Inline { contents: String, dest: String, dest_filename: String },
    #[serde(rename_all = "kebab-case")]
    Git { url: String, commit: String, dest: String },
    #[serde(rename_all = "kebab-case")]
    Shell { commands: Vec<String> },
}

impl CargoSource {
    fn name_and_version(&self) -> Option<(PackageName<String>, Version)> {
        match self {
            CargoSource::Archive { dest, .. } | CargoSource::Inline { dest, .. } => {
                dest.strip_prefix("cargo/vendor/")?.rsplit_once('-').and_then(|(name, version)| {
                    Some((PackageName::new(name.into()), Version::parse(version).ok()?))
                })
            }
            _ => None,
        }
    }
}

#[derive(Parser)]
struct Args {
    /// The target triple to use, should usually be `x86_64-unknown-linux-gnu`
    #[arg(long)]
    target: Option<String>,
    /// Comma-separated features to use. Can be specified multiple times
    #[arg(long)]
    features: Vec<String>,
    #[arg(long)]
    no_default_features: bool,
    #[arg(long)]
    cargo_sources: String,
    #[arg(long)]
    out: String,
}

fn main() {
    let args = Args::parse();

    let cargo_sources: Vec<CargoSource> =
        serde_json::from_reader(File::open(args.cargo_sources).unwrap()).unwrap();

    let features = args
        .features
        .iter()
        .flat_map(|feature_set| feature_set.split(','))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    let mut cmd = MetadataCommand::new();

    cmd.other_options(["--frozen"].map(ToOwned::to_owned));

    if let Some(target) = &args.target {
        cmd.other_options(["--filter-platform", target].map(ToOwned::to_owned));
    }

    if args.no_default_features {
        cmd.features(cargo_metadata::CargoOpt::NoDefaultFeatures);
    }

    if !features.is_empty() {
        cmd.features(CargoOpt::SomeFeatures(features));
    }

    let cargo_metadata = cmd.exec().unwrap();

    let package_set: HashSet<(PackageName<String>, Version)> =
        cargo_metadata.packages.into_iter().map(|pkg| (pkg.name, pkg.version)).collect();

    let filtered_sources = cargo_sources
        .into_iter()
        .filter(|source| {
            source
                .name_and_version()
                .is_some_and(|name_and_version| package_set.contains(&name_and_version))
        })
        .collect::<Vec<_>>();

    serde_json::to_writer_pretty(File::create(args.out).unwrap(), &filtered_sources).unwrap();
}
