// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Snapshots of what the connection to a remote viewer carries, kept in `snapshots.txt`.
//!
//! All patch releases of a minor version connect to each other, see [`super::PROTOCOL_SUBPROTOCOL`].
//! So a changed snapshot needs a new protocol version.
//! Run the test with `SLINT_PROTOCOL_SNAPSHOT_UPDATE=1` to rewrite the snapshots.

use std::{collections::BTreeMap, fmt::Debug, path::PathBuf};

use lsp_types::Url;
use serde::{Serialize, de::DeserializeOwned};
use sha2::Digest as _;

use super::pairing::{Confirmation, Element, Handshake, Role, Secrets, Token, TokenId};
use super::{
    LspToPreviewMessage, PairingRejection, PreviewComponent, PreviewToLspMessage, VersionedUrl,
    session,
};

const SNAPSHOTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/protocol/snapshots.txt");

fn url() -> Url {
    Url::parse("file:///project/app.slint").unwrap()
}

/// Only the messages a remote viewer exchanges; a local preview is the same build as the LSP.
/// `SetConfiguration` carries compiler-feature-dependent JSON, see `postcard_configuration_uses_named_fields_across_feature_sets`.
#[test]
fn wire_format_matches_snapshots() {
    let mut snapshots = Snapshots::load();

    use PreviewToLspMessage as P;
    snapshots.assert_roundtrip(P::Diagnostics {
        uri: url(),
        version: Some(300),
        diagnostics: vec![lsp_types::Diagnostic {
            severity: Some(lsp_types::DiagnosticSeverity::WARNING),
            message: "unused property".into(),
            ..Default::default()
        }],
    });
    snapshots.assert_roundtrip(P::RequestState {
        files: vec![url()],
        settings: vec!["settings.json".into()],
    });
    snapshots.assert_roundtrip(P::DebugMessage {
        location: Some((PathBuf::from("app.slint"), 300, 1000)),
        message: "hello".into(),
    });
    snapshots.assert_roundtrip(P::Pong);
    snapshots.assert_roundtrip(P::PairingReady);
    snapshots.assert_roundtrip(P::PairingRequired {
        attempts_left: 130,
        expires_in_seconds: 300,
        code_digits: 140,
        element: Element::for_test(vec![1, 2, 3]),
    });
    snapshots
        .assert_roundtrip(P::PairingTokenChallenge { element: Element::for_test(vec![4, 5, 6]) });
    snapshots.assert_roundtrip(P::PairingConfirm { confirmation: Confirmation::for_test(7) });
    snapshots.assert_roundtrip(P::PairingAccepted);
    for reason in [
        PairingRejection::BadCode,
        PairingRejection::BadToken,
        PairingRejection::Expired,
        PairingRejection::TooManyAttempts,
        PairingRejection::Busy,
        PairingRejection::TooSoon { retry_after_seconds: 900 },
    ] {
        snapshots.assert_roundtrip(P::PairingRejected { reason });
    }

    use LspToPreviewMessage as L;
    snapshots.assert_roundtrip(L::InvalidateContents { url: url() });
    snapshots.assert_roundtrip(L::ForgetFile { url: url() });
    snapshots.assert_roundtrip(L::SetContents {
        url: VersionedUrl::new(url(), Some(300)),
        contents: b"export component App {}".to_vec(),
    });
    snapshots.assert_roundtrip(L::SetUserSettings {
        name: "settings.json".into(),
        contents: "{}".into(),
    });
    snapshots.assert_roundtrip(L::ShowPreview(PreviewComponent {
        url: url(),
        component: Some("App".into()),
    }));
    snapshots.assert_roundtrip(L::HighlightFromEditor { url: Some(url()), offset: 1000 });
    snapshots.assert_roundtrip(L::Ping);
    snapshots.assert_roundtrip(L::OpenProject { root: Url::parse("file:///project/").unwrap() });
    snapshots.assert_roundtrip(L::PairingHello { token: Some(TokenId::for_test(8)) });
    snapshots.assert_roundtrip(L::PairingResponse {
        element: Element::for_test(vec![1, 2, 3]),
        confirmation: Confirmation::for_test(7),
    });

    let mut sealer = session::Sealer::new([7; 32]);
    let frames = [sealer.seal(b"a".to_vec()).unwrap(), sealer.seal(b"b".to_vec()).unwrap()];
    snapshots.assert_bytes("Two sealed frames", &frames.concat());

    let (code_exchange, token) = run_exchange(
        Handshake::with_code_from(ConstantRng(1), Role::Viewer, "4321"),
        Handshake::with_code_from(ConstantRng(2), Role::Editor, "4321"),
    );
    let (token_exchange, _) = run_exchange(
        Handshake::with_token_from(ConstantRng(3), Role::Viewer, &token),
        Handshake::with_token_from(ConstantRng(4), Role::Editor, &token),
    );
    snapshots.assert_bytes(
        "SHA-256 of a code and a token exchange",
        &sha2::Sha256::digest([code_exchange, token_exchange].concat()),
    );

    snapshots.finish();
}

fn run_exchange(viewer: Handshake, editor: Handshake) -> (Vec<u8>, Token) {
    let (viewer_element, editor_element) = (viewer.element().clone(), editor.element().clone());
    let viewer_secrets = viewer.finish(&editor_element).unwrap();
    let editor_secrets = editor.finish(&viewer_element).unwrap();
    let sealed = |secrets: &Secrets| secrets.session().0.seal(b"sealed".to_vec()).unwrap();
    let exchanged = postcard::to_allocvec(&(
        viewer_element,
        editor_element,
        viewer_secrets.confirmation(),
        editor_secrets.confirmation(),
        viewer_secrets.token_id,
        sealed(&viewer_secrets),
        sealed(&editor_secrets),
    ))
    .unwrap();
    (exchanged, viewer_secrets.token)
}

#[derive(Clone, Copy)]
struct ConstantRng(u8);

impl rand_core::RngCore for ConstantRng {
    fn next_u32(&mut self) -> u32 {
        rand_core::impls::next_u32_via_fill(self)
    }

    fn next_u64(&mut self) -> u64 {
        rand_core::impls::next_u64_via_fill(self)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        dest.fill(self.0);
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl rand_core::CryptoRng for ConstantRng {}

/// Each entry in the file is a name on one line and the bytes in hex on the next.
struct Snapshots {
    update: bool,
    stored: BTreeMap<String, String>,
    taken: BTreeMap<String, String>,
    problems: Vec<String>,
}

impl Snapshots {
    fn load() -> Self {
        let file = std::fs::read_to_string(SNAPSHOTS).unwrap_or_default();
        let lines: Vec<&str> = file.lines().filter(|line| !line.is_empty()).collect();
        Self {
            update: std::env::var("SLINT_PROTOCOL_SNAPSHOT_UPDATE").is_ok_and(|v| v == "1"),
            stored: lines.chunks(2).map(|entry| (entry[0].into(), entry[1].into())).collect(),
            taken: BTreeMap::new(),
            problems: Vec::new(),
        }
    }

    /// Checks that `message` encodes to its snapshot, and that its snapshot decodes to `message`.
    fn assert_roundtrip<M: Serialize + DeserializeOwned + PartialEq + Debug>(
        &mut self,
        message: M,
    ) {
        let name = format!("{message:?}");
        let snapshot = self.assert_bytes(&name, &postcard::to_allocvec(&message).unwrap());
        match postcard::from_bytes::<M>(&snapshot) {
            Ok(decoded) if decoded == message => {}
            Ok(decoded) => {
                self.problems.push(format!("{name}\n    its snapshot decodes to {decoded:?}"))
            }
            Err(error) => {
                self.problems.push(format!("{name}\n    its snapshot doesn't decode: {error}"))
            }
        }
    }

    /// Checks that `bytes` match the snapshot called `name`, and returns the snapshot's bytes.
    fn assert_bytes(&mut self, name: &str, bytes: &[u8]) -> Vec<u8> {
        let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        assert!(self.taken.insert(name.into(), hex.clone()).is_none(), "two snapshots of {name}");
        match self.stored.get(name) {
            _ if self.update => bytes.to_vec(),
            None => {
                self.problems.push(format!("{name}\n    has no snapshot"));
                bytes.to_vec()
            }
            Some(stored) => {
                if *stored != hex {
                    self.problems.push(format!("{name}\n    was {stored}\n    is  {hex}"));
                }
                (0..stored.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&stored[i..i + 2], 16).unwrap())
                    .collect()
            }
        }
    }

    fn finish(mut self) {
        if self.update {
            let file: String =
                self.taken.iter().map(|(name, hex)| format!("{name}\n{hex}\n\n")).collect();
            std::fs::write(SNAPSHOTS, file).unwrap();
        } else {
            for name in self.stored.keys().filter(|name| !self.taken.contains_key(*name)) {
                self.problems.push(format!("{name}\n    has a snapshot but no check"));
            }
        }
        assert!(
            self.problems.is_empty(),
            "{}\n\nRun with SLINT_PROTOCOL_SNAPSHOT_UPDATE=1 to update the snapshots, \
             and bump the protocol version if a released version sends or reads these bytes.",
            self.problems.join("\n")
        );
    }
}
