// Copyright © Luke. D Jones <luke@ljones.dev>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use zed::{DownloadedFileType, LanguageServerId};
use zed_extension_api::{self as zed, Architecture, Os, Result};

struct SlintExtension {
    cached_binary_path: Option<String>,
}

impl SlintExtension {
    fn language_server_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> Result<String> {
        // Use a local binary first
        // TODO: this causes a hang on linux
        // if let Some(path) = worktree.which("slint-lsp") {
        //     return Ok(path);
        // }

        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).is_ok_and(|stat| stat.is_file()) {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &zed::LanguageServerInstallationStatus::None,
                );
                return Ok(path.to_owned());
            }
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let release = zed::latest_github_release(
            "slint-ui/slint",
            zed::GithubReleaseOptions { require_assets: true, pre_release: false },
        )?;

        let target = zed::current_platform();

        let target_name = match target {
            (Os::Mac, _) => "slint-lsp-macos",
            (Os::Windows, Architecture::X86) | (Os::Windows, Architecture::X8664) => {
                "slint-lsp-windows"
            }
            (Os::Linux, Architecture::X86) | (Os::Linux, Architecture::X8664) => "slint-lsp-linux",
            (Os::Linux, Architecture::Aarch64) => "slint-lsp-aarch64-unknown-linux-gnu",
            (_, _) => return Err("platform or architecture not supported".to_string()),
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

        let extension_dir = "slint-lsp";
        let binary_path = format!(
            "{extension_dir}/{}/slint-lsp{binary_ext}",
            if target_name == "slint-lsp-aarch64-unknown-linux-gnu" {
                target_name
            } else {
                "slint-lsp"
            },
        );

        if !fs::metadata(&binary_path).is_ok_and(|stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            zed::download_file(&asset.download_url, extension_dir, asset_file_type)
                .map_err(|e| format!("failed to download file: {e}"))?;

            let entries =
                fs::read_dir(".").map_err(|e| format!("failed to list working directory {e}"))?;
            for entry in entries {
                let entry = entry.map_err(|e| format!("failed to load directory entry {e}"))?;
                if entry.file_name().to_str() != Some(extension_dir) {
                    fs::remove_dir_all(entry.path()).ok();
                }
            }

            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::None,
            );
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
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
        Ok(zed::Command {
            command: self.language_server_binary_path(language_server_id, worktree)?,
            args: vec![],
            env: Default::default(),
        })
    }
}

zed::register_extension!(SlintExtension);
