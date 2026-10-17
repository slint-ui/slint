// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use lsp_types::Url;

use super::{PreviewTarget, VersionedUrl};

/// The Component to preview
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PreviewComponent {
    /// The file name to preview
    pub url: Url,
    /// The name of the component within that file.
    /// If None, then the last component is going to be shown.
    pub component: Option<String>,
}

#[derive(Default, Clone, PartialEq, Debug, serde::Deserialize, serde::Serialize)]
pub struct PreviewConfig {
    pub hide_ui: Option<bool>,
    #[serde(default, with = "named_overrides")]
    pub overrides: i_slint_compiler::project_file::Overrides,
    pub format_utf8: bool,
}

#[derive(Clone, derive_more::Debug, serde::Deserialize, serde::Serialize)]
pub enum LspToPreviewMessage {
    InvalidateContents {
        url: lsp_types::Url,
    },
    ForgetFile {
        url: lsp_types::Url,
    },
    SetContents {
        url: VersionedUrl,
        #[debug("Vec<u8> {{ len: {} }}", contents.len())]
        contents: Vec<u8>,
    },
    SetConfiguration {
        config: PreviewConfig,
    },
    /// Deliver a stored user-settings blob to the preview. The LSP treats the
    /// payload opaquely: `name` is the settings file name the preview asked for
    /// via [`super::PreviewToLspMessage::RequestState`], `contents` is the raw
    /// serialized string read from disk. The preview owns (de)serialization.
    SetUserSettings {
        name: String,
        contents: String,
    },
    ShowPreview(PreviewComponent),
    HighlightFromEditor {
        url: Option<Url>,
        offset: u32,
    },
    /// State of the remote-preview WebSocket, as observed by the LSP main
    /// process. Sent back to the preview process so its Remote Preview
    /// dialog can show the live state.
    RemoteConnectionState {
        state: RemoteConnectionState,
        /// Human-readable `address:port` of the attempted target.
        target: String,
        /// Set on `Failed` to describe the cause.
        error: Option<String>,
    },
    Quit,
    /// Keepalive probe; the viewer answers with [`super::PreviewToLspMessage::Pong`].
    /// Never sent to local previews.
    /// A protocol message because the LSP's browser-compatible WebSocket layer
    /// doesn't expose frame-level pings.
    Ping,
    OpenProject {
        root: Url,
    },
    /// Answer to [`super::PreviewToLspMessage::PairingReady`], and the first
    /// thing a remote connection sends. `token` names the reconnect token
    /// this editor holds from pairing with the viewer earlier in its run;
    /// holding it is proven in the exchange that follows, never by sending it.
    PairingHello {
        token: Option<super::pairing::TokenId>,
    },
    /// The client's half of the SPAKE2 exchange, plus proof that it derived
    /// the same key. Answers [`super::PreviewToLspMessage::PairingRequired`]
    /// and [`super::PreviewToLspMessage::PairingTokenChallenge`].
    PairingResponse {
        element: super::pairing::Element,
        confirmation: super::pairing::Confirmation,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemoteConnectionState {
    Disconnected,
    Connecting,
    /// A session dropped and the LSP is redialing it on its own.
    /// A new connection attempt from the user replaces the redial.
    Reconnecting,
    /// The viewer is showing a pairing code and is waiting for the user to
    /// type it into the editor.
    PairingRequired {
        code_digits: u8,
    },
    /// The viewer has pairing disabled, so the session would be neither
    /// authenticated nor encrypted. Waiting for the user to accept that or
    /// cancel.
    UnpairedWarning,
    Connected,
    Failed,
}

impl lsp_types::notification::Notification for LspToPreviewMessage {
    type Params = Self;
    const METHOD: &'static str = "slint/lsp_to_preview";
}

/// One transport from the LSP to a preview.
pub trait LspToPreview {
    fn send(&self, message: &LspToPreviewMessage);
    fn preview_target(&self) -> PreviewTarget;
    fn shutdown<'a>(&'a self) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
        Box::pin(async move { self.send(&LspToPreviewMessage::Quit) })
    }
}

mod named_overrides {
    use i_slint_compiler::project_file::Overrides;
    use serde::{Deserialize, Serialize};

    pub fn serialize<S: serde::Serializer>(
        overrides: &Overrides,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            overrides.serialize(serializer)
        } else {
            let bytes = serde_json::to_vec(overrides).map_err(serde::ser::Error::custom)?;
            serializer.serialize_bytes(&bytes)
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Overrides, D::Error> {
        if deserializer.is_human_readable() {
            Overrides::deserialize(deserializer)
        } else {
            let bytes = Vec::<u8>::deserialize(deserializer)?;
            serde_json::from_slice(&bytes).map_err(serde::de::Error::custom)
        }
    }
}

#[cfg(test)]
mod configuration_tests {
    use super::PreviewConfig;

    fn future_overrides() -> serde_json::Value {
        serde_json::json!({
            "style": "fluent",
            "project": { "include-paths": [], "slint-edition": "1.19" },
            "embed_resources": "FutureEmbedding",
            "default_translation_context": "FutureContext",
            "debug_info": false,
            "const_scale_factor": 2.0,
            "use_sdf_fonts": true,
            "bundled_translations_path": "/translations",
            "future-compiler-setting": 17
        })
    }

    #[test]
    fn named_json_configuration_keeps_supported_fields() {
        let config: PreviewConfig = serde_json::from_value(serde_json::json!({
            "hide_ui": null, "format_utf8": true, "overrides": future_overrides()
        }))
        .unwrap();
        assert_eq!(config.overrides.style.as_deref(), Some("fluent"));
        assert_eq!(config.overrides.project.include_paths, Some(Vec::new()));
        assert_eq!(
            config.overrides.project.slint_edition,
            Some(i_slint_compiler::project_file::SlintVersion::V1_19)
        );
        assert_eq!(config.overrides.debug_info, Some(false));
        assert_eq!(config.overrides.const_scale_factor, Some(2.));
        assert_eq!(config.overrides.embed_resources, None);
        assert_eq!(config.overrides.default_translation_context, None);
        let json = serde_json::to_value(&config).unwrap();
        assert!(json["overrides"].is_object());
        assert_eq!(serde_json::from_value::<PreviewConfig>(json).unwrap(), config);
        assert!(
            serde_json::from_value::<PreviewConfig>(serde_json::json!({
                "hide_ui": null, "format_utf8": true, "overrides": {"embed_resources": 42}
            }))
            .is_err()
        );
    }

    #[test]
    fn nested_project_fields_remain_strict() {
        assert!(
            serde_json::from_value::<PreviewConfig>(serde_json::json!({
                "hide_ui": null, "format_utf8": true,
                "overrides": {"project": {"future-project-setting": true}}
            }))
            .is_err()
        );
    }

    #[cfg(feature = "remote")]
    #[test]
    fn postcard_configuration_uses_named_fields_across_feature_sets() {
        #[derive(serde::Serialize)]
        struct FutureConfig {
            hide_ui: Option<bool>,
            overrides: Vec<u8>,
            format_utf8: bool,
        }
        let encoded = postcard::to_allocvec(&FutureConfig {
            hide_ui: Some(true),
            overrides: serde_json::to_vec(&future_overrides()).unwrap(),
            format_utf8: true,
        })
        .unwrap();
        let decoded: PreviewConfig = postcard::from_bytes(&encoded).unwrap();
        assert_eq!(decoded.overrides.style.as_deref(), Some("fluent"));
        assert_eq!(decoded.overrides.embed_resources, None);
        assert_eq!(decoded.overrides.default_translation_context, None);
        let encoded = postcard::to_allocvec(&decoded).unwrap();
        assert_eq!(postcard::from_bytes::<PreviewConfig>(&encoded).unwrap(), decoded);
    }
}
