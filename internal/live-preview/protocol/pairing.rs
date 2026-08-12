// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The pairing handshake that guards the remote preview connection.
//!
//! The viewer listens on the wildcard address and advertises itself over
//! mDNS, so reaching the port proves nothing. A client is admitted only
//! once it proves it can see the viewer's screen, by echoing back the code
//! displayed there.
//!
//! Nothing secret crosses the wire: both proofs are HMAC-SHA256 over a
//! per-connection nonce, and the reconnect token is derived rather than
//! sent.

use std::time::Duration;

/// Byte length of the viewer's per-connection nonce.
const NONCE_LEN: usize = 32;

/// Byte length of a proof.
const MAC_LEN: usize = 32;

/// Byte length of a reconnect token.
const TOKEN_LEN: usize = 32;

/// The viewer's per-connection challenge.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub struct Nonce([u8; NONCE_LEN]);

/// Proof of holding a credential, without revealing it.
///
/// Deliberately not [`PartialEq`]: comparing proofs with `==` gives away
/// where they first differ, so [`Proof::matches`] is the only way to
/// compare two.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub struct Proof([u8; MAC_LEN]);

/// A reconnect token, derived from a completed code exchange.
///
/// Has no `Serialize`, which is the point: both ends compute it, and it
/// must never end up in a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token([u8; TOKEN_LEN]);

impl Proof {
    /// Compare without leaking where the two first differ.
    pub fn matches(&self, other: &Proof) -> bool {
        use subtle::ConstantTimeEq as _;

        self.0.ct_eq(&other.0).into()
    }
}

#[cfg(test)]
impl Nonce {
    /// A predictable nonce, so tests can assert on what a proof depends on.
    pub fn for_test(byte: u8) -> Self {
        Self([byte; NONCE_LEN])
    }
}

impl Token {
    /// Only for checking that a token never reaches the wire.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Digits in a generated pairing code, short enough to read off a phone
/// across a desk.
///
/// 10000 combinations isn't much, so what makes guessing impractical is the
/// limits around it: [`MAX_ATTEMPTS`] per prompt, a cool-down that doubles
/// with each failure, and a prompt the device's owner can see. A generated
/// code is fresh each time; one pinned with `--pairing-code` isn't, which
/// is what the escalation is for.
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

/// Which secret a proof was computed from. Domain separated, so a proof
/// from one exchange can't be presented as the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Credential {
    /// The code the viewer displays on screen.
    Code,
    /// The token the viewer issued after a previous successful pairing.
    Token,
}

impl Credential {
    fn domain(self) -> &'static [u8] {
        match self {
            Credential::Code => b"slint-preview-pairing-code-v1",
            Credential::Token => b"slint-preview-pairing-token-v1",
        }
    }
}

/// Compute the proof for `secret` against the viewer's `nonce`: the ASCII
/// digits for [`Credential::Code`], the raw bytes for [`Credential::Token`].
pub fn proof(credential: Credential, secret: &[u8], nonce: &Nonce) -> Proof {
    use hmac::Mac as _;

    // A key of any length is fine for HMAC, so this can't fail.
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(secret)
        .expect("HMAC accepts keys of any length");
    mac.update(credential.domain());
    mac.update(&nonce.0);
    Proof(mac.finalize().into_bytes().into())
}

/// Derive the reconnect token from a code exchange that just succeeded.
///
/// Both ends compute it, so it never has to be sent. Sending it would put
/// the credential itself on a plaintext socket for anyone to lift.
pub fn derive_token(code: &str, nonce: &Nonce) -> Token {
    use hmac::Mac as _;

    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(code.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(b"slint-preview-pairing-derive-v1");
    mac.update(&nonce.0);
    Token(mac.finalize().into_bytes().into())
}

/// Random material, which only the viewer needs. Keeping it here keeps the
/// random source out of the LSP's browser builds.
#[cfg(feature = "remote")]
pub mod generate {
    use super::{CODE_DIGITS, Nonce};
    use rand::Rng as _;

    /// A fresh nonce for one connection's challenge.
    pub fn nonce() -> Nonce {
        Nonce(rand::rng().random())
    }

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

    #[test]
    fn proof_is_stable_and_key_dependent() {
        let nonce = Nonce::for_test(7);
        assert!(proof(Credential::Code, b"123456", &nonce).matches(&proof(
            Credential::Code,
            b"123456",
            &nonce
        )));
        assert!(!proof(Credential::Code, b"123456", &nonce).matches(&proof(
            Credential::Code,
            b"123457",
            &nonce
        )));
    }

    #[test]
    fn proof_is_nonce_dependent() {
        assert!(!proof(Credential::Code, b"123456", &Nonce::for_test(1)).matches(&proof(
            Credential::Code,
            b"123456",
            &Nonce::for_test(2)
        )));
    }

    #[test]
    fn a_derived_token_is_not_a_proof_of_the_same_inputs() {
        let nonce = Nonce::for_test(7);
        // Sharing an HMAC key across purposes is how one exchange's output
        // becomes another's credential.
        for credential in [Credential::Code, Credential::Token] {
            assert_ne!(
                derive_token("4321", &nonce).as_bytes(),
                proof(credential, b"4321", &nonce).0.as_slice()
            );
        }
    }

    #[test]
    fn a_derived_token_needs_both_the_code_and_the_nonce() {
        let nonce = Nonce::for_test(7);
        assert_eq!(derive_token("4321", &nonce), derive_token("4321", &nonce));
        assert_ne!(derive_token("4321", &nonce), derive_token("1234", &nonce));
        assert_ne!(derive_token("4321", &nonce), derive_token("4321", &Nonce::for_test(8)));
    }

    #[test]
    fn credentials_are_domain_separated() {
        let nonce = Nonce::for_test(7);
        assert!(!proof(Credential::Code, b"secret", &nonce).matches(&proof(
            Credential::Token,
            b"secret",
            &nonce
        )));
    }

    #[test]
    fn matches_agrees_with_the_inputs() {
        let nonce = Nonce::for_test(7);
        let a = proof(Credential::Code, b"123456", &nonce);
        let b = proof(Credential::Code, b"123456", &nonce);
        let c = proof(Credential::Code, b"654321", &nonce);
        assert!(a.matches(&b));
        assert!(!a.matches(&c));
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
