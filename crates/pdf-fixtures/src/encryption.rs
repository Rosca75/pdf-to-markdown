//! PDF Standard Security Handler: RC4-128 (V2/R3) and AES-128 (V4/R4).

use crate::obj::{name, Dict, Obj};
use aes::cipher::{block_padding::Pkcs7, BlockModeEncrypt, KeyIvInit};
use md5::{Digest, Md5};

const PAD: [u8; 32] = [
    0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01, 0x08,
    0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53, 0x69, 0x7A,
];

/// Cipher used for strings and streams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// RC4 with a 128-bit key (V2, R3).
    Rc4_128,
    /// AES-128 CBC (V4, R4, crypt filter AESV2).
    Aes128,
}

/// Encryption settings requested by a fixture.
#[derive(Clone, Debug)]
pub struct Encryption {
    pub user_password: Vec<u8>,
    pub owner_password: Vec<u8>,
    pub method: Method,
    /// Permission flags (`/P`).
    pub permissions: i32,
}

impl Encryption {
    pub fn rc4(user: &str, owner: &str) -> Self {
        Encryption {
            user_password: user.as_bytes().to_vec(),
            owner_password: owner.as_bytes().to_vec(),
            method: Method::Rc4_128,
            permissions: -44,
        }
    }

    pub fn aes128(user: &str, owner: &str) -> Self {
        Encryption {
            method: Method::Aes128,
            ..Self::rc4(user, owner)
        }
    }
}

/// Hand-rolled RC4.
pub fn rc4(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut s: Vec<u8> = (0..=255u8).collect();
    let mut j = 0usize;
    for i in 0..256 {
        j = (j + s[i] as usize + key[i % key.len()] as usize) & 255;
        s.swap(i, j);
    }
    let (mut i, mut j) = (0usize, 0usize);
    data.iter()
        .map(|&b| {
            i = (i + 1) & 255;
            j = (j + s[i] as usize) & 255;
            s.swap(i, j);
            b ^ s[(s[i] as usize + s[j] as usize) & 255]
        })
        .collect()
}

fn md5(parts: &[&[u8]]) -> [u8; 16] {
    let mut h = Md5::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

fn pad(pw: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let n = pw.len().min(32);
    out[..n].copy_from_slice(&pw[..n]);
    out[n..].copy_from_slice(&PAD[..32 - n]);
    out
}

/// Computed encryption state for a document.
pub struct Crypt {
    key: Vec<u8>,
    method: Method,
    o: Vec<u8>,
    u: Vec<u8>,
    p: i32,
}

impl Crypt {
    pub fn new(e: &Encryption, id0: &[u8]) -> Crypt {
        // Algorithm 3: owner entry.
        let owner_src = if e.owner_password.is_empty() {
            &e.user_password
        } else {
            &e.owner_password
        };
        let mut h = md5(&[&pad(owner_src)]);
        for _ in 0..50 {
            h = md5(&[&h]);
        }
        let okey = &h[..16];
        let mut o = rc4(okey, &pad(&e.user_password));
        for i in 1..=19u8 {
            let k: Vec<u8> = okey.iter().map(|b| b ^ i).collect();
            o = rc4(&k, &o);
        }
        // Algorithm 2: file key.
        let p = e.permissions;
        let mut k = md5(&[&pad(&e.user_password), &o, &p.to_le_bytes(), id0]);
        for _ in 0..50 {
            k = md5(&[&k[..16]]);
        }
        let key = k[..16].to_vec();
        // Algorithm 5: user entry (R3+).
        let mut u = rc4(&key, &md5(&[&PAD, id0]));
        for i in 1..=19u8 {
            let kk: Vec<u8> = key.iter().map(|b| b ^ i).collect();
            u = rc4(&kk, &u);
        }
        u.extend_from_slice(&[0u8; 16]);
        Crypt {
            key,
            method: e.method,
            o,
            u,
            p,
        }
    }

    /// The `/Encrypt` dictionary.
    pub fn dict(&self) -> Dict {
        let mut d = Dict::new()
            .with("Filter", name("Standard"))
            .with("Length", 128);
        match self.method {
            Method::Rc4_128 => {
                d.set("V", 2);
                d.set("R", 3);
            }
            Method::Aes128 => {
                d.set("V", 4);
                d.set("R", 4);
                let cf = Dict::new().with(
                    "StdCF",
                    Dict::new()
                        .with("CFM", name("AESV2"))
                        .with("AuthEvent", name("DocOpen"))
                        .with("Length", 16),
                );
                d.set("CF", cf);
                d.set("StmF", name("StdCF"));
                d.set("StrF", name("StdCF"));
            }
        }
        d.set("O", Obj::Str(self.o.clone()));
        d.set("U", Obj::Str(self.u.clone()));
        d.set("P", self.p as i64);
        d
    }

    /// Encrypt a string or stream payload belonging to object `num`.
    pub fn encrypt(&self, num: u32, data: &[u8]) -> Vec<u8> {
        let n = num.to_le_bytes();
        let gen = [0u8, 0u8];
        let salt: &[u8] = if self.method == Method::Aes128 {
            b"sAlT"
        } else {
            b""
        };
        let k = md5(&[&self.key, &n[..3], &gen, salt]);
        let klen = (self.key.len() + 5).min(16);
        let okey = &k[..klen];
        match self.method {
            Method::Rc4_128 => rc4(okey, data),
            Method::Aes128 => {
                // Deterministic IV derived from the object number.
                let iv = md5(&[b"pdf-fixtures-iv", &n]);
                let enc = cbc::Encryptor::<aes::Aes128>::new_from_slices(okey, &iv)
                    .expect("key/iv length");
                let mut out = iv.to_vec();
                out.extend_from_slice(&enc.encrypt_padded_vec::<Pkcs7>(data));
                out
            }
        }
    }
}
