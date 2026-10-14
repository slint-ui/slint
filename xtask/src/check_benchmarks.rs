// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Compare benchmark medians and memory usage against a baseline.
//!
//! Both suites use divan with its allocation profiler, so each benchmark
//! reports a median wall-clock time plus per-iteration peak and total
//! allocated bytes. The baseline stores those values in nanoseconds and bytes;
//! a value fails when it exceeds its baseline entry by more than the
//! corresponding tolerance. A missing baseline entry is only a warning, so
//! adding a benchmark doesn't fail CI before the baseline is refreshed.
//!
//! Run `cargo xtask check_benchmarks --save-baseline` to record the measured
//! values, or `--from-results target/benchmark-results.toml --save-baseline`
//! to seed the baseline from a CI artifact without running the benchmarks.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use regex::Regex;
use toml_edit::{DocumentMut, Item};

/// Fail when a median is this much slower than its baseline, unless the
/// baseline's `[settings] tolerance` overrides it.
const DEFAULT_TOLERANCE: f64 = 1.2;

/// Peak and total allocated bytes are deterministic, so they get a tighter
/// default than execution time; the baseline's `[settings] memory_tolerance`
/// overrides it.
const DEFAULT_MEMORY_TOLERANCE: f64 = 1.1;

/// The core crate's string benchmarks run in tens of nanoseconds, where divan's
/// automatic sample sizing depends on the slowest outlier and can flip between
/// one and many iterations per sample. Pinning it keeps their times stable.
const CORE_SAMPLE_SIZE: u32 = 32;

/// Ignore time differences smaller than this many nanoseconds, unless the
/// baseline's `[settings] min_time_delta_ns` overrides it. Nanosecond-scale
/// benchmarks drift more than the tolerance on shared runners, while a real
/// regression is much larger. Memory sizes are exact, so they have no floor.
const DEFAULT_MIN_TIME_DELTA_NS: f64 = 100.0;

#[derive(Debug, clap::Parser)]
pub struct CheckBenchmarks {
    /// Baseline to compare the measured values against; times are in
    /// nanoseconds, memory in bytes.
    #[arg(long, value_name = "PATH", default_value = "xtask/benchmark-baseline.toml")]
    baseline: PathBuf,

    /// Fail when a median exceeds its baseline value by more than this factor.
    /// Overrides the setting in the baseline file.
    #[arg(long, value_name = "FACTOR")]
    tolerance: Option<f64>,

    /// Fail when peak or total allocated bytes exceed their baseline value by
    /// more than this factor. Overrides the setting in the baseline file.
    #[arg(long, value_name = "FACTOR")]
    memory_tolerance: Option<f64>,

    /// Number of samples divan collects per benchmark.
    #[arg(long, value_name = "N", default_value_t = 100)]
    sample_count: u32,

    /// Run only benchmarks whose name matches this regex.
    #[arg(long, value_name = "PATTERN")]
    filter: Option<String>,

    /// Repeat both suites this many times and keep the smallest values.
    #[arg(long, value_name = "N", default_value_t = 1)]
    repeat: u32,

    /// Read the measurements from this file instead of running the benchmarks.
    #[arg(long, value_name = "PATH", conflicts_with = "repeat")]
    from_results: Option<PathBuf>,

    /// Report the comparison without failing.
    #[arg(long)]
    report_only: bool,

    /// Write the measured values to the baseline instead of comparing.
    #[arg(long)]
    save_baseline: bool,

    /// Write the measured values to this file.
    #[arg(long, value_name = "PATH", default_value = "target/benchmark-results.toml")]
    results: PathBuf,
}

/// One benchmark's values: time in nanoseconds, memory in bytes.
#[derive(Debug, Clone, Copy, Default)]
struct Measurement {
    median_ns: Option<f64>,
    max_alloc_bytes: Option<f64>,
    alloc_bytes: Option<f64>,
}

type Measurements = BTreeMap<String, Measurement>;

impl Measurement {
    fn set_median(&mut self, value: f64) {
        self.median_ns = Some(value);
    }

    fn set_max_alloc(&mut self, value: f64) {
        self.max_alloc_bytes = Some(value);
    }

    fn set_alloc(&mut self, value: f64) {
        self.alloc_bytes = Some(value);
    }
}

impl CheckBenchmarks {
    pub fn run(&self) -> Result<()> {
        if self.repeat == 0 {
            bail!("--repeat must be at least 1");
        }

        let measurements = if let Some(path) = &self.from_results {
            load_measurements(&rooted(path))
                .with_context(|| format!("Failed to read {}", path.display()))?
        } else {
            self.measure()?
        };
        if measurements.is_empty() {
            bail!("No benchmark results collected");
        }

        let results_path = rooted(&self.results);
        write_measurements(&results_path, &measurements)?;
        println!("Wrote {} benchmark results to {}\n", measurements.len(), results_path.display());

        if self.save_baseline {
            let baseline_path = rooted(&self.baseline);
            update_baseline(&baseline_path, &measurements, self.filter.is_none())?;
            println!("Wrote baseline to {}", baseline_path.display());
            return Ok(());
        }

        let baseline = load_baseline(&rooted(&self.baseline))?;
        if baseline.measurements.is_empty() {
            eprintln!(
                "warning: no baseline entries in {}; every benchmark is reported as new",
                self.baseline.display()
            );
        }
        let time_tolerance = self.tolerance.or(baseline.tolerance).unwrap_or(DEFAULT_TOLERANCE);
        let memory_tolerance =
            self.memory_tolerance.or(baseline.memory_tolerance).unwrap_or(DEFAULT_MEMORY_TOLERANCE);
        let min_time_delta_ns = baseline.min_time_delta_ns.unwrap_or(DEFAULT_MIN_TIME_DELTA_NS);
        let rows = compare(
            &measurements,
            &baseline.measurements,
            time_tolerance,
            memory_tolerance,
            min_time_delta_ns,
            self.filter.is_none(),
        );
        print_report(&rows, time_tolerance, memory_tolerance, min_time_delta_ns);
        append_step_summary(&rows, time_tolerance, memory_tolerance, min_time_delta_ns)?;

        let regressions = rows.iter().filter(|row| row.has_regression()).count();
        if regressions > 0 && !self.report_only {
            bail!("{regressions} benchmark(s) exceed their baseline by more than the tolerance");
        }
        Ok(())
    }

    fn measure(&self) -> Result<Measurements> {
        let mut measurements = Measurements::new();
        for _ in 0..self.repeat {
            merge_min(
                &mut measurements,
                self.measure_divan("i-slint-compiler", "semantic_analysis", &["rust"], None)?,
            );
            merge_min(
                &mut measurements,
                self.measure_divan("i-slint-core", "string", &[], Some(CORE_SAMPLE_SIZE))?,
            );
        }
        Ok(measurements)
    }

    fn measure_divan(
        &self,
        package: &str,
        bench: &str,
        features: &[&str],
        sample_size: Option<u32>,
    ) -> Result<Measurements> {
        let sample_count = self.sample_count.to_string();
        let sample_size = sample_size.map(|size| size.to_string());
        let mut args = vec!["bench", "--locked", "-p", package];
        if !features.is_empty() {
            args.push("--features");
            args.extend(features.iter().copied());
        }
        args.extend(["--bench", bench, "--"]);
        if let Some(filter) = &self.filter {
            args.push(filter);
        }
        args.extend(["--sample-count", &sample_count]);
        if let Some(sample_size) = &sample_size {
            args.extend(["--sample-size", sample_size]);
        }
        let output = run_cargo_bench(&args)?;
        Ok(parse_divan_output(&output))
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

fn merge_min(target: &mut Measurements, source: Measurements) {
    for (name, value) in source {
        match target.entry(name) {
            Entry::Vacant(entry) => {
                entry.insert(value);
            }
            Entry::Occupied(mut entry) => {
                let slot = entry.get_mut();
                slot.median_ns = min_option(slot.median_ns, value.median_ns);
                slot.max_alloc_bytes = min_option(slot.max_alloc_bytes, value.max_alloc_bytes);
                slot.alloc_bytes = min_option(slot.alloc_bytes, value.alloc_bytes);
            }
        }
    }
}

fn min_option(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

static TIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    // µs is U+00B5 MICRO SIGN; accept U+03BC GREEK SMALL LETTER MU as well.
    Regex::new(r"([0-9]+(?:\.[0-9]+)?) (ps|ns|µs|μs|ms|s)").unwrap()
});

static BYTES_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"([0-9]+(?:\.[0-9]+)?) (B|KB|MB|GB|TB|PB|KiB|MiB|GiB|TiB|PiB)").unwrap()
});

/// The trailing `samples` and `iters` columns of a divan timing row.
static SAMPLES_ITERS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+)\s*│\s*(\d+)\s*$").unwrap());

/// Parse divan's tree table into `divan:<module>::<bench>::<arg>` measurements.
fn parse_divan_output(output: &str) -> Measurements {
    let lines: Vec<&str> = output.lines().collect();
    let mut measurements = Measurements::new();
    let mut stack: Vec<&str> = Vec::new();
    let mut current_leaf: Option<String> = None;
    let mut current_sample_size = 1.0;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];

        // The root line carries the column headers; its name is the benchmark
        // binary and not part of a benchmark's path.
        if line.contains("fastest") && line.contains("median") {
            current_leaf = None;
            index += 1;
            continue;
        }
        if line.contains("(ignored)") {
            index += 1;
            continue;
        }

        // Allocation statistics follow the leaf's timing row: a label row, a
        // row of operation counts, and a row of byte sizes.
        if let Some(label) = divan_row_label(line) {
            let metric = match label {
                "max alloc:" => Some(MemoryMetric::MaxAlloc),
                "alloc:" => Some(MemoryMetric::Alloc),
                _ => None,
            };
            if let (Some(metric), Some(leaf)) = (metric, current_leaf.as_ref()) {
                if let Some(bytes) =
                    lines.get(index + 2).and_then(|line| divan_alloc_row_median(line))
                {
                    let measurement = measurements.entry(leaf.clone()).or_default();
                    match metric {
                        // Divan divides the sample's peak by the sample size,
                        // so undo that to get the per-iteration peak.
                        MemoryMetric::MaxAlloc => {
                            measurement.max_alloc_bytes = Some(bytes * current_sample_size)
                        }
                        MemoryMetric::Alloc => measurement.alloc_bytes = Some(bytes),
                    }
                }
                // Skip the label, count, and size rows.
                index += 3;
                continue;
            }
        }

        let Some((depth, rest)) = split_divan_tree_prefix(line) else {
            index += 1;
            continue;
        };
        let name = rest.split("  ").next().unwrap_or_default().trim();
        if name.is_empty() {
            index += 1;
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
            let full_name = format!("divan:{full_name}");
            measurements.entry(full_name.clone()).or_default().set_median(times[2]);
            current_leaf = Some(full_name);
            current_sample_size = sample_size_of(rest);
        } else {
            stack.push(name);
            current_leaf = None;
            current_sample_size = 1.0;
        }
        index += 1;
    }
    measurements
}

enum MemoryMetric {
    MaxAlloc,
    Alloc,
}

/// The iterations per sample of a divan timing row, derived from the trailing
/// `samples` and `iters` columns.
fn sample_size_of(row: &str) -> f64 {
    SAMPLES_ITERS_RE
        .captures(row)
        .and_then(|captures| {
            let samples: f64 = captures[1].parse().ok()?;
            let iters: f64 = captures[2].parse().ok()?;
            (samples > 0.0).then_some(iters / samples)
        })
        .unwrap_or(1.0)
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

/// Return the label in a row's name column, for example `max alloc:` or
/// `alloc:` on the allocation statistic rows.
fn divan_row_label(line: &str) -> Option<&str> {
    let rest = line.trim_start_matches([' ', '│']);
    let label = rest.split('│').next()?.trim_end();
    if label.is_empty() { None } else { Some(label) }
}

/// Extract the median allocated bytes from a divan allocation size row.
fn divan_alloc_row_median(line: &str) -> Option<f64> {
    let sizes: Vec<f64> = BYTES_RE
        .captures_iter(line)
        .map(|captures| parse_bytes(&captures[1], &captures[2]))
        .collect();
    if sizes.len() == 4 { Some(sizes[2]) } else { None }
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

fn parse_bytes(value: &str, unit: &str) -> f64 {
    let value: f64 = value.parse().expect("divan only prints numeric byte values");
    let scale = match unit {
        "B" => 1.0,
        "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        "PB" => 1e15,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        "PiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => unreachable!("BYTES_RE only matches known units"),
    };
    value * scale
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

fn combine_status(a: Status, b: Status) -> Status {
    match (a, b) {
        (Status::Regression, _) | (_, Status::Regression) => Status::Regression,
        (Status::Missing, _) | (_, Status::Missing) => Status::Missing,
        (Status::New, _) | (_, Status::New) => Status::New,
        _ => Status::Ok,
    }
}

#[derive(Clone, Copy)]
struct Metric {
    measured: Option<f64>,
    baseline: Option<f64>,
    status: Status,
}

impl Metric {
    fn ratio(&self) -> Option<f64> {
        match (self.measured, self.baseline) {
            (Some(measured), Some(baseline)) if baseline > 0.0 => Some(measured / baseline),
            _ => None,
        }
    }
}

struct Row {
    name: String,
    median: Metric,
    max_alloc: Metric,
    alloc: Metric,
}

impl Row {
    fn memory_status(&self) -> Status {
        combine_status(self.max_alloc.status, self.alloc.status)
    }

    fn has_memory(&self) -> bool {
        self.max_alloc.measured.is_some()
            || self.max_alloc.baseline.is_some()
            || self.alloc.measured.is_some()
            || self.alloc.baseline.is_some()
    }

    fn has_regression(&self) -> bool {
        self.median.status == Status::Regression
            || self.max_alloc.status == Status::Regression
            || self.alloc.status == Status::Regression
    }
}

fn metric(measured: Option<f64>, baseline: Option<f64>, tolerance: f64, min_delta: f64) -> Metric {
    let status = match (measured, baseline) {
        (Some(measured), Some(baseline)) => {
            if measured > baseline * tolerance && measured - baseline > min_delta {
                Status::Regression
            } else {
                Status::Ok
            }
        }
        (Some(_), None) => Status::New,
        (None, Some(_)) => Status::Missing,
        (None, None) => Status::Ok,
    };
    Metric { measured, baseline, status }
}

fn compare(
    measurements: &Measurements,
    baseline: &Measurements,
    time_tolerance: f64,
    memory_tolerance: f64,
    min_time_delta_ns: f64,
    include_missing: bool,
) -> Vec<Row> {
    let mut names: Vec<&String> = measurements.keys().collect();
    if include_missing {
        names.extend(baseline.keys().filter(|name| !measurements.contains_key(*name)));
        names.sort();
        names.dedup();
    }

    names
        .into_iter()
        .map(|name| {
            let measured = measurements.get(name);
            let baseline = baseline.get(name);
            Row {
                name: name.clone(),
                median: metric(
                    measured.and_then(|m| m.median_ns),
                    baseline.and_then(|b| b.median_ns),
                    time_tolerance,
                    min_time_delta_ns,
                ),
                max_alloc: metric(
                    measured.and_then(|m| m.max_alloc_bytes),
                    baseline.and_then(|b| b.max_alloc_bytes),
                    memory_tolerance,
                    0.0,
                ),
                alloc: metric(
                    measured.and_then(|m| m.alloc_bytes),
                    baseline.and_then(|b| b.alloc_bytes),
                    memory_tolerance,
                    0.0,
                ),
            }
        })
        .collect()
}

fn print_report(rows: &[Row], time_tolerance: f64, memory_tolerance: f64, min_time_delta_ns: f64) {
    let name_width = rows.iter().map(|row| row.name.len()).max().unwrap_or(0);

    println!("Execution time");
    println!(
        "{:<name_width$}  {:>12}  {:>12}  {:>7}  Status",
        "Benchmark", "Median", "Baseline", "Ratio"
    );
    for row in rows {
        println!(
            "{:<name_width$}  {:>12}  {:>12}  {:>7}  {}",
            row.name,
            format_optional(row.median.measured, format_duration),
            format_optional(row.median.baseline, format_duration),
            format_optional(row.median.ratio(), format_ratio),
            row.median.status.as_str(),
        );
    }

    let memory_rows: Vec<&Row> = rows.iter().filter(|row| row.has_memory()).collect();
    if !memory_rows.is_empty() {
        println!("\nMemory per iteration");
        println!(
            "{:<name_width$}  {:>12}  {:>12}  {:>7}  {:>12}  {:>12}  {:>7}  Status",
            "Benchmark", "Peak", "Baseline", "Ratio", "Allocated", "Baseline", "Ratio"
        );
        for row in memory_rows {
            println!(
                "{:<name_width$}  {:>12}  {:>12}  {:>7}  {:>12}  {:>12}  {:>7}  {}",
                row.name,
                format_optional(row.max_alloc.measured, format_bytes),
                format_optional(row.max_alloc.baseline, format_bytes),
                format_optional(row.max_alloc.ratio(), format_ratio),
                format_optional(row.alloc.measured, format_bytes),
                format_optional(row.alloc.baseline, format_bytes),
                format_optional(row.alloc.ratio(), format_ratio),
                row.memory_status().as_str(),
            );
        }
    }

    println!(
        "\nTolerances: time {time_tolerance:.2}x above {min_time_delta_ns:.0} ns, memory {memory_tolerance:.2}x"
    );
}

fn append_step_summary(
    rows: &[Row],
    time_tolerance: f64,
    memory_tolerance: f64,
    min_time_delta_ns: f64,
) -> Result<()> {
    let Ok(summary_path) = std::env::var("GITHUB_STEP_SUMMARY") else {
        return Ok(());
    };
    let mut summary = String::from("### Benchmark medians\n\n");
    summary.push_str("| Benchmark | Median | Baseline | Ratio | Status |\n");
    summary.push_str("| --- | --- | --- | --- | --- |\n");
    for row in rows {
        summary.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            row.name,
            format_optional(row.median.measured, format_duration),
            format_optional(row.median.baseline, format_duration),
            format_optional(row.median.ratio(), format_ratio),
            row.median.status.as_str(),
        ));
    }

    let memory_rows: Vec<&Row> = rows.iter().filter(|row| row.has_memory()).collect();
    if !memory_rows.is_empty() {
        summary.push_str("\n### Benchmark memory per iteration\n\n");
        summary.push_str(
            "| Benchmark | Peak | Baseline | Ratio | Allocated | Baseline | Ratio | Status |\n",
        );
        summary.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
        for row in memory_rows {
            summary.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | {} | {} | {} |\n",
                row.name,
                format_optional(row.max_alloc.measured, format_bytes),
                format_optional(row.max_alloc.baseline, format_bytes),
                format_optional(row.max_alloc.ratio(), format_ratio),
                format_optional(row.alloc.measured, format_bytes),
                format_optional(row.alloc.baseline, format_bytes),
                format_optional(row.alloc.ratio(), format_ratio),
                row.memory_status().as_str(),
            ));
        }
    }

    summary.push_str(&format!(
        "\nTolerances: time {time_tolerance:.2}x above {min_time_delta_ns:.0} ns, memory {memory_tolerance:.2}x\n"
    ));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&summary_path)
        .with_context(|| format!("Failed to open {summary_path}"))?;
    file.write_all(summary.as_bytes()).context("Failed to write the step summary")
}

fn format_optional(value: Option<f64>, format: impl Fn(f64) -> String) -> String {
    value.map(format).unwrap_or_else(|| "-".to_string())
}

fn format_ratio(ratio: f64) -> String {
    format!("{ratio:.2}x")
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

fn format_bytes(bytes: f64) -> String {
    if bytes >= 1e12 {
        format!("{:.3} TB", bytes / 1e12)
    } else if bytes >= 1e9 {
        format!("{:.3} GB", bytes / 1e9)
    } else if bytes >= 1e6 {
        format!("{:.3} MB", bytes / 1e6)
    } else if bytes >= 1e3 {
        format!("{:.3} KB", bytes / 1e3)
    } else {
        format!("{bytes:.0} B")
    }
}

#[derive(Default)]
struct Baseline {
    tolerance: Option<f64>,
    memory_tolerance: Option<f64>,
    min_time_delta_ns: Option<f64>,
    measurements: Measurements,
}

fn load_baseline(path: &Path) -> Result<Baseline> {
    if !path.exists() {
        return Ok(Baseline::default());
    }
    let doc = parse_toml(path)?;
    let setting = |name: &str| -> Option<f64> {
        doc.get("settings")
            .and_then(Item::as_table)
            .and_then(|settings| settings.get(name))
            .and_then(item_as_f64)
    };
    Ok(Baseline {
        tolerance: setting("tolerance"),
        memory_tolerance: setting("memory_tolerance"),
        min_time_delta_ns: setting("min_time_delta_ns"),
        measurements: measurements_from_doc(&doc),
    })
}

fn load_measurements(path: &Path) -> Result<Measurements> {
    Ok(measurements_from_doc(&parse_toml(path)?))
}

fn parse_toml(path: &Path) -> Result<DocumentMut> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    text.parse().with_context(|| format!("Failed to parse {}", path.display()))
}

fn measurements_from_doc(doc: &DocumentMut) -> Measurements {
    let mut measurements = Measurements::new();
    for (table_name, set_metric) in [
        ("medians", Measurement::set_median as fn(&mut Measurement, f64)),
        ("max_alloc_bytes", Measurement::set_max_alloc),
        ("alloc_bytes", Measurement::set_alloc),
    ] {
        let Some(table) = doc.get(table_name).and_then(Item::as_table) else {
            continue;
        };
        for (name, item) in table.iter() {
            if let Some(value) = item_as_f64(item) {
                set_metric(measurements.entry(name.to_string()).or_default(), value);
            }
        }
    }
    measurements
}

fn item_as_f64(item: &Item) -> Option<f64> {
    item.as_float().or_else(|| item.as_integer().map(|value| value as f64))
}

fn write_measurements(path: &Path, measurements: &Measurements) -> Result<()> {
    let mut doc = DocumentMut::new();
    doc["medians"] = Item::Table(measurement_table(measurements, |m| m.median_ns));
    doc["max_alloc_bytes"] = Item::Table(measurement_table(measurements, |m| m.max_alloc_bytes));
    doc["alloc_bytes"] = Item::Table(measurement_table(measurements, |m| m.alloc_bytes));
    write_doc(path, &doc)
}

fn update_baseline(path: &Path, measurements: &Measurements, replace: bool) -> Result<()> {
    let mut doc = if path.exists() { parse_toml(path)? } else { DocumentMut::new() };
    for (table_name, get_metric) in [
        ("medians", (|m: &Measurement| m.median_ns) as fn(&Measurement) -> Option<f64>),
        ("max_alloc_bytes", |m| m.max_alloc_bytes),
        ("alloc_bytes", |m| m.alloc_bytes),
    ] {
        let table = doc[table_name]
            .as_table_mut()
            .with_context(|| format!("'{table_name}' is not a table"))?;
        if replace {
            table.clear();
        }
        for (name, measurement) in measurements {
            if let Some(value) = get_metric(measurement) {
                table.insert(name, toml_edit::value(value));
            }
        }
    }
    write_doc(path, &doc)
}

fn measurement_table(
    measurements: &Measurements,
    get_metric: fn(&Measurement) -> Option<f64>,
) -> toml_edit::Table {
    let mut table = toml_edit::Table::new();
    for (name, measurement) in measurements {
        if let Some(value) = get_metric(measurement) {
            table.insert(name, toml_edit::value(value));
        }
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
│     │              alloc:        │               │               │               │         │
│     │                1393        │ 1917          │ 1393          │ 1567          │         │
│     │                775.4 KB    │ 830.1 KB      │ 775.4 KB      │ 793.6 KB      │         │
│     │              dealloc:      │               │               │               │         │
│     │                1393        │ 1603          │ 1393          │ 1463          │         │
│     │                809 KB      │ 812.6 KB      │ 809 KB        │ 810.2 KB      │         │
│     ╰─ 200         1.397 ms      │ 1.491 ms      │ 1.475 ms      │ 1.454 ms      │ 3       │ 3
╰─ parsing                         │               │               │               │         │
   ├─ fast_bench                  57.21 ns      │ 1.7 µs        │ 59.21 ns      │ 75.85 ns      │ 100     │ 1600
   │              max alloc:       │               │               │               │         │
   │                0.125          │ 0.125         │ 0.125         │ 0.125         │         │
   │                3.437 B        │ 3.437 B       │ 3.437 B       │ 3.437 B       │         │
   │              alloc:           │               │               │               │         │
   │                2              │ 2             │ 2             │ 2             │         │
   │                55 B           │ 55 B          │ 55 B          │ 55 B          │         │
   ├─ ignored_bench (ignored)      │               │               │               │         │
   ╰─ simple_component             12.5 ns        │ 20 ns         │ 15 ns         │ 16 ns         │ 3       │ 3
"#;

    fn measurement(
        median_ns: f64,
        max_alloc_bytes: Option<f64>,
        alloc_bytes: Option<f64>,
    ) -> Measurement {
        Measurement { median_ns: Some(median_ns), max_alloc_bytes, alloc_bytes }
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
    }

    #[test]
    fn parses_divan_tree() {
        let measurements = parse_divan_output(DIVAN_OUTPUT);
        assert_eq!(measurements.len(), 4);

        let many_children_10 = &measurements["divan:full_compilation::many_children::10"];
        assert_close(many_children_10.median_ns.unwrap(), 835_100.0);
        assert_close(many_children_10.max_alloc_bytes.unwrap(), 466_400.0);
        assert_close(many_children_10.alloc_bytes.unwrap(), 775_400.0);

        let many_children_200 = &measurements["divan:full_compilation::many_children::200"];
        assert_close(many_children_200.median_ns.unwrap(), 1_475_000.0);
        assert_eq!(many_children_200.max_alloc_bytes, None);

        // Divan divides a sample's peak allocation by the sample size; the
        // parser undoes that to report the per-iteration peak.
        let fast_bench = &measurements["divan:parsing::fast_bench"];
        assert_close(fast_bench.median_ns.unwrap(), 59.21);
        assert_close(fast_bench.max_alloc_bytes.unwrap(), 3.437 * 16.0);
        assert_close(fast_bench.alloc_bytes.unwrap(), 55.0);

        let simple_component = &measurements["divan:parsing::simple_component"];
        assert_close(simple_component.median_ns.unwrap(), 15.0);
        assert_eq!(simple_component.max_alloc_bytes, None);
        assert_eq!(simple_component.alloc_bytes, None);
    }

    #[test]
    fn parses_duration_units() {
        assert_close(parse_duration("500", "ps"), 0.5);
        assert_close(parse_duration("1.5", "ns"), 1.5);
        assert_close(parse_duration("2.5", "µs"), 2_500.0);
        assert_close(parse_duration("2.5", "μs"), 2_500.0);
        assert_close(parse_duration("3", "ms"), 3_000_000.0);
        assert_close(parse_duration("1.25", "s"), 1_250_000_000.0);
    }

    #[test]
    fn parses_byte_units() {
        assert_close(parse_bytes("512", "B"), 512.0);
        assert_close(parse_bytes("2.5", "KB"), 2_500.0);
        assert_close(parse_bytes("3", "MB"), 3_000_000.0);
        assert_close(parse_bytes("4", "GiB"), 4.0 * 1024.0 * 1024.0 * 1024.0);
    }

    #[test]
    fn compares_against_baseline() {
        let measurements = Measurements::from([
            ("added".to_string(), measurement(10.0, None, None)),
            ("fast".to_string(), measurement(100.0, Some(1000.0), Some(2000.0))),
            ("grew-memory".to_string(), measurement(100.0, Some(1150.0), Some(2000.0))),
            ("regressed".to_string(), measurement(160.0, Some(1000.0), Some(2000.0))),
        ]);
        let baseline = Measurements::from([
            ("fast".to_string(), measurement(100.0, Some(1000.0), Some(2000.0))),
            ("grew-memory".to_string(), measurement(100.0, Some(1000.0), Some(2000.0))),
            ("regressed".to_string(), measurement(100.0, Some(1000.0), Some(2000.0))),
            ("removed".to_string(), measurement(100.0, Some(1000.0), Some(2000.0))),
        ]);
        let rows = compare(&measurements, &baseline, 1.2, 1.1, 0.0, true);
        assert_eq!(rows.len(), 5);

        assert_eq!(rows[0].name, "added");
        assert_eq!(rows[0].median.status, Status::New);
        assert_eq!(rows[0].memory_status(), Status::Ok);
        assert!(!rows[0].has_memory());

        assert_eq!(rows[1].name, "fast");
        assert_eq!(rows[1].median.status, Status::Ok);
        assert_eq!(rows[1].memory_status(), Status::Ok);
        assert!(!rows[1].has_regression());

        assert_eq!(rows[2].name, "grew-memory");
        assert_eq!(rows[2].median.status, Status::Ok);
        assert_eq!(rows[2].max_alloc.status, Status::Regression);
        assert_eq!(rows[2].memory_status(), Status::Regression);
        assert!(rows[2].has_regression());

        assert_eq!(rows[3].name, "regressed");
        assert_eq!(rows[3].median.status, Status::Regression);
        assert!(rows[3].has_regression());

        assert_eq!(rows[4].name, "removed");
        assert_eq!(rows[4].median.status, Status::Missing);
        assert!(!rows[4].has_regression());
    }

    #[test]
    fn ignores_small_time_deltas() {
        let measurements = Measurements::from([
            ("drifted".to_string(), measurement(160.0, None, None)),
            ("regressed".to_string(), measurement(300.0, None, None)),
        ]);
        let baseline = Measurements::from([
            ("drifted".to_string(), measurement(100.0, None, None)),
            ("regressed".to_string(), measurement(100.0, None, None)),
        ]);
        let rows = compare(&measurements, &baseline, 1.2, 1.1, 100.0, false);
        assert_eq!(rows[0].name, "drifted");
        assert_eq!(rows[0].median.status, Status::Ok);
        assert_eq!(rows[1].name, "regressed");
        assert_eq!(rows[1].median.status, Status::Regression);
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(0.5), "0.500 ns");
        assert_eq!(format_duration(1_500.0), "1.500 µs");
        assert_eq!(format_duration(2_500_000.0), "2.500 ms");
        assert_eq!(format_duration(1_250_000_000.0), "1.250 s");
    }

    #[test]
    fn formats_bytes() {
        assert_eq!(format_bytes(512.0), "512 B");
        assert_eq!(format_bytes(1_500.0), "1.500 KB");
        assert_eq!(format_bytes(2_500_000.0), "2.500 MB");
        assert_eq!(format_bytes(1_250_000_000.0), "1.250 GB");
    }
}
