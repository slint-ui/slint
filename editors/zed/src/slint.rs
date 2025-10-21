// Copyright © Luke. D Jones <luke@ljones.dev>
// SPDX-License-Identifier: MIT

use std::fs;
use zed::{DownloadedFileType, LanguageServerId};
use zed_extension_api::{self as zed, settings::LspSettings, Architecture, Os, Result};

struct SlintBinary {
    path: String,
    args: Option<Vec<String>>,
}

struct SlintExtension {
    cached_binary_path: Option<String>,
}

impl SlintExtension {
    fn language_server_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<SlintBinary> {
        let binary_settings = LspSettings::for_worktree("slint", worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.binary);
        let binary_args =
            binary_settings.as_ref().and_then(|binary_settings| binary_settings.arguments.clone());

        if let Some(path) = binary_settings.and_then(|binary_settings| binary_settings.path) {
            return Ok(SlintBinary { path, args: binary_args });
        }

        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).is_ok_and(|stat| stat.is_file()) {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &zed::LanguageServerInstallationStatus::None,
                );
                return Ok(SlintBinary { path: path.to_owned(), args: binary_args });
            }
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let dev_mode = worktree.shell_env().iter().any(|(k, v)| k == "SLINT_DEV_MODE" && v == "1");
        let release_tag =
            if dev_mode { "nightly" } else { concat!("v", env!("CARGO_PKG_VERSION")) };
        let release = zed::github_release_by_tag_name("slint-ui/slint", release_tag)
            .map_err(|e| format!("Failed to get Slint release {release_tag} from GitHub: {e}",))?;

        let target = zed::current_platform();

        let target_name = match target {
            (Os::Mac, _) => "slint-lsp-macos",
            (Os::Windows, Architecture::X8664) => "slint-lsp-windows-x86_64",
            (Os::Windows, Architecture::Aarch64) => "slint-lsp-windows-arm64",
            (Os::Linux, Architecture::X8664) => "slint-lsp-linux",
            (Os::Linux, Architecture::Aarch64) => "slint-lsp-aarch64-unknown-linux-gnu",
            (_, _) => return Err("Platform or architecture not supported for automatic slint-lsp download. Please install it manually and use the 'lsp.slint.binary' setting to point to it.".to_string()),
        };

        let (asset_file_type, asset_name_ext, binary_ext) = match target.0 {
            Os::Mac | Os::Linux => (DownloadedFileType::GzipTar, ".tar.gz", ""),
            Os::Windows => (DownloadedFileType::Zip, ".zip", ".exe"),
        };

        let asset_name = format!("{target_name}{asset_name_ext}");

        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == asset_name)
            .ok_or_else(|| format!("no asset found matching {:?}", asset_name))?;

        let extension_dir = format!(
            "slint-lsp-{}{}",
            release.version,
            if dev_mode {
                // Add a timestamp to the extension directory to invalidate the cache every day
                (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    / (60 * 60 * 24))
                    .to_string()
            } else {
                String::new()
            }
        );
        // The directory in the tarball is usually named "slint-lsp", but it is different for the slint-lsp-*-linux-*
        let subdir = if target_name == "slint-lsp-aarch64-unknown-linux-gnu" {
            target_name
        } else {
            "slint-lsp"
        };
        let binary_path = format!("{extension_dir}/{subdir}/slint-lsp{binary_ext}",);

        if !fs::metadata(&binary_path).is_ok_and(|stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            zed::download_file(&asset.download_url, &extension_dir, asset_file_type)
                .map_err(|e| format!("failed to download file: {e}"))?;

            let entries =
                fs::read_dir(".").map_err(|e| format!("failed to list working directory {e}"))?;
            for entry in entries {
                let entry = entry.map_err(|e| format!("failed to load directory entry {e}"))?;
                if entry.file_name().to_str() != Some(&extension_dir) {
                    fs::remove_dir_all(entry.path()).ok();
                }
            }

            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::None,
            );
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(SlintBinary { path: binary_path, args: binary_args })
    }
}

impl zed::Extension for SlintExtension {
    fn new() -> Self {
        Self { cached_binary_path: None }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let slint_binary = self.language_server_binary_path(language_server_id, worktree)?;
        Ok(zed::Command {
            command: slint_binary.path,
            args: slint_binary.args.unwrap_or(Vec::new()),
            env: Default::default(),
        })
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<zed::serde_json::Value>> {
        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.settings.clone())
            .unwrap_or_default();
        Ok(Some(settings))
    }
}

zed::register_extension!(SlintExtension);
