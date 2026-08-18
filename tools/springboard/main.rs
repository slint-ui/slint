// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use clap::{Args, Parser, Subcommand};
use i_slint_springboard::project::{ProjectRunTarget, load_project_run_target};
use slint_viewer::{ViewerLogLevel, ViewerRunnerOptions, run_auto_reload_simulator};

mod session_driver;
mod stdio;
mod tui;

#[derive(Debug, Parser)]
#[command(author, version, about, args_conflicts_with_subcommands = true)]
struct Cli {
    /// The Slint project directory. Defaults to the current directory.
    #[arg(value_name = "PROJECT")]
    project: Option<PathBuf>,

    #[command(flatten)]
    launch: LaunchOptions,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Start a development session.
    Start(StartOptions),
    /// List globally known devices.
    Devices,
    /// Serve a headless editor client over standard input and output.
    #[command(hide = true)]
    Serve(ServeOptions),
    /// Run the embedded local viewer on the process main thread.
    #[command(hide = true)]
    ViewerChild(ViewerChildOptions),
}

#[derive(Debug, Args)]
struct StartOptions {
    /// The Slint project directory. Defaults to the current directory.
    #[arg(value_name = "PROJECT", default_value = ".")]
    project: PathBuf,

    #[command(flatten)]
    launch: LaunchOptions,
}

#[derive(Clone, Debug, Default, Args)]
struct LaunchOptions {
    /// Launch the globally last-used device after starting the server.
    #[arg(long, conflicts_with_all = ["device", "ios", "android"])]
    last: bool,
    /// Launch a device by its stable ID after starting the server.
    #[arg(long, value_name = "ID", conflicts_with_all = ["last", "ios", "android"])]
    device: Option<String>,
    /// Launch an iOS Simulator viewer after starting the server.
    #[arg(long, conflicts_with_all = ["last", "device", "android"])]
    ios: bool,
    /// Launch an Android emulator viewer after starting the server.
    #[arg(long, conflicts_with_all = ["last", "device", "ios"])]
    android: bool,
}

impl LaunchOptions {
    #[cfg(test)]
    fn requested(&self) -> bool {
        self.last || self.device.is_some() || self.ios || self.android
    }
}

#[derive(Debug, Args)]
struct ServeOptions {
    /// Use the versioned JSON-lines protocol on standard input and output.
    #[arg(long)]
    stdio: bool,
    /// The Slint project directory. Defaults to the current directory.
    #[arg(value_name = "PROJECT", default_value = ".")]
    project: PathBuf,
}

#[derive(Debug, Args)]
struct ViewerChildOptions {
    /// The Slint entry file.
    #[arg(long, value_name = "FILE")]
    entry: PathBuf,
    /// The exported component to show.
    #[arg(long)]
    component: String,
    /// The Slint widget style.
    #[arg(long)]
    style: Option<String>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .log_internal_errors(false)
        .without_time()
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::level_filters::LevelFilter::WARN.into())
                .from_env_lossy(),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Some(Command::Start(options)) => start(options.project, options.launch),
        Some(Command::Devices) => list_devices(),
        Some(Command::Serve(options)) => serve(options),
        Some(Command::ViewerChild(options)) => viewer_child(options),
        None => start(cli.project.unwrap_or_else(|| PathBuf::from(".")), cli.launch),
    }
}

fn start(project: PathBuf, launch: LaunchOptions) -> Result<()> {
    let target = load_required_project(&project)?;
    let store = i_slint_springboard::DeviceStateStore::from_platform_config()
        .context("Cannot determine the Springboard configuration directory")?;
    let controller = session_driver::ProjectSessionController::new(
        target.clone(),
        store,
        session_driver::ViewerChildCommand::current_executable()?,
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("Failed to start the Springboard async runtime")?;
    runtime.block_on(tui::run(controller, launch))
}

fn list_devices() -> Result<()> {
    let Some(store) = i_slint_springboard::DeviceStateStore::from_platform_config() else {
        bail!("Cannot determine the Springboard configuration directory");
    };
    let loaded = store.load();
    if let Some(warning) = loaded.warning {
        tracing::warn!("{warning}");
    }
    let last = loaded.state.last_used_device.as_ref();
    let local_id = session_driver::LOCAL_VIEWER_DEVICE_ID;
    println!(
        "{}\t{local_id}\tLocal Viewer\tlocal-viewer",
        if last.is_some_and(|last| last.as_str() == local_id) { "*" } else { " " }
    );
    for profile in loaded.state.remembered_devices.values() {
        println!(
            "{}\t{}\t{}\t{:?}",
            if last == Some(&profile.id) { "*" } else { " " },
            profile.id,
            profile.name,
            profile.kind
        );
    }
    Ok(())
}

fn serve(options: ServeOptions) -> Result<()> {
    if !options.stdio {
        bail!("The serve command currently requires --stdio");
    }
    let target = load_required_project(&options.project)?;
    let store = i_slint_springboard::DeviceStateStore::from_platform_config()
        .context("Cannot determine the Springboard configuration directory")?;
    let controller = session_driver::ProjectSessionController::new(
        target,
        store,
        session_driver::ViewerChildCommand::current_executable()?,
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("Failed to start the Springboard async runtime")?;
    runtime.block_on(stdio::serve(controller))
}

fn viewer_child(options: ViewerChildOptions) -> Result<()> {
    let mut runner = ViewerRunnerOptions::new(options.entry, Some(options.component));
    runner.style = options.style;
    runner.log_sink = Some(std::sync::Arc::new(|message| match message.level {
        ViewerLogLevel::Error => tracing::error!("{}", message.message),
        ViewerLogLevel::Information => tracing::info!("{}", message.message),
    }));
    let exit_code = run_auto_reload_simulator(runner)?;
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

fn load_required_project(project: &Path) -> Result<ProjectRunTarget> {
    let project = std::fs::canonicalize(project)
        .with_context(|| format!("Failed to resolve project directory {}", project.display()))?;
    if !project.is_dir() {
        bail!("Project path {} is not a directory", project.display());
    }
    load_project_run_target(&project)?
        .with_context(|| format!("No slint.toml found in {}", project.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_subcommand_defaults_to_the_current_project() {
        let cli = Cli::try_parse_from(["slint-springboard"]).unwrap();

        assert!(cli.command.is_none());
        assert_eq!(cli.project, None);
        assert!(!cli.launch.requested());
    }

    #[test]
    fn start_and_top_level_forms_accept_projects_and_launch_flags() {
        let direct = Cli::try_parse_from(["slint-springboard", "project", "--last"]).unwrap();
        assert_eq!(direct.project, Some(PathBuf::from("project")));
        assert!(direct.launch.last);

        let explicit =
            Cli::try_parse_from(["slint-springboard", "start", "project", "--device", "phone"])
                .unwrap();
        let Some(Command::Start(options)) = explicit.command else { panic!() };
        assert_eq!(options.project, PathBuf::from("project"));
        assert_eq!(options.launch.device.as_deref(), Some("phone"));
    }

    #[test]
    fn launch_flags_are_mutually_exclusive() {
        assert!(Cli::try_parse_from(["slint-springboard", "--last", "--ios"]).is_err());
    }

    #[test]
    fn project_loading_requires_a_valid_manifest() {
        let directory = tempfile::tempdir().unwrap();
        let error = load_required_project(directory.path()).unwrap_err().to_string();

        assert!(error.contains("No slint.toml found"));
    }
}
