//! PuTTY private key files (`.ppk`) for Tools > SSH key generator (S7).
//!
//! Reading versions 2 and 3 is done by `ssh-key` (`PrivateKey::from_ppk`); this module
//! writes version 3, the format of PuTTY 0.75 and later, as described in
//! <https://the.earth.li/~sgtatham/putty/0.80/htmldoc/AppendixC.html>: an unencrypted file
//! is MACed with HMAC-SHA-256 under an empty key; an encrypted one derives the AES-256-CBC
//! key, IV and MAC key from the passphrase with Argon2id.

use hmac::{Hmac, KeyInit, Mac};
use russh::keys::ssh_key::encoding::Encode;
use russh::keys::ssh_key::encoding::base64::{Base64, Encoding};
use russh::keys::ssh_key::private::KeypairData;
use russh::keys::ssh_key::rand_core::CryptoRng;
use russh::keys::ssh_key::sha2::Sha256;
use russh::keys::ssh_key::{Cipher, Mpint, PrivateKey};
use zeroize::Zeroizing;

/// Argon2id memory in KiB, as PuTTYgen uses by default.
pub const ARGON2_MEMORY_KIB: u32 = 8192;
/// Argon2id passes. PuTTYgen tunes them to take about 0.1 s on the machine; this is a
/// fixed value in the same range (its own test vectors use 34).
pub const ARGON2_PASSES: u32 = 34;
/// Base64 characters per line, as PuTTYgen writes them.
const LINE: usize = 64;
/// AES block size: the encrypted private blob is padded to a multiple of it.
const BLOCK: usize = 16;

/// The Argon2id parameters and salt stored in an encrypted file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Argon2Params {
    pub memory_kib: u32,
    pub passes: u32,
    pub parallelism: u32,
    pub salt: Vec<u8>,
}

/// How [`encode_with`] encrypts the private blob: passphrase, Argon2id parameters and
/// the padding bytes (only as many as needed are used).
pub struct Encryption<'a> {
    pub passphrase: &'a str,
    pub params: &'a Argon2Params,
    pub padding: &'a [u8],
}

/// `key` (decrypted) as a `.ppk` version 3 file, encrypted with `passphrase` when it is
/// not empty, with a random salt and padding.
pub fn encode<R: CryptoRng + ?Sized>(
    key: &PrivateKey,
    passphrase: Option<&str>,
    rng: &mut R,
) -> Result<Zeroizing<String>, String> {
    match passphrase.filter(|p| !p.is_empty()) {
        None => encode_with(key, None),
        Some(passphrase) => {
            let mut salt = vec![0u8; 16];
            rng.fill_bytes(&mut salt);
            let mut padding = [0u8; BLOCK];
            rng.fill_bytes(&mut padding);
            let params = Argon2Params {
                memory_kib: ARGON2_MEMORY_KIB,
                passes: ARGON2_PASSES,
                parallelism: 1,
                salt,
            };
            encode_with(
                key,
                Some(Encryption {
                    passphrase,
                    params: &params,
                    padding: &padding,
                }),
            )
        }
    }
}

/// The private fields PuTTY stores for each key type (Appendix C.3).
fn private_blob(key: &PrivateKey) -> Result<Zeroizing<Vec<u8>>, String> {
    let mut blob = Zeroizing::new(Vec::new());
    let encoded = match key.key_data() {
        KeypairData::Ed25519(pair) => pair.private.as_ref().as_slice().encode(&mut *blob),
        KeypairData::Rsa(pair) => {
            let private = pair.private();
            [private.d(), private.p(), private.q(), private.iqmp()]
                .into_iter()
                .try_for_each(|value| value.encode(&mut *blob))
        }
        KeypairData::Ecdsa(pair) => Mpint::from_positive_bytes(pair.private_key_bytes()).encode(&mut *blob),
        _ => return Err("only Ed25519, RSA and ECDSA keys can be saved as PuTTY keys".into()),
    };
    encoded.map_err(|err| format!("cannot encode the private key: {err}"))?;
    Ok(blob)
}

/// The 80 bytes Argon2id derives, split: AES key (32), IV (16) and MAC key (32).
/// Zeroized on drop.
struct Derived {
    key: Zeroizing<[u8; 32]>,
    iv: Zeroizing<[u8; 16]>,
    mac_key: Zeroizing<[u8; 32]>,
}

fn derive(passphrase: &str, params: &Argon2Params) -> Result<Derived, String> {
    let argon2 = argon2::Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(params.memory_kib, params.passes, params.parallelism, None)
            .map_err(|err| format!("invalid Argon2 parameters: {err}"))?,
    );
    let mut output = Zeroizing::new([0u8; 80]);
    argon2
        .hash_password_into(passphrase.as_bytes(), &params.salt, &mut *output)
        .map_err(|err| format!("cannot derive the key: {err}"))?;
    let mut derived = Derived {
        key: Zeroizing::new([0; 32]),
        iv: Zeroizing::new([0; 16]),
        mac_key: Zeroizing::new([0; 32]),
    };
    let (key, rest) = output.split_at(32);
    let (iv, mac_key) = rest.split_at(16);
    derived.key.copy_from_slice(key);
    derived.iv.copy_from_slice(iv);
    derived.mac_key.copy_from_slice(mac_key);
    Ok(derived)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `label: n` followed by `data` in base64, [`LINE`] characters per line.
fn push_lines(out: &mut String, label: &str, data: &[u8]) {
    let encoded = Zeroizing::new(Base64::encode_string(data));
    let lines: Vec<&str> = encoded
        .as_bytes()
        .chunks(LINE)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
        .collect();
    out.push_str(&format!("{label}: {}\n", lines.len()));
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
}

/// `key` (decrypted) as a `.ppk` version 3 file, unencrypted or as `encryption` says.
pub fn encode_with(key: &PrivateKey, encryption: Option<Encryption<'_>>) -> Result<Zeroizing<String>, String> {
    if key.is_encrypted() {
        return Err("the key must be decrypted first".into());
    }
    let algorithm = key.algorithm();
    let algorithm = algorithm.as_str();
    let comment = key.comment().as_str_lossy();
    let public = key
        .public_key()
        .to_bytes()
        .map_err(|err| format!("cannot encode the public key: {err}"))?;
    let mut private = private_blob(key)?;

    let (kind, derived) = match &encryption {
        None => ("none", None),
        Some(encryption) => {
            let missing = (BLOCK - private.len() % BLOCK) % BLOCK;
            let padding = encryption.padding.get(..missing).ok_or("not enough padding")?;
            private.extend_from_slice(padding);
            let derived = derive(encryption.passphrase, encryption.params)?;
            ("aes256-cbc", Some(derived))
        }
    };

    let mac_key: &[u8] = derived.as_ref().map_or(&[], |d| d.mac_key.as_slice());
    let mut mac = Hmac::<Sha256>::new_from_slice(mac_key).map_err(|err| format!("cannot compute the MAC: {err}"))?;
    for field in [
        algorithm.as_bytes(),
        kind.as_bytes(),
        comment.as_bytes(),
        &public,
        &private,
    ] {
        mac.update(&(field.len() as u32).to_be_bytes());
        mac.update(field);
    }
    let mac = mac.finalize().into_bytes();

    if let Some(derived) = &derived {
        Cipher::Aes256Cbc
            .encrypt(derived.key.as_slice(), derived.iv.as_slice(), &mut private)
            .map_err(|err| format!("cannot encrypt the key: {err}"))?;
    }

    let mut out = Zeroizing::new(String::new());
    out.push_str(&format!("PuTTY-User-Key-File-3: {algorithm}\n"));
    out.push_str(&format!("Encryption: {kind}\n"));
    out.push_str(&format!("Comment: {comment}\n"));
    push_lines(&mut out, "Public-Lines", &public);
    if let Some(encryption) = &encryption {
        let params = encryption.params;
        out.push_str(&format!(
            "Key-Derivation: Argon2id\nArgon2-Memory: {}\nArgon2-Passes: {}\nArgon2-Parallelism: {}\nArgon2-Salt: {}\n",
            params.memory_kib,
            params.passes,
            params.parallelism,
            hex(&params.salt)
        ));
    }
    push_lines(&mut out, "Private-Lines", &private);
    out.push_str(&format!("Private-MAC: {}\n", hex(&mac)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    //! The `testdata` vectors come from the `ssh-key` crate (MIT/Apache-2.0): OpenSSH keys
    //! and the same keys converted by PuTTYgen, encrypted ones with the passphrase "123".
    use super::*;

    const ED25519: &str = include_str!("testdata/id_ed25519");
    const ECDSA_P256: &str = include_str!("testdata/id_ecdsa_p256");
    const RSA_3072: &str = include_str!("testdata/id_rsa_3072");

    fn openssh(text: &str) -> PrivateKey {
        PrivateKey::from_openssh(text).unwrap()
    }

    fn value<'a>(ppk: &'a str, name: &str) -> &'a str {
        ppk.lines()
            .find_map(|line| line.strip_prefix(&format!("{name}: ")))
            .unwrap()
    }

    fn unhex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    /// The Argon2 parameters of a PuTTYgen file and the padding it used (decrypted).
    fn puttygen_encryption(ppk: &str, passphrase: &str) -> (Argon2Params, Vec<u8>) {
        let params = Argon2Params {
            memory_kib: value(ppk, "Argon2-Memory").parse().unwrap(),
            passes: value(ppk, "Argon2-Passes").parse().unwrap(),
            parallelism: value(ppk, "Argon2-Parallelism").parse().unwrap(),
            salt: unhex(value(ppk, "Argon2-Salt")),
        };
        let mut lines = ppk.lines().skip_while(|l| !l.starts_with("Private-Lines: "));
        let count: usize = lines.next().unwrap()["Private-Lines: ".len()..].parse().unwrap();
        let text: String = lines.take(count).collect();
        let mut private = Base64::decode_vec(&text).unwrap();
        let derived = derive(passphrase, &params).unwrap();
        Cipher::Aes256Cbc
            .decrypt(derived.key.as_slice(), derived.iv.as_slice(), &mut private, None)
            .unwrap();
        (params, private)
    }

    #[test]
    fn unencrypted_files_match_puttygen_byte_for_byte() {
        for (key, ppk) in [
            (ED25519, include_str!("testdata/id_ed25519.ppk")),
            (ECDSA_P256, include_str!("testdata/id_ecdsa_p256.ppk")),
            (RSA_3072, include_str!("testdata/id_rsa_3072.ppk")),
        ] {
            let written = encode_with(&openssh(key), None).unwrap();
            assert_eq!(written.as_str(), ppk);
        }
    }

    #[test]
    fn encrypted_files_match_puttygen_byte_for_byte() {
        for (key, ppk) in [
            (ED25519, include_str!("testdata/id_ed25519_enc.ppk")),
            (ECDSA_P256, include_str!("testdata/id_ecdsa_p256_enc.ppk")),
        ] {
            let key = openssh(key);
            let (params, decrypted) = puttygen_encryption(ppk, "123");
            // The padding is whatever follows the private fields PuTTYgen wrote.
            let unpadded = private_blob(&key).unwrap().len();
            let written = encode_with(
                &key,
                Some(Encryption {
                    passphrase: "123",
                    params: &params,
                    padding: &decrypted[unpadded..],
                }),
            )
            .unwrap();
            assert_eq!(written.as_str(), ppk);
        }
    }

    #[test]
    fn random_salt_and_padding_still_read_back() {
        let key = openssh(ED25519);
        let mut rng = russh::keys::ssh_key::rand_core::UnwrapErr(rand::rngs::SysRng);
        let first = encode(&key, Some("pw"), &mut rng).unwrap();
        let second = encode(&key, Some("pw"), &mut rng).unwrap();
        assert_ne!(first.as_str(), second.as_str());
        assert!(first.contains("Key-Derivation: Argon2id\n"));
        let read = PrivateKey::from_ppk(first.as_str(), Some("pw".into())).unwrap();
        assert_eq!(read.key_data(), key.key_data());
        assert_eq!(read.comment(), key.comment());
        assert!(PrivateKey::from_ppk(first.as_str(), Some("wrong".into())).is_err());
        // No passphrase (or an empty one): not encrypted.
        let plain = encode(&key, Some(""), &mut rng).unwrap();
        assert!(plain.contains("Encryption: none\n"));
        assert_eq!(
            PrivateKey::from_ppk(plain.as_str(), None).unwrap().key_data(),
            key.key_data()
        );
    }
}
