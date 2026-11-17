//! Authenticated encryption for VPN datagrams (shared passphrase).
//!
//! Every datagram, including control messages, is sealed with
//! XChaCha20-Poly1305. The key comes from the shared passphrase via Argon2id.
//! The hub forwards sealed datagrams unchanged, so a packet is only ever
//! readable by holders of the passphrase and cannot be forged or tampered
//! with by anyone else. Wrong key = packet silently dropped.
//!
//! ```text
//! [0..3]   magic "OXD"
//! [3]      version (2)
//! [4]      packet type
//! [5..13]  sender session id (random per run)
//! [13..21] sender counter (big endian, starts at 1)
//! [21..]   ciphertext + 16-byte Poly1305 tag
//! ```
//! Header is authenticated as AAD. Nonce = session || counter || 8 zero bytes.

use anyhow::{anyhow, Result};
use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

pub const MAGIC: [u8; 3] = *b"OXD";
pub const VERSION: u8 = 2;
pub const HEADER_LEN: usize = 21;
pub const TAG_LEN: usize = 16;
/// Bytes added to every payload.
pub const OVERHEAD: usize = HEADER_LEN + TAG_LEN;
pub const MIN_KEY_LEN: usize = 8;

const KDF_SALT: &[u8] = b"oxide-vpn-key-v2";

pub struct Cipher {
    aead: XChaCha20Poly1305,
    session: [u8; 8],
    counter: AtomicU64,
}

impl Cipher {
    /// Derive the key from the shared passphrase.
    pub fn new(passphrase: &str) -> Result<Self> {
        if passphrase.chars().count() < MIN_KEY_LEN {
            return Err(anyhow!(
                "la clave debe tener al menos {} caracteres",
                MIN_KEY_LEN
            ));
        }
        let mut key = [0u8; 32];
        Argon2::default()
            .hash_password_into(passphrase.as_bytes(), KDF_SALT, &mut key)
            .map_err(|e| anyhow!("key derivation failed: {e}"))?;
        let aead = XChaCha20Poly1305::new(&key.into());
        Ok(Self {
            aead,
            session: rand::random(),
            counter: AtomicU64::new(1),
        })
    }

    fn nonce(session: &[u8; 8], counter: u64) -> XNonce {
        let mut n = [0u8; 24];
        n[..8].copy_from_slice(session);
        n[8..16].copy_from_slice(&counter.to_be_bytes());
        n.into()
    }

    /// Encrypt `payload` into a datagram of the given packet type.
    pub fn seal(&self, kind: u8, payload: &[u8]) -> Vec<u8> {
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        let mut out = Vec::with_capacity(OVERHEAD + payload.len());
        out.extend_from_slice(&MAGIC);
        out.push(VERSION);
        out.push(kind);
        out.extend_from_slice(&self.session);
        out.extend_from_slice(&counter.to_be_bytes());
        let ct = self
            .aead
            .encrypt(
                &Self::nonce(&self.session, counter),
                Payload {
                    msg: payload,
                    aad: &out[..HEADER_LEN],
                },
            )
            .expect("encryption cannot fail");
        out.extend_from_slice(&ct);
        out
    }

    /// Authenticate and decrypt. None if malformed, wrong key, tampered,
    /// replayed, or our own packet reflected back.
    pub fn open(&self, datagram: &[u8], replay: &mut ReplayGuard) -> Option<(u8, Vec<u8>)> {
        if datagram.len() < OVERHEAD || datagram[0..3] != MAGIC || datagram[3] != VERSION {
            return None;
        }
        let kind = datagram[4];
        let mut session = [0u8; 8];
        session.copy_from_slice(&datagram[5..13]);
        if session == self.session {
            return None;
        }
        let counter = u64::from_be_bytes(datagram[13..21].try_into().ok()?);
        let plain = self
            .aead
            .decrypt(
                &Self::nonce(&session, counter),
                Payload {
                    msg: &datagram[HEADER_LEN..],
                    aad: &datagram[..HEADER_LEN],
                },
            )
            .ok()?;
        // Only after authentication, so forged packets cannot disturb the window
        if !replay.accept(session, counter) {
            return None;
        }
        Some((kind, plain))
    }
}

/// Sliding-window replay protection, one window per sender session.
#[derive(Default)]
pub struct ReplayGuard {
    windows: HashMap<[u8; 8], (u64, u64)>, // session -> (highest counter, bitmap of last 64)
}

impl ReplayGuard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn accept(&mut self, session: [u8; 8], counter: u64) -> bool {
        if self.windows.len() > 4096 {
            self.windows.clear();
        }
        let (max, bits) = self.windows.entry(session).or_insert((0, 0));
        if counter > *max {
            let shift = counter - *max;
            *bits = if shift >= 64 { 0 } else { *bits << shift };
            *bits |= 1;
            *max = counter;
            true
        } else {
            let d = *max - counter;
            if d >= 64 || (*bits >> d) & 1 == 1 {
                false
            } else {
                *bits |= 1 << d;
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> (Cipher, Cipher) {
        (
            Cipher::new("correct horse battery").unwrap(),
            Cipher::new("correct horse battery").unwrap(),
        )
    }

    #[test]
    fn roundtrip() {
        let (a, b) = pair();
        let d = a.seal(1, b"hello ipv4");
        assert_eq!(d.len(), OVERHEAD + 10);
        let (kind, p) = b.open(&d, &mut ReplayGuard::new()).unwrap();
        assert_eq!((kind, p.as_slice()), (1, &b"hello ipv4"[..]));
    }

    #[test]
    fn wrong_key_rejected() {
        let a = Cipher::new("correct horse battery").unwrap();
        let b = Cipher::new("incorrect horse battery").unwrap();
        assert!(b.open(&a.seal(1, b"x"), &mut ReplayGuard::new()).is_none());
    }

    #[test]
    fn tamper_rejected() {
        let (a, b) = pair();
        let mut g = ReplayGuard::new();
        // flip a ciphertext byte, a header byte (type), and a counter byte
        for idx in [HEADER_LEN + 1, 4, 20] {
            let mut d = a.seal(1, b"payload");
            d[idx] ^= 0x01;
            assert!(b.open(&d, &mut g).is_none(), "byte {idx}");
        }
    }

    #[test]
    fn replay_rejected_but_reorder_ok() {
        let (a, b) = pair();
        let mut g = ReplayGuard::new();
        let d1 = a.seal(1, b"1");
        let d2 = a.seal(1, b"2");
        assert!(b.open(&d2, &mut g).is_some());
        assert!(b.open(&d1, &mut g).is_some()); // late but new
        assert!(b.open(&d1, &mut g).is_none()); // replay
        assert!(b.open(&d2, &mut g).is_none()); // replay
    }

    #[test]
    fn own_packet_reflection_rejected() {
        let (a, _) = pair();
        let d = a.seal(1, b"x");
        assert!(a.open(&d, &mut ReplayGuard::new()).is_none());
    }

    #[test]
    fn garbage_and_plaintext_rejected() {
        let (a, _) = pair();
        let mut g = ReplayGuard::new();
        assert!(a.open(b"HELLO:peer1", &mut g).is_none());
        assert!(a.open(&[], &mut g).is_none());
        assert!(a.open(&[0x4F, 0x58, 0x44, 0x01, 0x01, 0, 0, 0, 0], &mut g).is_none());
    }

    #[test]
    fn short_key_rejected() {
        assert!(Cipher::new("short").is_err());
    }
}
