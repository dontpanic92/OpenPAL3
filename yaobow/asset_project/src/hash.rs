//! Content hashing used for payload/manifest/patch integrity checks.
//!
//! SHA-256 is used (rather than the engine's xxh3, which
//! `ypk` uses purely for fast path-hash lookups)
//! because `ContentHash` here backs security-relevant integrity
//! checks: payload-store keys, the `.ybpatch` manifest hash, package
//! fingerprints, and `base_entry_hash` conflict detection. Patches are
//! expected to move across machines and trust boundaries (e.g.
//! downloaded from a distribution server), so collision resistance
//! actually matters here, unlike xxh3's in-process lookup use case.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer, de::Deserializer};
use sha2::{Digest, Sha256};

pub const CONTENT_HASH_BYTES: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash([u8; CONTENT_HASH_BYTES]);

impl ContentHash {
    pub fn of(data: &[u8]) -> Self {
        let digest = Sha256::digest(data);
        let mut bytes = [0u8; CONTENT_HASH_BYTES];
        bytes.copy_from_slice(&digest);
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; CONTENT_HASH_BYTES] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        use std::fmt::Write;
        let mut s = String::with_capacity(CONTENT_HASH_BYTES * 2);
        for byte in &self.0 {
            write!(s, "{byte:02x}").expect("writing to a String cannot fail");
        }
        s
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.len() != CONTENT_HASH_BYTES * 2 {
            return None;
        }
        let mut bytes = [0u8; CONTENT_HASH_BYTES];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Self(bytes))
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({})", self.to_hex())
    }
}

// Serialized as a fixed-width hex string rather than a byte array so
// manifests stay human-diffable and compact in JSON.
impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        ContentHash::from_hex(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid content hash {:?}", s)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let hash = ContentHash::of(b"hello world");
        let hex = hash.to_hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(ContentHash::from_hex(&hex), Some(hash));
    }

    #[test]
    fn same_bytes_same_hash() {
        assert_eq!(ContentHash::of(b"abc"), ContentHash::of(b"abc"));
        assert_ne!(ContentHash::of(b"abc"), ContentHash::of(b"abd"));
    }

    #[test]
    fn matches_known_sha256_vector() {
        // Published NIST test vector: SHA-256("abc").
        let hash = ContentHash::of(b"abc");
        assert_eq!(
            hash.to_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn json_roundtrip() {
        let hash = ContentHash::of(b"payload bytes");
        let json = serde_json::to_string(&hash).unwrap();
        assert_eq!(json, format!("\"{}\"", hash.to_hex()));
        let back: ContentHash = serde_json::from_str(&json).unwrap();
        assert_eq!(back, hash);
    }

    #[test]
    fn rejects_invalid_hex() {
        assert!(
            ContentHash::from_hex("not-hex-but-right-length-000000000000000000000000000000")
                .is_none()
        );
        assert!(ContentHash::from_hex("abcd").is_none()); // too short
    }
}
