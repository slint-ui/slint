// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The pairing handshake that guards the remote preview connection.
//!
//! The viewer listens on the wildcard address and advertises itself over
//! mDNS, so reaching the port proves nothing. A client is admitted only
//! once it proves it can see the viewer's screen, by echoing back the code
//! displayed there.
//!
//! The code is established with SPAKE2, a password-authenticated key
//! exchange: a passive observer learns nothing about it, and an active one
//! gets a single guess per attempt with no offline search to fall back on.
//! That is what makes four digits defensible -- the limits elsewhere in
//! this module bound the online guessing, and there is no other avenue.
//!
//! The exchange also yields a session key, so everything after pairing is
//! sealed. The reconnect token is derived from that key rather than from
//! the code, and using it means running the same exchange again with the
//! token as the secret: every session gets fresh keys, so recorded traffic
//! stays sealed even if the token leaks later.

use crate::protocol::session;
use std::time::Duration;

/// Byte length of a reconnect token.
const TOKEN_LEN: usize = 32;

/// Byte length of a token id.
const TOKEN_ID_LEN: usize = 16;

/// A reconnect token, derived from a completed code exchange.
///
/// Has no `Serialize`, which is the point: both ends compute it, and it
/// must never end up in a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token([u8; TOKEN_LEN]);

/// The public name of a [`Token`], derived alongside it.
///
/// The editor announces it on reconnect, so the viewer knows which token
/// to run the exchange with. It proves nothing by itself: anyone on the
/// network can read it off the wire and replay it, and gets no further
/// than the exchange the token itself is required for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct TokenId([u8; TOKEN_ID_LEN]);

#[cfg(test)]
impl Token {
    /// A token no viewer ever issued, for tests that need one to be refused.
    pub fn for_test(byte: u8) -> Self {
        Self([byte; TOKEN_LEN])
    }
}

#[cfg(test)]
impl TokenId {
    /// An id no viewer ever issued.
    pub fn for_test(byte: u8) -> Self {
        Self([byte; TOKEN_ID_LEN])
    }
}

impl Token {
    /// Only for checking that a token never reaches the wire.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Label mixed into every derivation, so keys from this protocol can never
/// collide with keys from another that happens to share inputs.
const TRANSCRIPT_LABEL: &[u8] = b"slint-preview-pairing-spake2-v1";

/// Which end of the exchange a party is. SPAKE2 is asymmetric: the two
/// sides use different generator points, and both must agree who is who or
/// the shared secret silently fails to match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Viewer,
    Editor,
}

/// One side's public element, sent to the peer.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Element(Vec<u8>);

#[cfg(test)]
impl Element {
    /// Deliberately malformed input, for tests.
    pub fn for_test(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

/// Proof of having derived the same key. Deliberately not [`PartialEq`]:
/// comparing with `==` gives away where two proofs first differ, so
/// [`Confirmation::matches`] is the only way to compare.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub struct Confirmation([u8; 32]);

impl Confirmation {
    /// Compare without leaking where the two first differ.
    pub fn matches(&self, other: &Confirmation) -> bool {
        use subtle::ConstantTimeEq as _;

        self.0.ct_eq(&other.0).into()
    }
}

/// The exchange failed. Deliberately one variant: distinguishing "wrong
/// code" from "tampered with" would tell an attacker which of the two they
/// achieved.
#[derive(Debug)]
pub struct HandshakeFailed;

/// Everything derived once both sides finish.
pub struct Secrets {
    /// The viewer proves knowledge with this; the editor checks it. Not
    /// public: which one to send and which to check is decided by
    /// [`Self::confirmation`] and [`Self::peer_confirms`], never at a call
    /// site.
    viewer_confirmation: Confirmation,
    /// The editor proves knowledge with this; the viewer checks it.
    editor_confirmation: Confirmation,
    /// Reconnect token, bound to this exchange rather than to the code.
    pub token: Token,
    /// Public name of `token`, for announcing it on reconnect.
    pub token_id: TokenId,
    /// Session key for viewer-to-editor frames. Not public: key material
    /// leaves this module only inside a [`session`] codec.
    viewer_to_editor: [u8; session::DIRECTION_KEY_LEN],
    /// Session key for editor-to-viewer frames.
    editor_to_viewer: [u8; session::DIRECTION_KEY_LEN],
    /// Which side derived this, so the role-dependent accessors know their
    /// directions.
    role: Role,
}

impl Secrets {
    /// The sealed session for the side that ran the handshake.
    ///
    /// The one place that decides who seals with which key. Getting this
    /// wrong compiles fine and only surfaces as every frame failing to
    /// open, which is why it isn't decided at the call sites.
    pub fn session(&self) -> (session::Sealing, session::Opening) {
        let (seal, open) = match self.role {
            Role::Viewer => (self.viewer_to_editor, self.editor_to_viewer),
            Role::Editor => (self.editor_to_viewer, self.viewer_to_editor),
        };
        (
            session::Sealing::Sealed(session::Sealer::new(seal)),
            session::Opening::Sealed(session::Opener::new(open)),
        )
    }

    /// The proof this side sends to the peer.
    pub fn confirmation(&self) -> Confirmation {
        match self.role {
            Role::Viewer => self.viewer_confirmation,
            Role::Editor => self.editor_confirmation,
        }
    }

    /// Whether `theirs` proves the peer derived the same key. Like the
    /// session keys, which confirmation is the peer's is decided here and
    /// not at the call sites: picking the wrong one would accept our own
    /// proof as the peer's.
    pub fn peer_confirms(&self, theirs: &Confirmation) -> bool {
        let peers = match self.role {
            Role::Viewer => &self.editor_confirmation,
            Role::Editor => &self.viewer_confirmation,
        };
        theirs.matches(peers)
    }
}

#[cfg(test)]
impl Secrets {
    /// Raw key material, for asserting it never crosses the wire.
    pub fn key_material(&self) -> [&[u8]; 2] {
        [&self.viewer_to_editor, &self.editor_to_viewer]
    }
}

/// One side of the SPAKE2 exchange, mid-flight.
pub struct Handshake {
    state: spake2::Spake2<spake2::Ed25519Group>,
    role: Role,
    element: Element,
}

impl Handshake {
    /// Begin a first pairing, with the on-screen code as the secret.
    pub fn with_code(role: Role, code: &str) -> Self {
        Self::start(role, b"code:", code.as_bytes())
    }

    /// Begin a reconnect, with an issued token as the secret.
    ///
    /// The token has plenty of entropy, but running it through the same
    /// exchange instead of keying a session from it directly buys forward
    /// secrecy: the session keys come out of the fresh exchange, so someone
    /// who records the traffic and later steals the token opens nothing.
    pub fn with_token(role: Role, token: &Token) -> Self {
        Self::start(role, b"token:", &token.0)
    }

    /// The prefix keeps the two secret spaces apart, so a code exchange
    /// and a token exchange can never agree with each other.
    fn start(role: Role, prefix: &[u8], secret: &[u8]) -> Self {
        let mut keyed = Vec::with_capacity(prefix.len() + secret.len());
        keyed.extend_from_slice(prefix);
        keyed.extend_from_slice(secret);
        let password = spake2::Password::new(&keyed);
        let viewer = spake2::Identity::new(b"slint-preview-viewer");
        let editor = spake2::Identity::new(b"slint-preview-editor");
        let (state, element) = match role {
            Role::Viewer => {
                spake2::Spake2::<spake2::Ed25519Group>::start_a(&password, &viewer, &editor)
            }
            Role::Editor => {
                spake2::Spake2::<spake2::Ed25519Group>::start_b(&password, &viewer, &editor)
            }
        };
        Self { state, role, element: Element(element) }
    }

    /// The element to send to the peer.
    pub fn element(&self) -> &Element {
        &self.element
    }

    /// Finish against the peer's element.
    ///
    /// A wrong code doesn't fail here -- it produces a *different* key, and
    /// the mismatch only surfaces when the confirmations are compared. That
    /// is the property the whole design rests on: the wire carries nothing
    /// an observer can test a guess against.
    pub fn finish(self, peer: &Element) -> Result<Secrets, HandshakeFailed> {
        let key = self.state.finish(&peer.0).map_err(|_| HandshakeFailed)?;

        // Both elements in a fixed order, length-prefixed so no pair of
        // different exchanges can produce the same transcript bytes.
        let (a, b) = match self.role {
            Role::Viewer => (&self.element, peer),
            Role::Editor => (peer, &self.element),
        };
        let mut transcript =
            Vec::with_capacity(TRANSCRIPT_LABEL.len() + a.0.len() + b.0.len() + 16);
        transcript.extend_from_slice(TRANSCRIPT_LABEL);
        for part in [&a.0, &b.0] {
            transcript.extend_from_slice(&(part.len() as u64).to_be_bytes());
            transcript.extend_from_slice(part);
        }

        let hkdf = hkdf::Hkdf::<sha2::Sha256>::new(Some(&transcript), &key);
        Ok(Secrets {
            viewer_confirmation: Confirmation(expand(&hkdf, b"confirm-viewer")),
            editor_confirmation: Confirmation(expand(&hkdf, b"confirm-editor")),
            token: Token(expand(&hkdf, b"reconnect-token")),
            token_id: TokenId(expand(&hkdf, b"reconnect-token-id")),
            viewer_to_editor: expand(&hkdf, b"seal-viewer-to-editor"),
            editor_to_viewer: expand(&hkdf, b"seal-editor-to-viewer"),
            role: self.role,
        })
    }

    /// Finish against the peer's element and require their proof, for the
    /// side that receives both together. One call, so no call site can
    /// finish and forget to check.
    pub fn finish_confirmed(
        self,
        peer: &Element,
        theirs: &Confirmation,
    ) -> Result<Secrets, HandshakeFailed> {
        let secrets = self.finish(peer)?;
        if secrets.peer_confirms(theirs) { Ok(secrets) } else { Err(HandshakeFailed) }
    }
}

fn expand<const N: usize>(hkdf: &hkdf::Hkdf<sha2::Sha256>, label: &[u8]) -> [u8; N] {
    let mut out = [0u8; N];
    // Only fails for absurd output lengths.
    hkdf.expand(label, &mut out).expect("a short HKDF output length is always valid");
    out
}

/// Digits in a generated pairing code, short enough to read off a phone
/// across a desk.
///
/// 10000 combinations isn't much, and that is only safe because the code is
/// established through a PAKE: there is no offline search to fall back on,
/// so every guess costs an online attempt. [`MAX_ATTEMPTS`] per prompt and a
/// cool-down that doubles with each failure bound those, and each attempt
/// puts a prompt in front of whoever holds the device.
///
/// A generated code is fresh each time; one pinned with `--pairing-code`
/// isn't, which is what the escalation is for.
pub const CODE_DIGITS: u32 = 4;

/// Whether `code` has the shape of a pairing code. Pinned codes are held to
/// it too, so they stay typeable in the editor's digits-only field.
pub fn is_valid_code(code: &str) -> bool {
    code.len() == CODE_DIGITS as usize && code.bytes().all(|b| b.is_ascii_digit())
}

/// How long a displayed code stays valid before the viewer drops the connection.
pub const CODE_TIMEOUT: Duration = Duration::from_secs(60);

/// How many wrong codes a single connection may offer before it's closed.
pub const MAX_ATTEMPTS: u8 = 3;

/// Shortest gap between prompts, so an unauthenticated peer can't keep one
/// on the device's screen. Grows with repeated failures.
pub const PROMPT_RATE_LIMIT: Duration = Duration::from_secs(30);

/// Random material, which only the viewer needs. Keeping it here keeps the
/// random source out of the LSP's browser builds.
#[cfg(feature = "remote")]
pub mod generate {
    use super::CODE_DIGITS;
    use rand::Rng as _;

    /// A fresh pairing code, zero-padded to [`CODE_DIGITS`] digits.
    pub fn code() -> String {
        let modulus = 10u32.pow(CODE_DIGITS);
        format!("{:0width$}", rand::rng().random_range(0..modulus), width = CODE_DIGITS as usize)
    }
}

/// Why the viewer turned a client away.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PairingRejection {
    /// The code didn't match. Retry until the attempts run out.
    BadCode,
    /// Not a token this viewer issued, usually because it restarted. The
    /// client should forget it and pair again.
    BadToken,
    /// The code wasn't entered within [`CODE_TIMEOUT`].
    Expired,
    /// [`MAX_ATTEMPTS`] wrong codes in a row.
    TooManyAttempts,
    /// Someone else is part-way through pairing with this viewer.
    Busy,
    /// A prompt ended too recently. Retrying works, just not yet; carries
    /// how long is left so the editor can say when.
    TooSoon { retry_after_seconds: u16 },
}

impl PairingRejection {
    /// Whether retrying is pointless. The reconnect loop retries forever by
    /// design, so a rejection that will keep happening has to stop it.
    /// [`Self::BadToken`] isn't terminal: the client forgets the token and
    /// falls through to the code.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Expired | Self::TooManyAttempts)
    }
}

/// Text shown in the editor when a connection attempt ends this way.
impl std::fmt::Display for PairingRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadCode => f.write_str("Incorrect pairing code"),
            Self::BadToken => f.write_str("The viewer no longer recognizes this editor"),
            Self::Expired => f.write_str("The pairing code expired before it was entered"),
            Self::TooManyAttempts => f.write_str("Too many incorrect pairing codes"),
            Self::Busy => f.write_str("Another computer is pairing with the viewer"),
            Self::TooSoon { retry_after_seconds: 1 } => f.write_str("Try again in a second"),
            Self::TooSoon { retry_after_seconds } => {
                write!(f, "Try again in {retry_after_seconds} seconds")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point: same code on both sides, same keys.
    #[test]
    fn matching_codes_agree_on_everything() {
        let viewer = Handshake::with_code(Role::Viewer, "4321");
        let editor = Handshake::with_code(Role::Editor, "4321");
        let (ve, ee) = (viewer.element().clone(), editor.element().clone());
        let vs = viewer.finish(&ee).unwrap();
        let es = editor.finish(&ve).unwrap();

        assert!(vs.viewer_confirmation.matches(&es.viewer_confirmation));
        assert!(vs.editor_confirmation.matches(&es.editor_confirmation));
        assert_eq!(vs.viewer_to_editor, es.viewer_to_editor);
        assert_eq!(vs.editor_to_viewer, es.editor_to_viewer);
        assert_eq!(vs.token, es.token);
        assert_eq!(vs.token_id, es.token_id);
    }

    /// A wrong code has to fail at confirmation, not earlier: nothing on
    /// the wire may let an observer test a guess.
    #[test]
    fn a_wrong_code_only_shows_up_at_confirmation() {
        let viewer = Handshake::with_code(Role::Viewer, "4321");
        let editor = Handshake::with_code(Role::Editor, "1234");
        let (ve, ee) = (viewer.element().clone(), editor.element().clone());

        // Both sides complete the exchange without complaint ...
        let vs = viewer.finish(&ee).unwrap();
        let es = editor.finish(&ve).unwrap();

        // ... and only the confirmations reveal the mismatch.
        assert!(!vs.editor_confirmation.matches(&es.editor_confirmation));
        assert!(!vs.viewer_confirmation.matches(&es.viewer_confirmation));
        assert_ne!(vs.viewer_to_editor, es.viewer_to_editor);
        assert_ne!(vs.token, es.token);
    }

    /// Each run must produce fresh keys, or a recorded session could be
    /// replayed against a later one.
    #[test]
    fn every_exchange_derives_different_keys() {
        let run = || {
            let viewer = Handshake::with_code(Role::Viewer, "4321");
            let editor = Handshake::with_code(Role::Editor, "4321");
            let (ve, ee) = (viewer.element().clone(), editor.element().clone());
            let _ = editor.finish(&ve).unwrap();
            viewer.finish(&ee).unwrap()
        };
        assert_ne!(run().viewer_to_editor, run().viewer_to_editor);
    }

    /// Reflecting a party's own message back at it must not work. The
    /// crate tags each element with its role and refuses the mismatch, so
    /// this fails outright rather than quietly deriving a different key.
    #[test]
    fn a_reflected_element_is_refused() {
        let one = Handshake::with_code(Role::Viewer, "4321");
        let two = Handshake::with_code(Role::Viewer, "4321");
        let b = two.element().clone();
        assert!(one.finish(&b).is_err(), "a same-role element was accepted");
    }

    /// The confirmations are separate outputs: revealing one must not
    /// hand over the other, nor the sealing keys.
    #[test]
    fn derived_values_are_all_distinct() {
        let viewer = Handshake::with_code(Role::Viewer, "4321");
        let editor = Handshake::with_code(Role::Editor, "4321");
        let ee = editor.element().clone();
        let s = viewer.finish(&ee).unwrap();
        let mut seen = std::collections::HashSet::new();
        assert!(seen.insert(s.viewer_confirmation.0.to_vec()));
        assert!(seen.insert(s.editor_confirmation.0.to_vec()));
        assert!(seen.insert(s.viewer_to_editor.to_vec()));
        assert!(seen.insert(s.editor_to_viewer.to_vec()));
        assert!(seen.insert(s.token.as_bytes().to_vec()));
        assert!(seen.insert(s.token_id.0.to_vec()));
    }

    /// A token exchange works like a code exchange: same token, same keys,
    /// and the sealed session connects the two roles.
    #[test]
    fn a_token_exchange_connects_the_two_roles() {
        let token = Token::for_test(3);
        let viewer = Handshake::with_token(Role::Viewer, &token);
        let editor = Handshake::with_token(Role::Editor, &token);
        let (ve, ee) = (viewer.element().clone(), editor.element().clone());
        let vs = viewer.finish(&ee).unwrap();
        let es = editor.finish(&ve).unwrap();

        assert!(vs.editor_confirmation.matches(&es.editor_confirmation));
        let (mut viewer_seal, _) = vs.session();
        let (_, mut editor_open) = es.session();
        let down = viewer_seal.seal(b"to the editor".to_vec()).unwrap();
        assert_eq!(editor_open.open(&down).unwrap().as_ref(), b"to the editor");
    }

    /// A wrong token fails like a wrong code: both sides finish without
    /// complaint and only the confirmations disagree. This is what stops a
    /// peer that sniffed a token *id* off the wire.
    #[test]
    fn a_wrong_token_only_shows_up_at_confirmation() {
        let viewer = Handshake::with_token(Role::Viewer, &Token::for_test(3));
        let editor = Handshake::with_token(Role::Editor, &Token::for_test(9));
        let (ve, ee) = (viewer.element().clone(), editor.element().clone());
        let vs = viewer.finish(&ee).unwrap();
        let es = editor.finish(&ve).unwrap();
        assert!(!vs.editor_confirmation.matches(&es.editor_confirmation));
        assert!(!vs.viewer_confirmation.matches(&es.viewer_confirmation));
    }

    /// The forward-secrecy property this design buys: reusing a token still
    /// produces fresh keys every session, so no recorded session can be
    /// opened by learning the token later.
    #[test]
    fn a_token_exchange_gives_fresh_keys_every_time() {
        let token = Token::for_test(3);
        let run = || {
            let viewer = Handshake::with_token(Role::Viewer, &token);
            let editor = Handshake::with_token(Role::Editor, &token);
            let ee = editor.element().clone();
            viewer.finish(&ee).unwrap()
        };
        assert_ne!(run().viewer_to_editor, run().viewer_to_editor);
    }

    #[test]
    fn code_validation_matches_what_is_generated() {
        assert!(is_valid_code("0000"));
        assert!(is_valid_code("4321"));
        assert!(!is_valid_code(""));
        assert!(!is_valid_code("123"));
        assert!(!is_valid_code("12345"));
        assert!(!is_valid_code("12a4"));
        // Digits, not just anything that parses as a number.
        assert!(!is_valid_code("+123"));
        assert!(!is_valid_code("１２３４"));
    }

    #[cfg(feature = "remote")]
    #[test]
    fn generated_codes_are_valid() {
        for _ in 0..1000 {
            let code = generate::code();
            assert!(is_valid_code(&code), "generated {code} is not a valid code");
        }
    }

    #[test]
    fn only_hopeless_rejections_are_terminal() {
        assert!(PairingRejection::Expired.is_terminal());
        assert!(PairingRejection::TooManyAttempts.is_terminal());
        // The client retries these: a bad code gets another attempt, a stale
        // token falls back to the code flow, and Busy clears on its own.
        assert!(!PairingRejection::BadCode.is_terminal());
        assert!(!PairingRejection::BadToken.is_terminal());
        assert!(!PairingRejection::Busy.is_terminal());
        assert!(!PairingRejection::TooSoon { retry_after_seconds: 30 }.is_terminal());
    }

    #[test]
    fn the_cool_down_says_how_long_is_left() {
        assert_eq!(
            PairingRejection::TooSoon { retry_after_seconds: 18 }.to_string(),
            "Try again in 18 seconds"
        );
        // One second reads badly in the plural form.
        assert_eq!(
            PairingRejection::TooSoon { retry_after_seconds: 1 }.to_string(),
            "Try again in a second"
        );
    }
}
