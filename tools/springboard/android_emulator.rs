// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Discovery and lifecycle management for Android emulator viewers.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use i_slint_springboard::{
    Device, DeviceCapabilities, DeviceId, DeviceKind, DeviceOrigin, DeviceStatus,
    MobileViewerArtifactKind,
};
use tokio::process::Command;

use crate::artifacts::{ArtifactCache, ArtifactCacheProgress, ArtifactSource};

pub const ANDROID_EMULATOR_DEVICE_PREFIX: &str = "simulator:android:";
pub const DEFAULT_ANDROID_VIEWER_PACKAGE: &str = "dev.slint.viewer";
const DEFAULT_ANDROID_VIEWER_ACTIVITY: &str = "android.app.NativeActivity";

/// One Android Virtual Device available through the Android SDK.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AndroidEmulator {
    pub id: DeviceId,
    pub avd_name: String,
    pub serial: Option<String>,
}

impl AndroidEmulator {
    pub fn to_device(&self) -> Device {
        Device {
            id: self.id.clone(),
            name: self.avd_name.clone(),
            kind: DeviceKind::AndroidEmulator,
            origin: DeviceOrigin::Discovered,
            status: DeviceStatus::Available,
            capabilities: DeviceCapabilities::launchable(),
            version: None,
            platform: Some("Android Emulator".into()),
        }
    }
}

/// Observable work performed while launching a managed Android viewer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AndroidLaunchProgress {
    Booting,
    Artifact(ArtifactCacheProgress),
    Installing,
    Launching,
    WaitingForDiscovery,
}

/// The emulator and viewer details resolved by one launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AndroidLaunchResult {
    pub emulator: AndroidEmulator,
    pub viewer_name: String,
    pub package: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AndroidTool {
    Adb,
    Emulator,
}

#[derive(Clone, Debug)]
struct CommandPrefix {
    executable: PathBuf,
    prefix_args: Vec<OsString>,
    tool: AndroidTool,
}

impl CommandPrefix {
    fn invocation<I, S>(&self, args: I) -> CommandInvocation
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut invocation = CommandInvocation {
            executable: self.executable.clone(),
            args: self.prefix_args.clone(),
            tool: self.tool,
        };
        invocation.args.extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
        invocation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommandInvocation {
    executable: PathBuf,
    args: Vec<OsString>,
    tool: AndroidTool,
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
            .with_context(|| missing_tool_message(self.tool))?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let detail = if stderr.is_empty() { stdout } else { stderr };
            bail!("{operation} failed: {}", actionable_android_error(&detail));
        }
        Ok(stdout)
    }
}

/// The Android SDK and artifact-cache client used by a project session.
#[derive(Clone)]
pub struct AndroidEmulatorManager {
    adb: CommandPrefix,
    emulator: CommandPrefix,
    artifacts: ArtifactCache,
}

impl AndroidEmulatorManager {
    pub fn from_environment() -> Result<Self> {
        let sdk = android_sdk_root();
        let adb = resolve_sdk_tool(sdk.as_deref(), &["platform-tools"], "adb");
        let emulator = resolve_sdk_tool(sdk.as_deref(), &["emulator"], "emulator");
        let source = ArtifactSource::from_environment();
        Ok(Self {
            adb: CommandPrefix { executable: adb, prefix_args: Vec::new(), tool: AndroidTool::Adb },
            emulator: CommandPrefix {
                executable: emulator,
                prefix_args: Vec::new(),
                tool: AndroidTool::Emulator,
            },
            artifacts: ArtifactCache::from_platform_cache(source)?,
        })
    }

    pub async fn discover(&self) -> Result<Vec<AndroidEmulator>> {
        let running = self.running_emulators().await?;
        let configured = match self
            .emulator
            .invocation(["-list-avds"])
            .output("Listing Android Virtual Devices")
            .await
        {
            Ok(output) => parse_avd_list(&output),
            Err(_) if !running.is_empty() => BTreeSet::new(),
            Err(error) => return Err(error),
        };
        let mut by_name = configured
            .into_iter()
            .map(|name| (name.clone(), AndroidEmulator::stopped(name)))
            .collect::<BTreeMap<_, _>>();
        for (serial, avd_name) in running {
            by_name
                .entry(avd_name.clone())
                .and_modify(|emulator| emulator.serial = Some(serial.clone()))
                .or_insert_with(|| AndroidEmulator::running(avd_name, serial));
        }
        let mut emulators = by_name.into_values().collect::<Vec<_>>();
        emulators.sort_by(|left, right| {
            left.serial
                .is_none()
                .cmp(&right.serial.is_none())
                .then_with(|| left.avd_name.cmp(&right.avd_name))
        });
        Ok(emulators)
    }

    pub async fn launch(
        &self,
        mut emulator: AndroidEmulator,
        mut progress: impl FnMut(AndroidLaunchProgress),
    ) -> Result<AndroidLaunchResult> {
        let serial = match emulator.serial.clone() {
            Some(serial) => serial,
            None => {
                progress(AndroidLaunchProgress::Booting);
                self.start_emulator(&emulator.avd_name).await?;
                self.wait_for_emulator(&emulator.avd_name).await?
            }
        };
        emulator.serial = Some(serial.clone());
        progress(AndroidLaunchProgress::Booting);
        self.wait_for_boot(&serial).await?;

        let abi = self.adb_value(&serial, ["shell", "getprop", "ro.product.cpu.abi"]).await?;
        let artifact = self
            .artifacts
            .prepare(MobileViewerArtifactKind::AndroidApk, abi.trim(), |event| {
                progress(AndroidLaunchProgress::Artifact(event))
            })
            .await?;

        progress(AndroidLaunchProgress::Installing);
        let install = self
            .adb_for(&serial, ["install", "-r", artifact.path.to_string_lossy().as_ref()])
            .output("Installing the Android viewer")
            .await?;
        if install.to_ascii_lowercase().contains("failure") {
            bail!("Installing the Android viewer failed: {}", actionable_android_error(&install));
        }

        progress(AndroidLaunchProgress::Launching);
        let component =
            format!("{}/{}", artifact.artifact.bundle_id, DEFAULT_ANDROID_VIEWER_ACTIVITY);
        let launch = self
            .adb_for(&serial, ["shell", "am", "start", "-n", component.as_str()])
            .output("Launching the Android viewer")
            .await?;
        if launch.to_ascii_lowercase().contains("error type") {
            bail!("Launching the Android viewer failed: {launch}");
        }

        progress(AndroidLaunchProgress::WaitingForDiscovery);
        let viewer_name =
            self.viewer_name(&serial).await.unwrap_or_else(|_| emulator.avd_name.clone());
        Ok(AndroidLaunchResult { emulator, viewer_name, package: artifact.artifact.bundle_id })
    }

    pub async fn stop(&self, emulator: &AndroidEmulator, package: &str) -> Result<()> {
        let serial = match &emulator.serial {
            Some(serial) => Some(serial.clone()),
            None => {
                self.running_emulators().await?.into_iter().find_map(|(serial, avd_name)| {
                    (avd_name == emulator.avd_name).then_some(serial)
                })
            }
        };
        let Some(serial) = serial else { return Ok(()) };
        let result = self
            .adb_for(&serial, ["shell", "am", "force-stop", package])
            .output("Stopping the Android viewer")
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(error) => {
                let message = error.to_string().to_ascii_lowercase();
                if message.contains("offline")
                    || message.contains("not found")
                    || message.contains("no devices")
                {
                    Ok(())
                } else {
                    Err(error)
                }
            }
        }
    }

    async fn running_emulators(&self) -> Result<Vec<(String, String)>> {
        let output =
            self.adb.invocation(["devices", "-l"]).output("Listing Android devices").await?;
        let devices = parse_adb_devices(&output)?;
        let mut running = Vec::new();
        for serial in devices {
            let name = self
                .adb_for(&serial, ["emu", "avd", "name"])
                .output("Reading the running Android Virtual Device name")
                .await?;
            let Some(name) = parse_running_avd_name(&name) else {
                bail!("The running Android emulator {serial} did not report its AVD name");
            };
            running.push((serial, name));
        }
        Ok(running)
    }

    async fn start_emulator(&self, avd_name: &str) -> Result<()> {
        let invocation = self.emulator.invocation(["-avd", avd_name]);
        let mut child = Command::new(&invocation.executable)
            .args(&invocation.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(false)
            .spawn()
            .with_context(|| missing_tool_message(AndroidTool::Emulator))?;
        tokio::time::sleep(Duration::from_millis(250)).await;
        if let Some(status) = child.try_wait().context("Failed to inspect the Android emulator")? {
            if status.success() {
                return Ok(());
            }
            bail!(
                "Starting Android Virtual Device {avd_name} failed with {status}. Open it in Android Studio for detailed diagnostics."
            );
        }
        tokio::spawn(async move {
            let _ = child.wait().await;
        });
        Ok(())
    }

    async fn wait_for_emulator(&self, avd_name: &str) -> Result<String> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
        loop {
            if let Ok(running) = self.running_emulators().await
                && let Some((serial, _)) =
                    running.into_iter().find(|(_, candidate)| candidate == avd_name)
            {
                return Ok(serial);
            }
            if tokio::time::Instant::now() >= deadline {
                bail!(
                    "The Android emulator {avd_name} did not appear in ADB. Check the emulator window and retry."
                );
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    async fn wait_for_boot(&self, serial: &str) -> Result<()> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(180);
        loop {
            match self.adb_value(serial, ["shell", "getprop", "sys.boot_completed"]).await {
                Ok(value) if value.trim() == "1" => return Ok(()),
                Ok(_) | Err(_) => {}
            }
            if tokio::time::Instant::now() >= deadline {
                bail!(
                    "The Android emulator did not finish booting. Check the emulator window and Android SDK acceleration settings."
                );
            }
            tokio::time::sleep(Duration::from_millis(750)).await;
        }
    }

    async fn viewer_name(&self, serial: &str) -> Result<String> {
        for property in ["shell settings get global device_name", "shell getprop ro.product.model"]
        {
            let arguments = property.split_whitespace().collect::<Vec<_>>();
            let value = self.adb_value(serial, arguments).await?;
            let value = value.trim();
            if !value.is_empty() && value != "null" {
                return Ok(value.into());
            }
        }
        bail!("The Android emulator did not report a device name")
    }

    async fn adb_value<I, S>(&self, serial: &str, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.adb_for(serial, args).output("Querying the Android emulator").await
    }

    fn adb_for<I, S>(&self, serial: &str, args: I) -> CommandInvocation
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut arguments = vec![OsString::from("-s"), OsString::from(serial)];
        arguments.extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
        self.adb.invocation(arguments)
    }
}

impl AndroidEmulator {
    fn stopped(avd_name: String) -> Self {
        Self { id: android_emulator_id(&avd_name), avd_name, serial: None }
    }

    fn running(avd_name: String, serial: String) -> Self {
        Self { id: android_emulator_id(&avd_name), avd_name, serial: Some(serial) }
    }
}

fn android_emulator_id(avd_name: &str) -> DeviceId {
    DeviceId::new(format!("{ANDROID_EMULATOR_DEVICE_PREFIX}{avd_name}")).unwrap()
}

fn parse_avd_list(output: &str) -> BTreeSet<String> {
    output.lines().map(str::trim).filter(|line| !line.is_empty()).map(Into::into).collect()
}

fn parse_adb_devices(output: &str) -> Result<Vec<String>> {
    let mut emulators = Vec::new();
    for line in output.lines().map(str::trim) {
        let mut fields = line.split_whitespace();
        let Some(serial) = fields.next() else { continue };
        if !serial.starts_with("emulator-") {
            continue;
        }
        let state = fields.next().unwrap_or_default();
        match state {
            "device" => emulators.push(serial.into()),
            "unauthorized" => bail!(
                "Android emulator {serial} is not authorized for ADB. Unlock it and accept the debugging prompt."
            ),
            "offline" => bail!(
                "Android emulator {serial} is offline. Restart it from Android Studio and refresh devices."
            ),
            _ => {}
        }
    }
    Ok(emulators)
}

fn parse_running_avd_name(output: &str) -> Option<String> {
    output.lines().map(str::trim).find(|line| !line.is_empty() && *line != "OK").map(Into::into)
}

fn android_sdk_root() -> Option<PathBuf> {
    ["ANDROID_SDK_ROOT", "ANDROID_HOME"]
        .into_iter()
        .filter_map(|variable| std::env::var_os(variable))
        .find(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(default_android_sdk_root)
}

fn default_android_sdk_root() -> Option<PathBuf> {
    #[cfg(not(target_os = "windows"))]
    let home = directories::BaseDirs::new()?.home_dir().to_owned();
    #[cfg(target_os = "macos")]
    return Some(home.join("Library/Android/sdk"));
    #[cfg(target_os = "windows")]
    return std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|directory| directory.join("Android/Sdk"));
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return Some(home.join("Android/Sdk"));
}

fn resolve_sdk_tool(sdk: Option<&Path>, subdirectories: &[&str], name: &str) -> PathBuf {
    let executable = if cfg!(target_os = "windows") { format!("{name}.exe") } else { name.into() };
    if let Some(sdk) = sdk {
        let candidate = subdirectories
            .iter()
            .fold(sdk.to_owned(), |path, segment| path.join(segment))
            .join(&executable);
        if candidate.is_file() {
            return candidate;
        }
    }
    executable.into()
}

fn missing_tool_message(tool: AndroidTool) -> &'static str {
    match tool {
        AndroidTool::Adb => {
            "ADB is unavailable. Install Android SDK Platform-Tools and set ANDROID_SDK_ROOT."
        }
        AndroidTool::Emulator => {
            "The Android emulator tool is unavailable. Install it with Android Studio and set ANDROID_SDK_ROOT."
        }
    }
}

fn actionable_android_error(message: &str) -> String {
    let lowercase = message.to_ascii_lowercase();
    if lowercase.contains("unauthorized") {
        return "ADB is not authorized. Unlock the emulator and accept the debugging prompt."
            .into();
    }
    if lowercase.contains("offline") {
        return "The Android emulator is offline. Restart it from Android Studio and retry.".into();
    }
    if lowercase.contains("update_incompatible") || lowercase.contains("signatures do not match") {
        return "The installed viewer has a different signature. Uninstall it from the emulator, then retry; this removes that viewer installation's saved identity.".into();
    }
    if lowercase.contains("insufficient_storage") || lowercase.contains("no space left") {
        return "The Android emulator does not have enough storage for the viewer.".into();
    }
    if lowercase.contains("hardware acceleration") || lowercase.contains("hypervisor") {
        return "Android emulator acceleration is unavailable. Enable the platform hypervisor and retry.".into();
    }
    if message.trim().is_empty() {
        "the Android SDK command returned an unknown error".into()
    } else {
        message.trim().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADB_DEVICES: &str = "List of devices attached\nemulator-5554 device product:sdk model:Pixel_9\n0123456789ABCDEF device product:phone\nemulator-5556 offline\n";

    #[test]
    fn parses_configured_avds_and_running_emulators() {
        assert_eq!(
            parse_avd_list("Pixel_9_API_36\nTablet_API_35\n\n"),
            BTreeSet::from(["Pixel_9_API_36".into(), "Tablet_API_35".into()])
        );
        assert_eq!(
            parse_running_avd_name("Pixel_9_API_36\nOK\n").as_deref(),
            Some("Pixel_9_API_36")
        );

        let error = parse_adb_devices(ADB_DEVICES).unwrap_err().to_string();
        assert!(error.contains("offline"));
        assert_eq!(
            parse_adb_devices("List of devices attached\nemulator-5554 device\nphone device\n")
                .unwrap(),
            ["emulator-5554"]
        );
    }

    #[test]
    fn commands_are_constructed_without_shell_interpolation() {
        let adb = CommandPrefix {
            executable: "adb".into(),
            prefix_args: Vec::new(),
            tool: AndroidTool::Adb,
        };
        let manager = AndroidEmulatorManager {
            adb,
            emulator: CommandPrefix {
                executable: "emulator".into(),
                prefix_args: Vec::new(),
                tool: AndroidTool::Emulator,
            },
            artifacts: ArtifactCache::new(
                PathBuf::from("/cache"),
                ArtifactSource::new(PathBuf::from("/viewer-artifacts")).unwrap(),
            )
            .unwrap(),
        };

        assert_eq!(
            manager.adb_for("emulator-5554", ["install", "-r", "/tmp/viewer.apk"]),
            CommandInvocation {
                executable: "adb".into(),
                args: ["-s", "emulator-5554", "install", "-r", "/tmp/viewer.apk"]
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                tool: AndroidTool::Adb,
            }
        );
        assert_eq!(
            manager.emulator.invocation(["-avd", "Pixel_9_API_36"]).args,
            ["-avd", "Pixel_9_API_36"].into_iter().map(Into::into).collect::<Vec<OsString>>()
        );
    }

    #[test]
    fn common_android_failures_are_actionable() {
        assert!(actionable_android_error("device unauthorized").contains("debugging prompt"));
        assert!(
            actionable_android_error("INSTALL_FAILED_UPDATE_INCOMPATIBLE").contains("signature")
        );
        assert!(actionable_android_error("No space left on device").contains("storage"));
    }

    #[tokio::test]
    #[ignore = "requires an Android Virtual Device and a local viewer artifact mirror"]
    async fn managed_android_emulator_smoke() {
        let avd_name = std::env::var("SLINT_SPRINGBOARD_ANDROID_AVD")
            .expect("SLINT_SPRINGBOARD_ANDROID_AVD is required");
        let manager = AndroidEmulatorManager::from_environment().unwrap();
        let emulator = manager
            .discover()
            .await
            .unwrap()
            .into_iter()
            .find(|emulator| emulator.avd_name == avd_name)
            .expect("the requested Android Virtual Device was not discovered");

        let result = manager.launch(emulator.clone(), |_| {}).await;
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                let _ = manager.stop(&emulator, DEFAULT_ANDROID_VIEWER_PACKAGE).await;
                panic!("managed Android emulator launch failed: {error:#}");
            }
        };

        assert_eq!(result.emulator.id, emulator.id);
        assert!(!result.viewer_name.is_empty());
        manager.stop(&result.emulator, &result.package).await.unwrap();
    }
}
