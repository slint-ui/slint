// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use slint_editor_mcp::{AnnotationMessage, ChatRegistration, EditorAnnotation};

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct SavedAnnotation {
    pub snapshot: EditorAnnotation,
    pub offset: u32,
    pub unread: bool,
    pub sent: bool,
    #[serde(default)]
    pub replies: Vec<SavedReply>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct SavedReply {
    pub message: AnnotationMessage,
    pub sent: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AnnotationThreadDelivery {
    #[serde(flatten)]
    pub snapshot: EditorAnnotation,
    pub conversation: Vec<AnnotationMessage>,
    pub pending_message_ids: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct SavedAnnotations {
    pub project_root: PathBuf,
    pub chat: Option<ChatRegistration>,
    pub annotations: Vec<SavedAnnotation>,
    pub next_id: u64,
    pub send_error: String,
    pub sources: HashMap<PathBuf, String>,
}

pub(super) struct AnnotationStorage {
    name: String,
}

impl AnnotationStorage {
    pub fn new(project_root: &Path) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        project_root.hash(&mut hasher);
        Self { name: format!("annotations-{:016x}.json", hasher.finish()) }
    }

    pub fn load(&self) -> Option<SavedAnnotations> {
        #[cfg(not(test))]
        let contents =
            i_slint_editor_preview::settings_store::load(super::settings::TOOL_NAME, &self.name)?;
        #[cfg(test)]
        let contents = std::fs::read_to_string(self.test_path()).ok()?;
        serde_json::from_str(&contents)
            .map_err(|error| tracing::warn!("Ignoring malformed editor annotations: {error}"))
            .ok()
    }

    pub fn save(&self, stored: &SavedAnnotations) -> Result<(), String> {
        let contents = serde_json::to_string(stored).map_err(|error| error.to_string())?;
        #[cfg(not(test))]
        {
            i_slint_editor_preview::settings_store::save(
                super::settings::TOOL_NAME,
                &self.name,
                &contents,
            )
            .map_err(|error| error.to_string())
        }
        #[cfg(test)]
        {
            std::fs::write(self.test_path(), contents).map_err(|error| error.to_string())
        }
    }

    #[cfg(test)]
    fn test_path(&self) -> PathBuf {
        std::env::temp_dir().join(format!("slint-editor-{}-{}", std::process::id(), self.name))
    }
}

pub(super) struct AnnotationDelivery {
    pub project_root: PathBuf,
    pub chat: ChatRegistration,
    pub annotations: Vec<AnnotationThreadDelivery>,
}

impl AnnotationDelivery {
    pub fn send(&self) -> Result<(), String> {
        let message = format!(
            "Slint Visual Editor annotations for {}:\n{}",
            self.project_root.display(),
            serde_json::to_string_pretty(&self.annotations).map_err(|error| error.to_string())?
        );
        let output = std::process::Command::new(&self.chat.cli_path)
            .args(["queue", "--thread", &self.chat.thread_id, "--message", &message])
            .current_dir(&self.project_root)
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .filter(|line| !line.trim().is_empty())
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(500)
            .collect::<String>();
        Err(if reason.is_empty() { format!("Codex exited with {}", output.status) } else { reason })
    }
}
