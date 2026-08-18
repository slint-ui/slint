// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Discovery and lifecycle management for iOS Simulator viewers.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use i_slint_springboard::{
    Device, DeviceCapabilities, DeviceId, DeviceKind, DeviceOrigin, DeviceStatus,
    MobileViewerArtifactKind,
};
use serde::Deserialize;
use tokio::process::Command;

use crate::artifacts::{ArtifactCache, ArtifactCacheProgress, ArtifactSource};

pub const IOS_SIMULATOR_DEVICE_PREFIX: &str = "simulator:ios:";
pub const DEFAULT_IOS_VIEWER_BUNDLE_ID: &str = "dev.slint.slint-viewer";
const VIEWER_IDENTITY_RELATIVE_PATH: &str =
    "Library/Application Support/dev.Slint.slint-viewer/installation-id";

/// One iOS Simulator available through CoreSimulator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IosSimulator {
    pub id: DeviceId,
    pub udid: String,
    pub name: String,
    pub runtime: String,
    pub state: IosSimulatorState,
}

impl IosSimulator {
    pub fn to_device(&self) -> Device {
        Device {
            id: self.id.clone(),
            name: format!("{} (iOS {})", self.name, self.runtime),
            kind: DeviceKind::IosSimulator,
            origin: DeviceOrigin::Discovered,
            status: DeviceStatus::Available,
            capabilities: DeviceCapabilities::launchable(),
            version: Some(self.runtime.clone()),
            platform: Some("iOS Simulator".into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IosSimulatorState {
    Booted,
    Shutdown,
    Other,
}

/// Observable work performed while launching a managed iOS viewer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IosLaunchProgress {
    Artifact(ArtifactCacheProgress),
    Booting,
    Installing,
    Launching,
    WaitingForDiscovery,
}

/// The persistent remote-viewer identity launched in one simulator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IosLaunchResult {
    pub simulator_id: DeviceId,
    pub viewer_id: DeviceId,
    pub bundle_id: String,
}

#[derive(Clone, Debug)]
struct CommandPrefix {
    executable: PathBuf,
    prefix_args: Vec<OsString>,
}

impl CommandPrefix {
    fn xcrun_simctl() -> Self {
        Self { executable: "xcrun".into(), prefix_args: vec!["simctl".into()] }
    }

    fn invocation<I, S>(&self, args: I) -> CommandInvocation
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut invocation = CommandInvocation {
            executable: self.executable.clone(),
            args: self.prefix_args.clone(),
        };
        invocation.args.extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
        invocation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommandInvocation {
    executable: PathBuf,
    args: Vec<OsString>,
}

impl CommandInvocation {
    async fn output(&self, operation: &str) -> Result<String> {
        let output = Command::new(&self.executable)
            .args(&self.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| {
                if self.executable == Path::new("xcrun") {
                    "Xcode command-line tools are unavailable. Install Xcode and select it with xcode-select."
                        .to_string()
                } else {
                    format!("Failed to start {}", self.executable.display())
                }
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            bail!("{operation} failed: {}", actionable_simctl_error(&stderr));
        }
        String::from_utf8(output.stdout)
            .with_context(|| format!("{operation} returned non-UTF-8 output"))
    }
}

/// The CoreSimulator and artifact-cache client used by a project session.
#[derive(Clone)]
pub struct IosSimulatorManager {
    simctl: CommandPrefix,
    ditto: CommandPrefix,
    artifacts: ArtifactCache,
}

impl IosSimulatorManager {
    pub fn from_environment() -> Result<Self> {
        if !cfg!(target_os = "macos") {
            bail!("iOS Simulator management requires macOS and Xcode");
        }
        let source = ArtifactSource::from_environment()?;
        Ok(Self {
            simctl: CommandPrefix::xcrun_simctl(),
            ditto: CommandPrefix { executable: "/usr/bin/ditto".into(), prefix_args: Vec::new() },
            artifacts: ArtifactCache::from_platform_cache(source)?,
        })
    }

    pub async fn discover(&self) -> Result<Vec<IosSimulator>> {
        let output = self
            .simctl
            .invocation(["list", "devices", "available", "--json"])
            .output("Listing iOS Simulators")
            .await?;
        parse_simulators(&output)
    }

    pub async fn launch(
        &self,
        simulator: IosSimulator,
        mut progress: impl FnMut(IosLaunchProgress),
    ) -> Result<IosLaunchResult> {
        let artifact = self
            .artifacts
            .prepare(MobileViewerArtifactKind::IosSimulatorApp, std::env::consts::ARCH, |event| {
                progress(IosLaunchProgress::Artifact(event))
            })
            .await?;

        if simulator.state != IosSimulatorState::Booted {
            progress(IosLaunchProgress::Booting);
            let invocation = self.simctl.invocation(["boot", simulator.udid.as_str()]);
            if let Err(error) = invocation.output("Booting the iOS Simulator").await
                && !error.to_string().to_ascii_lowercase().contains("state: booted")
            {
                return Err(error);
            }
        }
        progress(IosLaunchProgress::Booting);
        self.simctl
            .invocation(["bootstatus", simulator.udid.as_str(), "-b"])
            .output("Waiting for the iOS Simulator to finish booting")
            .await?;

        let extracted = tempfile::tempdir().context("Failed to prepare the iOS viewer archive")?;
        self.ditto
            .invocation([
                OsStr::new("-x"),
                OsStr::new("-k"),
                artifact.path.as_os_str(),
                extracted.path().as_os_str(),
            ])
            .output("Extracting the iOS Simulator viewer")
            .await?;
        let app = find_app_bundle(extracted.path()).await?;

        progress(IosLaunchProgress::Installing);
        self.simctl
            .invocation([OsStr::new("install"), OsStr::new(&simulator.udid), app.as_os_str()])
            .output("Installing the iOS Simulator viewer")
            .await?;

        progress(IosLaunchProgress::Launching);
        self.simctl
            .invocation(["launch", simulator.udid.as_str(), artifact.artifact.bundle_id.as_str()])
            .output("Launching the iOS Simulator viewer")
            .await?;

        progress(IosLaunchProgress::WaitingForDiscovery);
        let viewer_id =
            self.wait_for_viewer_identity(&simulator.udid, &artifact.artifact.bundle_id).await?;
        Ok(IosLaunchResult {
            simulator_id: simulator.id,
            viewer_id,
            bundle_id: artifact.artifact.bundle_id,
        })
    }

    pub async fn stop(&self, simulator: &IosSimulator, bundle_id: &str) -> Result<()> {
        let result = self
            .simctl
            .invocation(["terminate", simulator.udid.as_str(), bundle_id])
            .output("Stopping the iOS Simulator viewer")
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(error) => {
                let message = error.to_string().to_ascii_lowercase();
                if message.contains("found nothing to terminate")
                    || message.contains("not running")
                    || message.contains("application not found")
                {
                    Ok(())
                } else {
                    Err(error)
                }
            }
        }
    }

    async fn wait_for_viewer_identity(&self, udid: &str, bundle_id: &str) -> Result<DeviceId> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
        loop {
            if let Ok(container) = self
                .simctl
                .invocation(["get_app_container", udid, bundle_id, "data"])
                .output("Locating the iOS Simulator viewer data")
                .await
            {
                let identity_path =
                    PathBuf::from(container.trim()).join(VIEWER_IDENTITY_RELATIVE_PATH);
                if let Ok(identity) = tokio::fs::read_to_string(identity_path).await {
                    let identity = identity.trim();
                    if !identity.is_empty() {
                        return DeviceId::new(format!("remote:{identity}"))
                            .context("The iOS Simulator viewer returned an invalid identity");
                    }
                }
            }
            if tokio::time::Instant::now() >= deadline {
                bail!(
                    "The iOS Simulator viewer did not finish initializing. Keep the Simulator open and retry."
                );
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
}

#[derive(Deserialize)]
struct SimctlDeviceList {
    devices: BTreeMap<String, Vec<SimctlDevice>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SimctlDevice {
    udid: String,
    name: String,
    state: String,
    #[serde(default = "default_available")]
    is_available: bool,
}

const fn default_available() -> bool {
    true
}

fn parse_simulators(json: &str) -> Result<Vec<IosSimulator>> {
    let parsed: SimctlDeviceList =
        serde_json::from_str(json).context("The simctl device list is malformed")?;
    let mut simulators = Vec::new();
    for (runtime_id, devices) in parsed.devices {
        let Some(runtime) = ios_runtime_version(&runtime_id) else { continue };
        for device in devices.into_iter().filter(|device| device.is_available) {
            simulators.push(IosSimulator {
                id: DeviceId::new(format!("{IOS_SIMULATOR_DEVICE_PREFIX}{}", device.udid))?,
                udid: device.udid,
                name: device.name,
                runtime: runtime.clone(),
                state: match device.state.as_str() {
                    "Booted" => IosSimulatorState::Booted,
                    "Shutdown" => IosSimulatorState::Shutdown,
                    _ => IosSimulatorState::Other,
                },
            });
        }
    }
    simulators.sort_by(|left, right| {
        (left.state != IosSimulatorState::Booted)
            .cmp(&(right.state != IosSimulatorState::Booted))
            .then_with(|| version_key(&right.runtime).cmp(&version_key(&left.runtime)))
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(simulators)
}

fn ios_runtime_version(runtime_id: &str) -> Option<String> {
    runtime_id
        .strip_prefix("com.apple.CoreSimulator.SimRuntime.iOS-")
        .map(|version| version.replace('-', "."))
}

fn version_key(version: &str) -> Vec<u32> {
    version.split('.').map(|part| part.parse().unwrap_or_default()).collect()
}

async fn find_app_bundle(directory: &Path) -> Result<PathBuf> {
    let mut entries = tokio::fs::read_dir(directory)
        .await
        .context("Failed to inspect the extracted iOS viewer archive")?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("app")
            && entry.file_type().await?.is_dir()
        {
            return Ok(path);
        }
    }
    bail!("The iOS viewer archive does not contain an application bundle")
}

fn actionable_simctl_error(stderr: &str) -> String {
    let lowercase = stderr.to_ascii_lowercase();
    if lowercase.contains("unable to find utility") || lowercase.contains("active developer path") {
        return "Xcode is unavailable. Install Xcode and select it with xcode-select.".into();
    }
    if lowercase.contains("runtime") && lowercase.contains("not found") {
        return "The selected iOS Simulator runtime is no longer installed. Install it in Xcode and refresh devices.".into();
    }
    if lowercase.contains("invalid device") || lowercase.contains("no devices are booted") {
        return "The selected iOS Simulator is unavailable. Refresh devices and select an installed simulator.".into();
    }
    if stderr.trim().is_empty() {
        "simctl returned an unknown error".into()
    } else {
        stderr.trim().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEVICE_LIST: &str = r#"{
        "devices": {
            "com.apple.CoreSimulator.SimRuntime.iOS-18-6": [
                {"udid":"old","isAvailable":true,"state":"Shutdown","name":"iPhone 16"}
            ],
            "com.apple.CoreSimulator.SimRuntime.iOS-26-5": [
                {"udid":"booted","isAvailable":true,"state":"Booted","name":"iPhone 17"},
                {"udid":"missing","isAvailable":false,"state":"Shutdown","name":"Missing"}
            ],
            "com.apple.CoreSimulator.SimRuntime.watchOS-26-5": [
                {"udid":"watch","isAvailable":true,"state":"Shutdown","name":"Apple Watch"}
            ]
        }
    }"#;

    #[test]
    fn parses_available_ios_devices_and_prefers_booted_recent_runtimes() {
        let devices = parse_simulators(DEVICE_LIST).unwrap();

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].udid, "booted");
        assert_eq!(devices[0].runtime, "26.5");
        assert_eq!(devices[0].state, IosSimulatorState::Booted);
        assert_eq!(devices[1].runtime, "18.6");
        assert_eq!(devices[0].id.as_str(), "simulator:ios:booted");
    }

    #[test]
    fn simctl_commands_are_constructed_without_shell_interpolation() {
        let command = CommandPrefix::xcrun_simctl();

        assert_eq!(
            command.invocation(["list", "devices", "available", "--json"]),
            CommandInvocation {
                executable: "xcrun".into(),
                args: ["simctl", "list", "devices", "available", "--json"]
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            }
        );
        assert_eq!(
            command.invocation(["install", "sim-id", "/cache/Slint Viewer.app"]).args,
            ["simctl", "install", "sim-id", "/cache/Slint Viewer.app"]
                .into_iter()
                .map(Into::into)
                .collect::<Vec<OsString>>()
        );
    }

    #[test]
    fn common_xcode_failures_are_actionable() {
        assert!(actionable_simctl_error("invalid device: gone").contains("Refresh devices"));
        assert!(actionable_simctl_error("runtime profile not found").contains("Install it"));
        assert!(actionable_simctl_error("unable to find utility simctl").contains("Xcode"));
    }

    #[tokio::test]
    #[ignore = "requires Xcode, an iOS Simulator, and a local viewer artifact mirror"]
    async fn managed_ios_simulator_smoke() {
        let udid = std::env::var("SLINT_SPRINGBOARD_IOS_SIMULATOR_UDID")
            .expect("SLINT_SPRINGBOARD_IOS_SIMULATOR_UDID is required");
        let manager = IosSimulatorManager::from_environment().unwrap();
        let simulator = manager
            .discover()
            .await
            .unwrap()
            .into_iter()
            .find(|simulator| simulator.udid == udid)
            .expect("the requested iOS Simulator was not discovered");

        let result = manager.launch(simulator.clone(), |_| {}).await;
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                let _ = manager.stop(&simulator, DEFAULT_IOS_VIEWER_BUNDLE_ID).await;
                panic!("managed iOS Simulator launch failed: {error:#}");
            }
        };

        assert_eq!(result.simulator_id, simulator.id);
        assert!(result.viewer_id.as_str().starts_with("remote:"));
        manager.stop(&simulator, &result.bundle_id).await.unwrap();
    }
}
