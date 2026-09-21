// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Authenticated encryption for an established session.
//!
//! One key per direction, so the two halves can live in different tasks
//! without sharing a counter. Nonces are the frame counter, which works
//! because every key here is used by exactly one sender and TCP delivers
//! in order: a repeated or reordered frame fails to open rather than
//! being accepted twice.

use chacha20poly1305::aead::{Aead, AeadInPlace, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use std::borrow::Cow;

/// Bytes of key material one direction needs.
pub const DIRECTION_KEY_LEN: usize = 32;

/// The frame counter ran out. Unreachable in practice at 2^64 frames, but
/// wrapping it would reuse a nonce, which loses everything.
#[derive(Debug)]
pub struct CounterExhausted;

/// The frame didn't decrypt: tampered with, reordered, or from a peer that
/// derived a different key. All three are the same answer -- drop it.
#[derive(Debug)]
pub struct NotAuthentic;

fn nonce_for(counter: u64) -> Nonce {
    let mut bytes = [0u8; 12];
    bytes[4..].copy_from_slice(&counter.to_be_bytes());
    *Nonce::from_slice(&bytes)
}

/// The writing half of a session.
pub struct Sealer {
    cipher: ChaCha20Poly1305,
    counter: u64,
}

impl Sealer {
    pub fn new(key: [u8; DIRECTION_KEY_LEN]) -> Self {
        Self { cipher: ChaCha20Poly1305::new(Key::from_slice(&key)), counter: 0 }
    }

    /// Takes the buffer and encrypts it in place: the plaintext is spent
    /// either way, and this saves copying every outbound frame.
    pub fn seal(&mut self, mut plaintext: Vec<u8>) -> Result<Vec<u8>, CounterExhausted> {
        let nonce = nonce_for(self.counter);
        self.counter = self.counter.checked_add(1).ok_or(CounterExhausted)?;
        // Encryption itself only fails on absurd input lengths.
        self.cipher.encrypt_in_place(&nonce, b"", &mut plaintext).map_err(|_| CounterExhausted)?;
        Ok(plaintext)
    }
}

/// The reading half of a session.
pub struct Opener {
    cipher: ChaCha20Poly1305,
    counter: u64,
}

impl Opener {
    pub fn new(key: [u8; DIRECTION_KEY_LEN]) -> Self {
        Self { cipher: ChaCha20Poly1305::new(Key::from_slice(&key)), counter: 0 }
    }

    pub fn open(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, NotAuthentic> {
        let nonce = nonce_for(self.counter);
        let plaintext = self.cipher.decrypt(&nonce, ciphertext).map_err(|_| NotAuthentic)?;
        // Only advance once the frame proved authentic, so a rejected frame
        // doesn't throw the counters out of step.
        self.counter = self.counter.saturating_add(1);
        Ok(plaintext)
    }
}

/// The outbound half of a session's codec.
///
/// Owns the decision between sealing and passing frames through, so no send
/// path carries its own "is this session encrypted?" branch. `Plaintext`
/// exists for `--no-pairing`, which establishes no shared secret.
pub enum Sealing {
    Plaintext,
    Sealed(Sealer),
}

impl Sealing {
    pub fn seal(&mut self, plaintext: Vec<u8>) -> Result<Vec<u8>, CounterExhausted> {
        match self {
            Self::Plaintext => Ok(plaintext),
            Self::Sealed(sealer) => sealer.seal(plaintext),
        }
    }
}

/// The inbound half of a session's codec, mirroring [`Sealing`].
pub enum Opening {
    Plaintext,
    Sealed(Opener),
}

impl Opening {
    /// Borrowed in the plaintext case, so `--no-pairing` doesn't pay for a
    /// copy of every inbound frame.
    pub fn open<'a>(&mut self, frame: &'a [u8]) -> Result<Cow<'a, [u8]>, NotAuthentic> {
        match self {
            Self::Plaintext => Ok(Cow::Borrowed(frame)),
            Self::Sealed(opener) => opener.open(frame).map(Cow::Owned),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> (Sealer, Opener) {
        (Sealer::new([7u8; 32]), Opener::new([7u8; 32]))
    }

    #[test]
    fn a_sealed_frame_opens() {
        let (mut sealer, mut opener) = pair();
        let sealed = sealer.seal(b"hello".to_vec()).unwrap();
        assert_ne!(sealed, b"hello", "the plaintext went out as-is");
        assert_eq!(opener.open(&sealed).unwrap(), b"hello");
    }

    #[test]
    fn frames_must_arrive_in_order() {
        let (mut sealer, mut opener) = pair();
        let first = sealer.seal(b"one".to_vec()).unwrap();
        let second = sealer.seal(b"two".to_vec()).unwrap();
        // Skipping the first puts the counter out of step, so the second
        // doesn't open either.
        assert!(opener.open(&second).is_err());
        assert_eq!(opener.open(&first).unwrap(), b"one");
    }

    #[test]
    fn a_replayed_frame_is_refused() {
        let (mut sealer, mut opener) = pair();
        let sealed = sealer.seal(b"once".to_vec()).unwrap();
        assert_eq!(opener.open(&sealed).unwrap(), b"once");
        assert!(opener.open(&sealed).is_err(), "a repeat was accepted");
    }

    #[test]
    fn tampering_is_caught() {
        let (mut sealer, mut opener) = pair();
        let mut sealed = sealer.seal(b"intact".to_vec()).unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 0x01;
        assert!(opener.open(&sealed).is_err());
    }

    #[test]
    fn a_different_key_cannot_open() {
        let mut sealer = Sealer::new([1u8; 32]);
        let mut opener = Opener::new([2u8; 32]);
        let sealed = sealer.seal(b"secret".to_vec()).unwrap();
        assert!(opener.open(&sealed).is_err());
    }

    #[test]
    fn a_rejected_frame_keeps_the_counter_in_step() {
        let (mut sealer, mut opener) = pair();
        let good = sealer.seal(b"good".to_vec()).unwrap();
        let mut bad = good.clone();
        bad[0] ^= 0xff;
        assert!(opener.open(&bad).is_err());
        // The counter didn't move, so the real frame still opens.
        assert_eq!(opener.open(&good).unwrap(), b"good");
    }

    #[test]
    fn the_plaintext_codec_passes_frames_through() {
        let (mut sealing, mut opening) = (Sealing::Plaintext, Opening::Plaintext);
        let sent = sealing.seal(b"open".to_vec()).unwrap();
        assert_eq!(sent, b"open");
        assert!(matches!(opening.open(&sent).unwrap(), Cow::Borrowed(b"open")));
    }

    #[test]
    fn the_sealed_codec_round_trips() {
        let (sealer, opener) = pair();
        let (mut sealing, mut opening) = (Sealing::Sealed(sealer), Opening::Sealed(opener));
        let sent = sealing.seal(b"shut".to_vec()).unwrap();
        assert_ne!(sent, b"shut");
        assert_eq!(opening.open(&sent).unwrap().as_ref(), b"shut");
    }
}
