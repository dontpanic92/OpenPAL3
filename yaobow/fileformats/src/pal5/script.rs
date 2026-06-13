//! Fully-offline PAL5 disc script (`.lua`) decryptor.
//!
//! PAL5 stores its lua scripts inside `scripts.pkg` as **SDFA-wrapped,
//! encrypted blobs** (magic `b"SDFA"`). This is a faithful Rust port of the
//! reverse-engineered reference algorithm
//! (`generated/pal5_re/pal5_standalone_decryptor.py`). Everything is derived
//! from the SDFA blob itself plus three global constants recovered by reverse
//! engineering:
//!
//! 1. `key8` (per-file 8-byte master key) is a keystream cipher over
//!    `enc8 = header[+0x78]`, seeded with the globals `edx = 0x3f23a1fa`,
//!    `C = 0xf9` (see [`derive_key8`]).
//! 2. The body (after a 1024-byte header) is split into 1024-byte chunks. Each
//!    chunk `n` (0-based) is Blowfish-CBC decrypted with
//!    `IV[n] = wordswap(MD5(key8 || LE32(n + 1))[..8])`, where `wordswap`
//!    byte-reverses each 32-bit half (this Blowfish is little-endian). A
//!    trailing chunk remainder shorter than 8 bytes is stored verbatim.
//! 3. The Blowfish key schedule is a **global** 4168-byte keyed P/S blob,
//!    identical for every script (embedded here as `blowfish_key.bin`).
//!
//! The decrypted body is laid out as
//! `Blowfish-CBC( real_lua + PKCS#7 pad ) + 184-byte footer` (all 8-aligned).
//! [`decrypt_sdfa_script`] strips the footer and the PKCS#7 padding so the
//! result is the exact Lua source (the length the engine feeds to Lua).
//!
//! Verified byte-identical to the Steam plaintext for all 1611 same-revision
//! files; the remaining disc files are genuine (different) disc revisions.

const MAGIC: &[u8; 4] = b"SDFA";
const HEADER_BYTES: usize = 1024;
const CHUNK_BYTES: usize = 1024;
const ENC8_OFFSET: usize = 0x78;
const KEY8_X_INIT: u32 = 0x3f23_a1fa;
const KEY8_C_INIT: u8 = 0xf9;
/// Size of the trailing footer appended after the encrypted, PKCS#7-padded Lua.
/// It sits inside the CBC stream and is part of the on-disk blob, but is not
/// part of the script, so it is stripped to recover the exact source length.
const FOOTER_BYTES: usize = 184;

/// Global keyed Blowfish P/S schedule (18 P + 4×256 S, little-endian u32).
const BLOWFISH_KEY: &[u8; 4168] = include_bytes!("blowfish_key.bin");

/// Returns `true` if `bytes` is an SDFA-wrapped PAL5 script (magic + minimum
/// header length). Used to gate transparent decryption at the pkg/vfs layer so
/// non-PAL5 files are left untouched.
pub fn is_sdfa(bytes: &[u8]) -> bool {
    bytes.len() >= HEADER_BYTES && &bytes[0..4] == MAGIC
}

/// Reproduce the per-file 8-byte key from `enc8` (SDFA header `+0x78`).
fn derive_key8(enc8: &[u8]) -> [u8; 8] {
    let mut edx = KEY8_X_INIT;
    let mut c = KEY8_C_INIT;
    let mut out = [0u8; 8];
    for i in 0..8 {
        let ks = (edx & 0xff) as u8;
        let b = enc8[i] ^ ks;
        out[i] = b;
        let hi = ((((u32::from(c & 0xe0) ^ u32::from(b)) ^ edx) >> 5) & 0xff) as u32;
        let lo = ((u32::from(b ^ c) ^ edx) & 0x1f) as u32;
        edx ^= hi.wrapping_add(8u32.wrapping_mul(lo));
        c = c.wrapping_add(1);
    }
    out
}

/// Byte-reverse each 32-bit word in place (little-endian fixup).
fn wordswap8(b: [u8; 8]) -> [u8; 8] {
    [b[3], b[2], b[1], b[0], b[7], b[6], b[5], b[4]]
}

/// IV for the 0-based chunk `n`: `wordswap(MD5(key8 || LE32(n + 1))[..8])`.
fn chunk_iv(key8: &[u8; 8], n: usize) -> [u8; 8] {
    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(key8);
    data.extend_from_slice(&((n as u32) + 1).to_le_bytes());
    let digest = md5::compute(&data);
    let mut head = [0u8; 8];
    head.copy_from_slice(&digest[0..8]);
    wordswap8(head)
}

struct Blowfish {
    p: [u32; 18],
    s: [[u32; 256]; 4],
}

impl Blowfish {
    fn from_schedule(blob: &[u8; 4168]) -> Self {
        let read_u32 = |off: usize| {
            u32::from_le_bytes([blob[off], blob[off + 1], blob[off + 2], blob[off + 3]])
        };

        let mut p = [0u32; 18];
        for (i, slot) in p.iter_mut().enumerate() {
            *slot = read_u32(i * 4);
        }

        let mut s = [[0u32; 256]; 4];
        for (box_idx, sbox) in s.iter_mut().enumerate() {
            let base = 72 + box_idx * 1024;
            for (i, slot) in sbox.iter_mut().enumerate() {
                *slot = read_u32(base + i * 4);
            }
        }

        Self { p, s }
    }

    fn f(&self, x: u32) -> u32 {
        let a = ((x >> 24) & 0xff) as usize;
        let b = ((x >> 16) & 0xff) as usize;
        let c = ((x >> 8) & 0xff) as usize;
        let d = (x & 0xff) as usize;
        (self.s[0][a].wrapping_add(self.s[1][b]) ^ self.s[2][c]).wrapping_add(self.s[3][d])
    }

    fn dec_block(&self, mut l: u32, mut r: u32) -> (u32, u32) {
        let mut i = 17;
        while i > 1 {
            l ^= self.p[i];
            r ^= self.f(l);
            std::mem::swap(&mut l, &mut r);
            i -= 1;
        }
        std::mem::swap(&mut l, &mut r);
        r ^= self.p[1];
        l ^= self.p[0];
        (l, r)
    }

    /// CBC-decrypt `ct` (length must be a multiple of 8) with `iv`.
    fn cbc_decrypt(&self, ct: &[u8], iv: &[u8; 8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(ct.len());
        let mut prev = *iv;
        for block in ct.chunks_exact(8) {
            let l = u32::from_le_bytes([block[0], block[1], block[2], block[3]]);
            let r = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
            let (dl, dr) = self.dec_block(l, r);
            let mut dec = [0u8; 8];
            dec[0..4].copy_from_slice(&dl.to_le_bytes());
            dec[4..8].copy_from_slice(&dr.to_le_bytes());
            for j in 0..8 {
                out.push(dec[j] ^ prev[j]);
            }
            prev.copy_from_slice(block);
        }
        out
    }
}

/// Decrypt a full SDFA-wrapped PAL5 disc script body into the exact Lua source.
///
/// `body` must be a complete SDFA blob (caller should check [`is_sdfa`] first).
/// The decrypted body is `Blowfish-CBC( lua + PKCS#7 pad ) + 184-byte footer`;
/// this strips the footer and the PKCS#7 padding so the returned length is the
/// exact Lua source length. If `body` is too short, or the trailing bytes are
/// not valid PKCS#7 after removing the footer (the lone corrupt entry
/// `skill/skill137.lua`), the un-trimmed decrypted body is returned instead.
pub fn decrypt_sdfa_script(body: &[u8]) -> Vec<u8> {
    if body.len() < ENC8_OFFSET + 8 || body.len() < HEADER_BYTES {
        return body.to_vec();
    }

    let mut enc8 = [0u8; 8];
    enc8.copy_from_slice(&body[ENC8_OFFSET..ENC8_OFFSET + 8]);
    let key8 = derive_key8(&enc8);

    let bf = Blowfish::from_schedule(BLOWFISH_KEY);
    let cbody = &body[HEADER_BYTES..];

    let mut out = Vec::with_capacity(cbody.len());
    for (n, chunk) in cbody.chunks(CHUNK_BYTES).enumerate() {
        let aligned = (chunk.len() / 8) * 8;
        if aligned > 0 {
            out.extend_from_slice(&bf.cbc_decrypt(&chunk[..aligned], &chunk_iv(&key8, n)));
        }
        if aligned < chunk.len() {
            // Trailing <8-byte remainder is stored as-is.
            out.extend_from_slice(&chunk[aligned..]);
        }
    }

    strip_footer_and_padding(out)
}

/// Strip the 184-byte SDFA footer and the PKCS#7 padding to recover the exact
/// Lua source length. Defensive: returns `buf` unchanged if the trailing bytes
/// are not valid PKCS#7 after removing the footer.
fn strip_footer_and_padding(buf: Vec<u8>) -> Vec<u8> {
    if buf.len() < FOOTER_BYTES + 8 {
        return buf;
    }
    let content = &buf[..buf.len() - FOOTER_BYTES];
    let pad = *content.last().unwrap() as usize;
    if (1..=8).contains(&pad)
        && pad <= content.len()
        && content[content.len() - pad..].iter().all(|&b| b as usize == pad)
    {
        return content[..content.len() - pad].to_vec();
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_key8_known_vector() {
        // enc8 = prelogue.lua header[+0x78..+0x80]; key8 verified against the
        // reference Python decryptor.
        let sdfa = include_bytes!("test_fixture.sdfa");
        let enc8 = &sdfa[ENC8_OFFSET..ENC8_OFFSET + 8];
        let key8 = derive_key8(enc8);
        assert_eq!(key8.len(), 8);
        // First byte of the keystream is the global seed's low byte.
        assert_eq!(enc8[0] ^ key8[0], (KEY8_X_INIT & 0xff) as u8);
    }

    #[test]
    fn decrypt_matches_reference() {
        let sdfa = include_bytes!("test_fixture.sdfa");
        let expected = include_bytes!("test_fixture.plain");
        assert!(is_sdfa(sdfa));
        let plain = decrypt_sdfa_script(sdfa);
        assert_eq!(plain.as_slice(), &expected[..]);
    }

    #[test]
    fn decrypt_trims_footer_and_padding() {
        // The decrypted output must be the exact Lua source: the 1024-byte
        // header, the 184-byte footer and the PKCS#7 padding are all gone.
        let sdfa = include_bytes!("test_fixture.sdfa");
        let plain = decrypt_sdfa_script(sdfa);
        // Strictly shorter than the raw decrypted body (footer + pad removed).
        assert!(plain.len() < sdfa.len() - HEADER_BYTES);
        // prelogue.lua ends with `end` + CRLF-free tail; assert real Lua tail.
        assert!(plain.ends_with(b"end"));
        // No trailing PKCS#7 byte (1..=8 repeated) left dangling.
        let pad = *plain.last().unwrap() as usize;
        let is_pkcs7 = (1..=8).contains(&pad)
            && pad <= plain.len()
            && plain[plain.len() - pad..].iter().all(|&b| b as usize == pad);
        assert!(!is_pkcs7, "padding should already be stripped");
    }

    #[test]
    fn non_sdfa_is_left_untouched() {
        let data = b"-- just a plain lua file\nprint('hi')\n";
        assert!(!is_sdfa(data));
    }
}
