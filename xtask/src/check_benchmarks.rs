// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Compare benchmark medians against a baseline.
//!
//! The baseline stores one median wall-clock time per benchmark, in
//! nanoseconds, measured on the full-CI runner (see
//! `xtask/benchmark-baseline.toml`). A benchmark fails when its median exceeds
//! the baseline by more than the tolerance; a missing baseline entry is only a
//! warning, so adding a benchmark doesn't fail CI before the baseline is
//! refreshed.
//!
//! Run `cargo xtask check_benchmarks --save-baseline` to record the measured
//! medians, or `--from-results target/benchmark-results.toml --save-baseline`
//! to seed the baseline from a CI artifact without running the benchmarks.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use regex::Regex;
use toml_edit::{DocumentMut, Item};

/// Fail when a median is this much slower than its baseline, unless the
/// baseline's `[settings] tolerance` overrides it.
const DEFAULT_TOLERANCE: f64 = 1.2;

/// Bound criterion's per-benchmark run time, since the comparison only needs a
/// rough median.
const CRITERION_WARM_UP_SECS: &str = "0.5";
const CRITERION_MEASUREMENT_SECS: &str = "2";

#[derive(Debug, clap::Parser)]
pub struct CheckBenchmarks {
    /// Baseline to compare the measured medians against; values are in
    /// nanoseconds.
    #[arg(long, value_name = "PATH", default_value = "xtask/benchmark-baseline.toml")]
    baseline: PathBuf,

    /// Fail when a median exceeds its baseline value by more than this factor.
    /// Overrides the value in the baseline file.
    #[arg(long, value_name = "FACTOR")]
    tolerance: Option<f64>,

    /// Number of samples divan collects per benchmark.
    #[arg(long, value_name = "N", default_value_t = 100)]
    sample_count: u32,

    /// Number of samples criterion collects per benchmark.
    #[arg(long, value_name = "N", default_value_t = 100)]
    sample_size: u32,

    /// Run only benchmarks whose name matches this pattern (a regex for divan,
    /// a substring for criterion).
    #[arg(long, value_name = "PATTERN")]
    filter: Option<String>,

    /// Repeat both suites this many times and keep the fastest median per
    /// benchmark.
    #[arg(long, value_name = "N", default_value_t = 1)]
    repeat: u32,

    /// Read the medians from this file instead of running the benchmarks.
    #[arg(long, value_name = "PATH", conflicts_with = "repeat")]
    from_results: Option<PathBuf>,

    /// Report the comparison without failing.
    #[arg(long)]
    report_only: bool,

    /// Write the measured medians to the baseline instead of comparing.
    #[arg(long)]
    save_baseline: bool,

    /// Write the measured medians to this file.
    #[arg(long, value_name = "PATH", default_value = "target/benchmark-results.toml")]
    results: PathBuf,
}

impl CheckBenchmarks {
    pub fn run(&self) -> Result<()> {
        if self.repeat == 0 {
            bail!("--repeat must be at least 1");
        }

        let medians = if let Some(path) = &self.from_results {
            load_medians(&rooted(path))
                .with_context(|| format!("Failed to read {}", path.display()))?
        } else {
            self.measure()?
        };
        if medians.is_empty() {
            bail!("No benchmark medians collected");
        }

        let results_path = rooted(&self.results);
        write_medians(&results_path, &medians)?;
        println!("Wrote {} benchmark medians to {}\n", medians.len(), results_path.display());

        if self.save_baseline {
            let baseline_path = rooted(&self.baseline);
            update_baseline(&baseline_path, &medians, self.filter.is_none())?;
            println!("Wrote baseline to {}", baseline_path.display());
            return Ok(());
        }

        let baseline = load_baseline(&rooted(&self.baseline))?;
        if baseline.medians.is_empty() {
            eprintln!(
                "warning: no baseline medians in {}; every benchmark is reported as new",
                self.baseline.display()
            );
        }
        let tolerance = self.tolerance.or(baseline.tolerance).unwrap_or(DEFAULT_TOLERANCE);
        let rows = compare(&medians, &baseline.medians, tolerance, self.filter.is_none());
        print_report(&rows, tolerance);
        append_step_summary(&rows, tolerance)?;

        let regressions = rows.iter().filter(|row| row.status == Status::Regression).count();
        if regressions > 0 && !self.report_only {
            bail!("{regressions} benchmark(s) exceed the baseline by more than {tolerance:.2}x");
        }
        Ok(())
    }

    fn measure(&self) -> Result<BTreeMap<String, f64>> {
        let mut medians = BTreeMap::new();
        for _ in 0..self.repeat {
            merge_min(&mut medians, self.measure_divan()?);
            merge_min(&mut medians, self.measure_criterion()?);
        }
        Ok(medians)
    }

    fn measure_divan(&self) -> Result<BTreeMap<String, f64>> {
        let sample_count = self.sample_count.to_string();
        let mut args = vec![
            "bench",
            "--locked",
            "-p",
            "i-slint-compiler",
            "--features",
            "rust",
            "--bench",
            "semantic_analysis",
            "--",
        ];
        if let Some(filter) = &self.filter {
            args.push(filter);
        }
        args.extend(["--sample-count", &sample_count]);
        let output = run_cargo_bench(&args)?;
        Ok(parse_divan_output(&output))
    }

    fn measure_criterion(&self) -> Result<BTreeMap<String, f64>> {
        let criterion_dir = cargo_target_dir()?.join("criterion");
        // Criterion keeps the results of earlier runs (and filters), so start
        // from a clean directory to read exactly this run's medians.
        if criterion_dir.exists() {
            std::fs::remove_dir_all(&criterion_dir)
                .with_context(|| format!("Failed to remove {}", criterion_dir.display()))?;
        }

        let sample_size = self.sample_size.to_string();
        let mut args = vec!["bench", "--locked", "-p", "i-slint-core", "--bench", "string", "--"];
        if let Some(filter) = &self.filter {
            args.push(filter);
        }
        args.extend([
            "--sample-size",
            &sample_size,
            "--warm-up-time",
            CRITERION_WARM_UP_SECS,
            "--measurement-time",
            CRITERION_MEASUREMENT_SECS,
        ]);
        run_cargo_bench(&args)?;
        collect_criterion_medians(&criterion_dir)
    }
}

fn rooted(path: &Path) -> PathBuf {
    if path.is_absolute() { path.to_path_buf() } else { crate::root_dir().join(path) }
}

fn run_cargo_bench(args: &[&str]) -> Result<String> {
    let output = Command::new("cargo")
        .args(args)
        .current_dir(crate::root_dir())
        .stdin(Stdio::null())
        // Keep the build and benchmark progress visible in the log.
        .stderr(Stdio::inherit())
        .stdout(Stdio::piped())
        .output()
        .context("Failed to run cargo bench")?;
    if !output.status.success() {
        bail!("cargo bench failed with {}", output.status);
    }
    String::from_utf8(output.stdout).context("Benchmark output is not UTF-8")
}

fn cargo_target_dir() -> Result<PathBuf> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .current_dir(crate::root_dir())
        .no_deps()
        .exec()
        .context("Failed to run cargo metadata")?;
    Ok(metadata.target_directory.into_std_path_buf())
}

fn merge_min(target: &mut BTreeMap<String, f64>, source: BTreeMap<String, f64>) {
    for (name, value) in source {
        let slot = target.entry(name).or_insert(f64::INFINITY);
        *slot = slot.min(value);
    }
}

static TIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    // µs is U+00B5 MICRO SIGN; accept U+03BC GREEK SMALL LETTER MU as well.
    Regex::new(r"([0-9]+(?:\.[0-9]+)?) (ps|ns|µs|μs|ms|s)").unwrap()
});

/// Parse divan's tree table into `divan:<module>::<bench>::<arg>` medians.
fn parse_divan_output(output: &str) -> BTreeMap<String, f64> {
    let mut medians = BTreeMap::new();
    let mut stack: Vec<&str> = Vec::new();
    for line in output.lines() {
        // The root line carries the column headers; its name is the benchmark
        // binary and not part of a benchmark's path.
        if line.contains("fastest") && line.contains("median") {
            continue;
        }
        if line.contains("(ignored)") {
            continue;
        }
        let Some((depth, rest)) = split_divan_tree_prefix(line) else {
            continue;
        };
        let name = rest.split("  ").next().unwrap_or_default().trim();
        if name.is_empty() {
            continue;
        }
        stack.truncate(depth);

        let times: Vec<f64> = TIME_RE
            .captures_iter(rest)
            .map(|captures| parse_duration(&captures[1], &captures[2]))
            .collect();
        if times.len() == 4 {
            let mut full_name = stack.join("::");
            if !full_name.is_empty() {
                full_name.push_str("::");
            }
            full_name.push_str(name);
            medians.insert(format!("divan:{full_name}"), times[2]);
        } else {
            stack.push(name);
        }
    }
    medians
}

/// Split `│  ├─ name ...` into its depth and the text after the branch marker.
fn split_divan_tree_prefix(line: &str) -> Option<(usize, &str)> {
    let mut rest = line;
    let mut depth = 0;
    while let Some(stripped) = rest.strip_prefix("│  ").or_else(|| rest.strip_prefix("   ")) {
        rest = stripped;
        depth += 1;
    }
    let rest = rest.strip_prefix("├─ ").or_else(|| rest.strip_prefix("╰─ "))?;
    Some((depth, rest))
}

fn parse_duration(value: &str, unit: &str) -> f64 {
    let value: f64 = value.parse().expect("divan only prints numeric time values");
    match unit {
        "ps" => value / 1000.0,
        "ns" => value,
        "µs" | "μs" => value * 1000.0,
        "ms" => value * 1_000_000.0,
        "s" => value * 1_000_000_000.0,
        _ => unreachable!("TIME_RE only matches known units"),
    }
}

/// Read the median point estimates from `target/criterion/**/new/estimates.json`.
fn collect_criterion_medians(criterion_dir: &Path) -> Result<BTreeMap<String, f64>> {
    let mut medians = BTreeMap::new();
    if criterion_dir.exists() {
        collect_criterion_medians_recursive(criterion_dir, criterion_dir, &mut medians)?;
    }
    Ok(medians)
}

fn collect_criterion_medians_recursive(
    root: &Path,
    dir: &Path,
    medians: &mut BTreeMap<String, f64>,
) -> Result<()> {
    for entry in
        std::fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))?
    {
        let path = entry?.path();
        if path.is_dir() {
            collect_criterion_medians_recursive(root, &path, medians)?;
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("estimates.json")
            || path.parent().and_then(Path::file_name).and_then(|name| name.to_str()) != Some("new")
        {
            continue;
        }
        let Some(bench_dir) =
            path.parent().and_then(Path::parent).and_then(|dir| dir.strip_prefix(root).ok())
        else {
            continue;
        };
        let name = bench_dir
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("::");
        let json = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        medians.insert(format!("criterion:{name}"), parse_criterion_estimate(&json)?);
    }
    Ok(())
}

fn parse_criterion_estimate(json: &str) -> Result<f64> {
    #[derive(serde::Deserialize)]
    struct Estimates {
        median: Estimate,
    }
    #[derive(serde::Deserialize)]
    struct Estimate {
        point_estimate: f64,
    }
    let estimates: Estimates =
        serde_json::from_str(json).context("Failed to parse criterion estimates.json")?;
    Ok(estimates.median.point_estimate)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Ok,
    Regression,
    New,
    Missing,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Regression => "REGRESSION",
            Self::New => "new",
            Self::Missing => "missing",
        }
    }
}

struct Row {
    name: String,
    median: Option<f64>,
    baseline: Option<f64>,
    status: Status,
}

fn compare(
    medians: &BTreeMap<String, f64>,
    baseline: &BTreeMap<String, f64>,
    tolerance: f64,
    include_missing: bool,
) -> Vec<Row> {
    let mut names: Vec<&String> = medians.keys().collect();
    if include_missing {
        names.extend(baseline.keys().filter(|name| !medians.contains_key(*name)));
        names.sort();
        names.dedup();
    }

    names
        .into_iter()
        .map(|name| {
            let median = medians.get(name).copied();
            let baseline_value = baseline.get(name).copied();
            let status = match (median, baseline_value) {
                (Some(median), Some(baseline)) => {
                    if median > baseline * tolerance {
                        Status::Regression
                    } else {
                        Status::Ok
                    }
                }
                (Some(_), None) => Status::New,
                (None, _) => Status::Missing,
            };
            Row { name: name.clone(), median, baseline: baseline_value, status }
        })
        .collect()
}

fn print_report(rows: &[Row], tolerance: f64) {
    let name_width = rows.iter().map(|row| row.name.len()).max().unwrap_or(0);
    println!(
        "{:<name_width$}  {:>12}  {:>12}  {:>7}  Status",
        "Benchmark", "Median", "Baseline", "Ratio"
    );
    for row in rows {
        let ratio = match (row.median, row.baseline) {
            (Some(median), Some(baseline)) if baseline > 0.0 => {
                format!("{:.2}x", median / baseline)
            }
            _ => "-".to_string(),
        };
        println!(
            "{:<name_width$}  {:>12}  {:>12}  {:>7}  {}",
            row.name,
            row.median.map(format_duration).unwrap_or_else(|| "-".to_string()),
            row.baseline.map(format_duration).unwrap_or_else(|| "-".to_string()),
            ratio,
            row.status.as_str(),
        );
    }
    println!("\nTolerance: {tolerance:.2}x");
}

fn append_step_summary(rows: &[Row], tolerance: f64) -> Result<()> {
    let Ok(summary_path) = std::env::var("GITHUB_STEP_SUMMARY") else {
        return Ok(());
    };
    let mut summary = String::from("### Benchmark medians\n\n");
    summary.push_str("| Benchmark | Median | Baseline | Ratio | Status |\n");
    summary.push_str("| --- | --- | --- | --- | --- |\n");
    for row in rows {
        let ratio = match (row.median, row.baseline) {
            (Some(median), Some(baseline)) if baseline > 0.0 => {
                format!("{:.2}x", median / baseline)
            }
            _ => "-".to_string(),
        };
        summary.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            row.name,
            row.median.map(format_duration).unwrap_or_else(|| "-".to_string()),
            row.baseline.map(format_duration).unwrap_or_else(|| "-".to_string()),
            ratio,
            row.status.as_str(),
        ));
    }
    summary.push_str(&format!("\nTolerance: {tolerance:.2}x\n"));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&summary_path)
        .with_context(|| format!("Failed to open {summary_path}"))?;
    file.write_all(summary.as_bytes()).context("Failed to write the step summary")
}

fn format_duration(nanoseconds: f64) -> String {
    if nanoseconds >= 1_000_000_000.0 {
        format!("{:.3} s", nanoseconds / 1_000_000_000.0)
    } else if nanoseconds >= 1_000_000.0 {
        format!("{:.3} ms", nanoseconds / 1_000_000.0)
    } else if nanoseconds >= 1_000.0 {
        format!("{:.3} µs", nanoseconds / 1_000.0)
    } else {
        format!("{nanoseconds:.3} ns")
    }
}

#[derive(Default)]
struct Baseline {
    tolerance: Option<f64>,
    medians: BTreeMap<String, f64>,
}

fn load_baseline(path: &Path) -> Result<Baseline> {
    if !path.exists() {
        return Ok(Baseline::default());
    }
    let doc = parse_toml(path)?;
    let tolerance = doc
        .get("settings")
        .and_then(Item::as_table)
        .and_then(|settings| settings.get("tolerance"))
        .and_then(item_as_f64);
    Ok(Baseline { tolerance, medians: medians_from_doc(&doc) })
}

fn load_medians(path: &Path) -> Result<BTreeMap<String, f64>> {
    Ok(medians_from_doc(&parse_toml(path)?))
}

fn parse_toml(path: &Path) -> Result<DocumentMut> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    text.parse().with_context(|| format!("Failed to parse {}", path.display()))
}

fn medians_from_doc(doc: &DocumentMut) -> BTreeMap<String, f64> {
    doc.get("medians")
        .and_then(Item::as_table)
        .map(|table| {
            table
                .iter()
                .filter_map(|(name, item)| item_as_f64(item).map(|value| (name.to_string(), value)))
                .collect()
        })
        .unwrap_or_default()
}

fn item_as_f64(item: &Item) -> Option<f64> {
    item.as_float().or_else(|| item.as_integer().map(|value| value as f64))
}

fn write_medians(path: &Path, medians: &BTreeMap<String, f64>) -> Result<()> {
    let mut doc = DocumentMut::new();
    doc["medians"] = Item::Table(medians_table(medians));
    write_doc(path, &doc)
}

fn update_baseline(path: &Path, medians: &BTreeMap<String, f64>, replace: bool) -> Result<()> {
    let mut doc = if path.exists() { parse_toml(path)? } else { DocumentMut::new() };
    let table = doc["medians"].as_table_mut().context("'medians' is not a table")?;
    if replace {
        table.clear();
    }
    for (name, value) in medians {
        table.insert(name, toml_edit::value(*value));
    }
    write_doc(path, &doc)
}

fn medians_table(medians: &BTreeMap<String, f64>) -> toml_edit::Table {
    let mut table = toml_edit::Table::new();
    for (name, value) in medians {
        table.insert(name, toml_edit::value(*value));
    }
    table
}

fn write_doc(path: &Path, doc: &DocumentMut) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    std::fs::write(path, doc.to_string())
        .with_context(|| format!("Failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIVAN_OUTPUT: &str = r#"semantic_analysis    fastest       │ slowest       │ median        │ mean          │ samples │ iters
├─ full_compilation                │               │               │               │         │
│  ╰─ many_children                │               │               │               │         │
│     ├─ 10          667.5 µs      │ 1.684 ms      │ 835.1 µs      │ 1.062 ms      │ 3       │ 3
│     │              max alloc:    │               │               │               │         │
│     │                810         │ 1122          │ 810           │ 914           │         │
│     │                466.4 KB    │ 505.3 KB      │ 466.4 KB      │ 479.4 KB      │         │
│     ╰─ 200         1.397 ms      │ 1.491 ms      │ 1.475 ms      │ 1.454 ms      │ 3       │ 3
╰─ parsing                         │               │               │               │         │
   ├─ ignored_bench (ignored)      │               │               │               │         │
   ╰─ simple_component             12.5 ns        │ 20 ns         │ 15 ns         │ 16 ns         │ 3       │ 3
"#;

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
    }

    #[test]
    fn parses_divan_tree() {
        let medians = parse_divan_output(DIVAN_OUTPUT);
        assert_eq!(medians.len(), 3);
        assert_close(medians["divan:full_compilation::many_children::10"], 835_100.0);
        assert_close(medians["divan:full_compilation::many_children::200"], 1_475_000.0);
        assert_close(medians["divan:parsing::simple_component"], 15.0);
    }

    #[test]
    fn parses_divan_units() {
        assert_close(parse_duration("500", "ps"), 0.5);
        assert_close(parse_duration("1.5", "ns"), 1.5);
        assert_close(parse_duration("2.5", "µs"), 2_500.0);
        assert_close(parse_duration("2.5", "μs"), 2_500.0);
        assert_close(parse_duration("3", "ms"), 3_000_000.0);
        assert_close(parse_duration("1.25", "s"), 1_250_000_000.0);
    }

    #[test]
    fn parses_criterion_estimate() {
        let json = r#"{"median": {"confidence_interval": {"lower_bound": 58.6, "upper_bound": 66.9, "confidence_level": 0.95}, "point_estimate": 63.5, "standard_error": 2.0}}"#;
        assert_close(parse_criterion_estimate(json).unwrap(), 63.5);
    }

    #[test]
    fn compares_against_baseline() {
        let medians = BTreeMap::from([
            ("fast".to_string(), 100.0),
            ("regressed".to_string(), 160.0),
            ("added".to_string(), 10.0),
        ]);
        let baseline = BTreeMap::from([
            ("fast".to_string(), 100.0),
            ("regressed".to_string(), 100.0),
            ("removed".to_string(), 100.0),
        ]);
        let rows = compare(&medians, &baseline, 1.5, true);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].status, Status::New);
        assert_eq!(rows[1].name, "fast");
        assert_eq!(rows[1].status, Status::Ok);
        assert_eq!(rows[2].name, "regressed");
        assert_eq!(rows[2].status, Status::Regression);
        assert_eq!(rows[3].name, "removed");
        assert_eq!(rows[3].status, Status::Missing);
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(0.5), "0.500 ns");
        assert_eq!(format_duration(1_500.0), "1.500 µs");
        assert_eq!(format_duration(2_500_000.0), "2.500 ms");
        assert_eq!(format_duration(1_250_000_000.0), "1.250 s");
    }
}
