//! Tools > SSH key generator (S2, S7): generates Ed25519, RSA and ECDSA key pairs or loads
//! an existing private key (OpenSSH or PuTTY `.ppk`), lets the comment and passphrase
//! change, saves the public key, the private key (OpenSSH) or a PuTTY `.ppk` file, and
//! installs the public key on a server like `ssh-copy-id`.
//!
//! The private key stays in the backend: [`generate`] and [`load`] keep it in a
//! [`KeyStore`] under an id and return only its public half, which the other functions
//! then refer to. The private key only ever leaves memory as the files written by
//! [`save_private`], [`export_ppk`] and [`save`].
//!
//! Ported from PlexusTerm v0.2.15 `src-tauri/src/tools/keygen.rs` without Tauri: the caller
//! owns the [`KeyStore`] (v0.2.15 kept one global store for its commands) and runs these
//! functions off its UI thread (bcrypt-pbkdf, Argon2 and RSA generation take a while).

use russh::keys::ssh_key::private::{KeypairData, RsaKeypair};
use russh::keys::ssh_key::rand_core::UnwrapErr;
use russh::keys::ssh_key::{EcdsaCurve, LineEnding};
use russh::keys::{Algorithm, HashAlg, PrivateKey};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use zeroize::Zeroizing;

mod install;
mod ppk;

pub use install::{InstallOutcome, install, install_command};

use crate::lock;

/// Longest comment accepted (OpenSSH has no limit; this keeps lines readable).
const MAX_COMMENT: usize = 256;

/// Keys kept at once; the oldest is dropped beyond this.
const MAX_KEYS: usize = 8;

/// Largest key file [`load`] reads (an RSA 16384 `.ppk` is about 12 KiB).
const MAX_KEY_FILE: u64 = 64 * 1024;

/// Errors of generating, loading and saving keys (v0.2.15 messages).
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum KeygenError {
    #[error("{message}")]
    Invalid { message: String },
    /// `path` already exists and `overwrite` was not set.
    #[error("{path} already exists")]
    Exists { path: String },
    #[error("the generated key is no longer available; generate it again")]
    UnknownKey,
    #[error("{message}")]
    Io { message: String },
    /// The key file is encrypted and no passphrase was given.
    #[error("the key is protected by a passphrase")]
    PassphraseRequired,
    #[error("wrong passphrase")]
    WrongPassphrase,
}

fn invalid(message: impl Into<String>) -> KeygenError {
    KeygenError::Invalid {
        message: message.into(),
    }
}

/// The key types and sizes offered (S7): no DSA, no SSH-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyType {
    #[default]
    Ed25519,
    Rsa2048,
    Rsa3072,
    Rsa4096,
    EcdsaP256,
    EcdsaP384,
    EcdsaP521,
}

/// What callers get back for a key: never the private key.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedKey {
    pub id: u32,
    /// One `authorized_keys` line: `ssh-ed25519 AAAA... comment`.
    pub public_key: String,
    /// `SHA256:...`, as printed by `ssh-keygen -l`.
    pub fingerprint: String,
    pub comment: String,
    /// `ED25519`, `RSA 3072`, `ECDSA P-256`...
    pub key_type: String,
    /// Whether the saved private key is protected by a passphrase.
    pub encrypted: bool,
}

/// A key pair. Deliberately not `Debug`: it holds the private key.
struct StoredKey {
    /// The decrypted key (ssh-key zeroizes its private parts on drop).
    key: PrivateKey,
    /// Its passphrase; empty means none.
    passphrase: Zeroizing<String>,
    /// OpenSSH private key file contents, encrypted when there is a passphrase.
    private_openssh: Zeroizing<String>,
    public_key: String,
}

/// Keys by id, until the dialog that made or loaded them is closed.
#[derive(Default)]
pub struct KeyStore {
    next_id: AtomicU32,
    keys: Mutex<HashMap<u32, StoredKey>>,
}

impl KeyStore {
    fn insert(&self, key: StoredKey) -> u32 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        let mut keys = lock(&self.keys);
        if keys.len() >= MAX_KEYS
            && let Some(oldest) = keys.keys().min().copied()
        {
            keys.remove(&oldest);
        }
        keys.insert(id, key);
        id
    }

    /// Replaces key `id`; false when it is no longer there.
    fn replace(&self, id: u32, key: StoredKey) -> bool {
        match lock(&self.keys).get_mut(&id) {
            Some(slot) => {
                *slot = key;
                true
            }
            None => false,
        }
    }

    fn with<T>(&self, id: u32, f: impl FnOnce(&StoredKey) -> T) -> Option<T> {
        lock(&self.keys).get(&id).map(f)
    }

    /// Forgets the key `id` (the dialog was closed); its private parts are zeroized.
    pub fn discard(&self, id: u32) {
        lock(&self.keys).remove(&id);
    }
}

impl std::fmt::Debug for KeyStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyStore")
            .field("keys", &lock(&self.keys).len())
            .finish_non_exhaustive()
    }
}

/// A comment that fits on one `authorized_keys` line.
fn validate_comment(comment: &str) -> Result<&str, KeygenError> {
    let comment = comment.trim();
    if comment.chars().count() > MAX_COMMENT {
        return Err(invalid(format!("the comment must be at most {MAX_COMMENT} characters")));
    }
    if comment.chars().any(char::is_control) {
        return Err(invalid("the comment cannot contain control characters"));
    }
    Ok(comment)
}

fn rng() -> UnwrapErr<rand::rngs::SysRng> {
    UnwrapErr(rand::rngs::SysRng)
}

/// `ED25519`, `RSA 3072`, `ECDSA P-256`; `None` for the types this tool does not handle.
fn type_label(key: &PrivateKey) -> Option<String> {
    match key.key_data() {
        KeypairData::Ed25519(_) => Some("ED25519".into()),
        KeypairData::Rsa(pair) => Some(format!("RSA {}", pair.key_size())),
        KeypairData::Ecdsa(pair) => Some(
            match pair.curve() {
                EcdsaCurve::NistP256 => "ECDSA P-256",
                EcdsaCurve::NistP384 => "ECDSA P-384",
                EcdsaCurve::NistP521 => "ECDSA P-521",
            }
            .into(),
        ),
        _ => None,
    }
}

/// Builds what the store keeps for the decrypted `key` with `passphrase` (empty: none).
fn seal(key: PrivateKey, passphrase: &str) -> Result<StoredKey, KeygenError> {
    let public_key = key
        .public_key()
        .to_openssh()
        .map_err(|err| invalid(format!("cannot encode the public key: {err}")))?;
    let encrypted;
    let file_key = if passphrase.is_empty() {
        &key
    } else {
        encrypted = key
            .encrypt(&mut rng(), passphrase)
            .map_err(|err| invalid(format!("cannot encrypt the key: {err}")))?;
        &encrypted
    };
    let private_openssh = file_key
        .to_openssh(LineEnding::LF)
        .map_err(|err| invalid(format!("cannot encode the private key: {err}")))?;
    Ok(StoredKey {
        key,
        passphrase: Zeroizing::new(passphrase.to_string()),
        private_openssh,
        public_key,
    })
}

/// What callers may see of `stored`.
fn describe(id: u32, stored: &StoredKey) -> GeneratedKey {
    let public = stored.key.public_key();
    GeneratedKey {
        id,
        public_key: stored.public_key.clone(),
        fingerprint: public.fingerprint(HashAlg::Sha256).to_string(),
        comment: public.comment().as_str_lossy().to_string(),
        key_type: type_label(&stored.key).unwrap_or_default(),
        encrypted: !stored.passphrase.is_empty(),
    }
}

/// Keeps `key` (decrypted) in `store` with `passphrase` and describes it.
fn keep(store: &KeyStore, key: PrivateKey, passphrase: &str) -> Result<GeneratedKey, KeygenError> {
    let stored = seal(key, passphrase)?;
    let described = describe(0, &stored);
    let id = store.insert(stored);
    Ok(GeneratedKey { id, ..described })
}

/// Generates a key pair of `key_type` with `comment`, encrypted with `passphrase` when
/// given (empty means none), keeps it in `store` and returns its public half.
pub fn generate(
    store: &KeyStore,
    key_type: KeyType,
    passphrase: Option<&str>,
    comment: &str,
) -> Result<GeneratedKey, KeygenError> {
    let comment = validate_comment(comment)?;
    let mut random = rng();
    let rsa = |bits: usize| -> Result<PrivateKey, russh::keys::ssh_key::Error> {
        PrivateKey::new(KeypairData::from(RsaKeypair::random(&mut rng(), bits)?), "")
    };
    let ecdsa = |curve| Algorithm::Ecdsa { curve };
    let key = match key_type {
        KeyType::Ed25519 => PrivateKey::random(&mut random, Algorithm::Ed25519),
        KeyType::Rsa2048 => rsa(2048),
        KeyType::Rsa3072 => rsa(3072),
        KeyType::Rsa4096 => rsa(4096),
        KeyType::EcdsaP256 => PrivateKey::random(&mut random, ecdsa(EcdsaCurve::NistP256)),
        KeyType::EcdsaP384 => PrivateKey::random(&mut random, ecdsa(EcdsaCurve::NistP384)),
        KeyType::EcdsaP521 => PrivateKey::random(&mut random, ecdsa(EcdsaCurve::NistP521)),
    };
    let mut key = key.map_err(|err| invalid(format!("cannot generate the key: {err}")))?;
    key.set_comment(comment);
    keep(store, key, passphrase.unwrap_or(""))
}

/// Parses a private key file (OpenSSH, or PuTTY `.ppk` v2/v3) and decrypts it with
/// `passphrase` when it is encrypted; also says whether it was.
fn parse_private(contents: &str, passphrase: &str) -> Result<(PrivateKey, bool), KeygenError> {
    if contents.starts_with("PuTTY-User-Key-File-") {
        let encrypted = contents
            .lines()
            .any(|l| l.starts_with("Encryption: ") && l.trim_end() != "Encryption: none");
        if encrypted && passphrase.is_empty() {
            return Err(KeygenError::PassphraseRequired);
        }
        let given = encrypted.then(|| passphrase.to_string());
        let key = PrivateKey::from_ppk(contents, given).map_err(|err| {
            // ssh-key reports a failed MAC check; with a passphrase, that is a wrong one.
            if encrypted && err.to_string().contains("incorrect MAC") {
                KeygenError::WrongPassphrase
            } else {
                invalid(format!("not a valid PuTTY key file: {err}"))
            }
        })?;
        return Ok((key, encrypted));
    }
    let key =
        PrivateKey::from_openssh(contents.trim()).map_err(|_| invalid("not an OpenSSH or PuTTY private key file"))?;
    if !key.is_encrypted() {
        return Ok((key, false));
    }
    if passphrase.is_empty() {
        return Err(KeygenError::PassphraseRequired);
    }
    let key = key.decrypt(passphrase).map_err(|_| KeygenError::WrongPassphrase)?;
    Ok((key, true))
}

/// Loads the private key file at `path` (OpenSSH or PuTTY `.ppk`), decrypting it with
/// `passphrase` when it is encrypted, keeps it in `store` with that passphrase and
/// returns its public half.
pub fn load(store: &KeyStore, path: &Path, passphrase: Option<&str>) -> Result<GeneratedKey, KeygenError> {
    let read_error = |err: std::io::Error| KeygenError::Io {
        message: format!("cannot read {}: {err}", path.display()),
    };
    let file = std::fs::File::open(path).map_err(read_error)?;
    let mut contents = Zeroizing::new(String::new());
    file.take(MAX_KEY_FILE + 1)
        .read_to_string(&mut contents)
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::InvalidData => invalid("not an OpenSSH or PuTTY private key file"),
            _ => read_error(err),
        })?;
    if contents.len() as u64 > MAX_KEY_FILE {
        return Err(invalid("not an OpenSSH or PuTTY private key file"));
    }
    let passphrase = passphrase.unwrap_or("");
    let (key, encrypted) = parse_private(&contents, passphrase)?;
    if type_label(&key).is_none() {
        return Err(invalid(format!(
            "{} keys are not supported (only Ed25519, RSA and ECDSA)",
            key.algorithm()
        )));
    }
    validate_comment(key.comment().as_str_lossy())?;
    // An unencrypted file stays unencrypted even if a passphrase was typed.
    keep(store, key, if encrypted { passphrase } else { "" })
}

/// Sets the comment and passphrase (empty removes it) of key `id`; save it again to
/// keep the change.
pub fn update(store: &KeyStore, id: u32, comment: &str, passphrase: Option<&str>) -> Result<GeneratedKey, KeygenError> {
    let comment = validate_comment(comment)?;
    let mut key = store
        .with(id, |stored| stored.key.clone())
        .ok_or(KeygenError::UnknownKey)?;
    key.set_comment(comment);
    let stored = seal(key, passphrase.unwrap_or(""))?;
    let described = describe(id, &stored);
    if !store.replace(id, stored) {
        return Err(KeygenError::UnknownKey);
    }
    Ok(described)
}

/// `path` with `.pub` appended (`id_ed25519` -> `id_ed25519.pub`).
pub fn public_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".pub");
    PathBuf::from(name)
}

fn io(path: &Path, err: std::io::Error) -> KeygenError {
    KeygenError::Io {
        message: format!("cannot write {}: {err}", path.display()),
    }
}

/// Writes `contents` to `path` with `mode` (Unix), creating it, or replacing it only when
/// `overwrite` is set. The mode is set before any byte is written, also on a replaced file.
fn write_file(path: &Path, contents: &[u8], mode: u32, overwrite: bool) -> Result<(), KeygenError> {
    use std::io::Write;
    if path.as_os_str().is_empty() {
        return Err(invalid("choose where to save the key"));
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if overwrite {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, mode);
    #[cfg(not(unix))]
    let _ = mode;
    let mut file = options.open(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::AlreadyExists => KeygenError::Exists {
            path: path.display().to_string(),
        },
        _ => io(path, err),
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(mode))
            .map_err(|err| io(path, err))?;
    }
    file.write_all(contents).map_err(|err| io(path, err))?;
    file.sync_all().map_err(|err| io(path, err))
}

/// Saves key `id` as `path` (private, 0600) and `path.pub` (public, 0644). Existing files
/// are never replaced unless `overwrite` is set: [`KeygenError::Exists`] names the first one.
pub fn save(store: &KeyStore, id: u32, path: &Path, overwrite: bool) -> Result<(), KeygenError> {
    if path.as_os_str().is_empty() {
        return Err(invalid("choose where to save the key"));
    }
    let pub_path = public_path(path);
    if !overwrite {
        for existing in [path, pub_path.as_path()] {
            if existing.exists() {
                return Err(KeygenError::Exists {
                    path: existing.display().to_string(),
                });
            }
        }
    }
    let (private, public) = store
        .with(id, |key| (key.private_openssh.clone(), key.public_key.clone()))
        .ok_or(KeygenError::UnknownKey)?;
    write_file(path, private.as_bytes(), 0o600, overwrite)?;
    write_file(&pub_path, format!("{public}\n").as_bytes(), 0o644, overwrite)
}

/// Saves the public key line of key `id` as `path` (0644), replacing it only if `overwrite`.
pub fn save_public(store: &KeyStore, id: u32, path: &Path, overwrite: bool) -> Result<(), KeygenError> {
    let public = store
        .with(id, |key| key.public_key.clone())
        .ok_or(KeygenError::UnknownKey)?;
    write_file(path, format!("{public}\n").as_bytes(), 0o644, overwrite)
}

/// Saves the OpenSSH private key of key `id` (encrypted when it has a passphrase) as
/// `path` (0600), replacing it only if `overwrite`.
pub fn save_private(store: &KeyStore, id: u32, path: &Path, overwrite: bool) -> Result<(), KeygenError> {
    let private = store
        .with(id, |key| key.private_openssh.clone())
        .ok_or(KeygenError::UnknownKey)?;
    write_file(path, private.as_bytes(), 0o600, overwrite)
}

/// Saves key `id` as a PuTTY `.ppk` version 3 file (encrypted with its passphrase when it
/// has one) as `path` (0600), replacing it only if `overwrite`.
pub fn export_ppk(store: &KeyStore, id: u32, path: &Path, overwrite: bool) -> Result<(), KeygenError> {
    let (key, passphrase) = store
        .with(id, |stored| (stored.key.clone(), stored.passphrase.clone()))
        .ok_or(KeygenError::UnknownKey)?;
    let contents = ppk::encode(&key, Some(&passphrase), &mut rng()).map_err(invalid)?;
    write_file(path, contents.as_bytes(), 0o600, overwrite)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored_private(store: &KeyStore, id: u32) -> String {
        store.with(id, |k| k.private_openssh.to_string()).unwrap()
    }

    #[test]
    fn generates_an_ed25519_key_whose_halves_parse_back() {
        let store = KeyStore::default();
        let key = generate(&store, KeyType::Ed25519, None, " me@laptop ").unwrap();
        assert!(key.public_key.starts_with("ssh-ed25519 AAAA"), "{}", key.public_key);
        assert!(key.public_key.ends_with(" me@laptop"), "{}", key.public_key);
        let public = russh::keys::PublicKey::from_openssh(&key.public_key).unwrap();
        assert_eq!(public.algorithm(), Algorithm::Ed25519);
        assert_eq!(key.fingerprint, public.fingerprint(HashAlg::Sha256).to_string());
        assert!(key.fingerprint.starts_with("SHA256:"));
        let private = PrivateKey::from_openssh(stored_private(&store, key.id)).unwrap();
        assert!(!private.is_encrypted());
        assert_eq!(private.public_key().key_data(), public.key_data());
        // Every generation is a new key.
        assert_ne!(
            generate(&store, KeyType::Ed25519, None, "").unwrap().public_key,
            key.public_key
        );
    }

    #[test]
    fn a_passphrase_encrypts_the_private_key() {
        let store = KeyStore::default();
        let key = generate(&store, KeyType::Ed25519, Some("correct horse"), "").unwrap();
        let private = PrivateKey::from_openssh(stored_private(&store, key.id)).unwrap();
        assert!(private.is_encrypted());
        assert!(private.decrypt("wrong").is_err());
        let decrypted = private.decrypt("correct horse").unwrap();
        assert_eq!(decrypted.public_key().to_openssh().unwrap(), key.public_key);
        // An empty passphrase means none.
        let plain = generate(&store, KeyType::Ed25519, Some(""), "").unwrap();
        assert!(
            !PrivateKey::from_openssh(stored_private(&store, plain.id))
                .unwrap()
                .is_encrypted()
        );
    }

    #[test]
    fn rejects_comments_that_do_not_fit_on_one_line() {
        let store = KeyStore::default();
        for bad in ["a\nb".to_string(), "x".repeat(MAX_COMMENT + 1)] {
            assert!(
                matches!(
                    generate(&store, KeyType::Ed25519, None, &bad),
                    Err(KeygenError::Invalid { .. })
                ),
                "{bad:?}"
            );
        }
    }

    #[cfg(unix)]
    fn mode(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn saves_the_private_key_0600_and_the_public_key_0644() {
        let store = KeyStore::default();
        let key = generate(&store, KeyType::Ed25519, None, "c").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("id_test");
        save(&store, key.id, &path, false).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), stored_private(&store, key.id));
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .starts_with("-----BEGIN OPENSSH PRIVATE KEY-----")
        );
        assert_eq!(mode(&path), 0o600);
        let pub_path = dir.path().join("id_test.pub");
        assert_eq!(
            std::fs::read_to_string(&pub_path).unwrap(),
            format!("{}\n", key.public_key)
        );
        assert_eq!(mode(&pub_path), 0o644);
    }

    #[cfg(unix)]
    #[test]
    fn never_overwrites_without_being_asked_to() {
        let store = KeyStore::default();
        let key = generate(&store, KeyType::Ed25519, None, "").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("id_test");
        let pub_path = public_path(&path);
        // Only the .pub exists: nothing is written at all.
        std::fs::write(&pub_path, "old pub").unwrap();
        assert_eq!(
            save(&store, key.id, &path, false),
            Err(KeygenError::Exists {
                path: pub_path.display().to_string()
            })
        );
        assert!(!path.exists());
        assert_eq!(std::fs::read_to_string(&pub_path).unwrap(), "old pub");
        // A world-readable file being replaced ends up 0600.
        std::fs::write(&path, "old key").unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        assert_eq!(
            save(&store, key.id, &path, false),
            Err(KeygenError::Exists {
                path: path.display().to_string()
            })
        );
        save(&store, key.id, &path, true).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), stored_private(&store, key.id));
        assert_eq!(mode(&path), 0o600);
        assert_eq!(
            std::fs::read_to_string(&pub_path).unwrap(),
            format!("{}\n", key.public_key)
        );
    }

    #[test]
    fn unknown_and_discarded_keys_cannot_be_saved() {
        let store = KeyStore::default();
        let key = generate(&store, KeyType::Ed25519, None, "").unwrap();
        store.discard(key.id);
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            save(&store, key.id, &dir.path().join("k"), false),
            Err(KeygenError::UnknownKey)
        );
        assert!(!dir.path().join("k").exists());
    }

    const ALL_TYPES: [(KeyType, &str, u32, &str); 7] = [
        (KeyType::Ed25519, "ED25519", 256, "ssh-ed25519"),
        (KeyType::Rsa2048, "RSA 2048", 2048, "ssh-rsa"),
        (KeyType::Rsa3072, "RSA 3072", 3072, "ssh-rsa"),
        (KeyType::Rsa4096, "RSA 4096", 4096, "ssh-rsa"),
        (KeyType::EcdsaP256, "ECDSA P-256", 256, "ecdsa-sha2-nistp256"),
        (KeyType::EcdsaP384, "ECDSA P-384", 384, "ecdsa-sha2-nistp384"),
        (KeyType::EcdsaP521, "ECDSA P-521", 521, "ecdsa-sha2-nistp521"),
    ];

    /// `ssh-keygen` with `args`, or `None` when it is not installed.
    fn ssh_keygen(args: &[&str]) -> Option<std::process::Output> {
        std::process::Command::new("ssh-keygen").args(args).output().ok()
    }

    fn testdata(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/keygen/testdata")
            .join(name)
    }

    /// The `type base64` part of a public key line.
    fn key_part(line: &str) -> String {
        line.split(' ').take(2).collect::<Vec<_>>().join(" ")
    }

    /// Every type and size makes a key `ssh-keygen` reads back (private key with its
    /// passphrase, and the type and bits of the public key).
    #[cfg(unix)]
    #[test]
    fn every_type_and_size_generates_a_key_ssh_keygen_accepts() {
        if ssh_keygen(&["-?"]).is_none() {
            eprintln!("ssh-keygen not installed, skipped");
            return;
        }
        let store = KeyStore::default();
        let dir = tempfile::tempdir().unwrap();
        for (kind, label, bits, prefix) in ALL_TYPES {
            let key = generate(&store, kind, Some("pass word"), "me@host").unwrap();
            assert_eq!(key.key_type, label);
            assert_eq!(key.comment, "me@host");
            assert!(key.encrypted);
            assert!(
                key.public_key.starts_with(&format!("{prefix} AAAA")),
                "{}",
                key.public_key
            );
            let private = dir.path().join(format!("id_{bits}_{prefix}"));
            let public = public_path(&private);
            save_private(&store, key.id, &private, false).unwrap();
            save_public(&store, key.id, &public, false).unwrap();
            let derived = ssh_keygen(&["-y", "-P", "pass word", "-f", private.to_str().unwrap()]).unwrap();
            assert!(derived.status.success(), "{label}: {derived:?}");
            assert_eq!(
                key_part(String::from_utf8_lossy(&derived.stdout).trim()),
                key_part(&key.public_key),
                "{label}"
            );
            let listed = ssh_keygen(&["-l", "-f", public.to_str().unwrap()]).unwrap();
            let listed = String::from_utf8_lossy(&listed.stdout).trim().to_string();
            let ssh_type = label.split(' ').next().unwrap();
            assert_eq!(
                listed,
                format!("{bits} {} me@host ({ssh_type})", key.fingerprint),
                "{label}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn loads_openssh_keys_asking_for_the_passphrase_of_encrypted_ones() {
        let store = KeyStore::default();
        let dir = tempfile::tempdir().unwrap();
        let plain = generate(&store, KeyType::Ed25519, None, "plain").unwrap();
        let plain_path = dir.path().join("plain");
        save_private(&store, plain.id, &plain_path, false).unwrap();
        let loaded = load(&store, &plain_path, None).unwrap();
        assert_ne!(loaded.id, plain.id);
        assert_eq!(
            (loaded.public_key.as_str(), loaded.fingerprint.as_str()),
            (plain.public_key.as_str(), plain.fingerprint.as_str())
        );
        assert_eq!(loaded.comment, "plain");
        assert!(!loaded.encrypted);
        // A typed passphrase does not encrypt an unencrypted file.
        assert!(!load(&store, &plain_path, Some("typed")).unwrap().encrypted);

        let secret = generate(&store, KeyType::EcdsaP256, Some("right"), "secret").unwrap();
        let secret_path = dir.path().join("secret");
        save_private(&store, secret.id, &secret_path, false).unwrap();
        assert_eq!(load(&store, &secret_path, None), Err(KeygenError::PassphraseRequired));
        assert_eq!(
            load(&store, &secret_path, Some("")),
            Err(KeygenError::PassphraseRequired)
        );
        assert_eq!(
            load(&store, &secret_path, Some("wrong")),
            Err(KeygenError::WrongPassphrase)
        );
        let loaded = load(&store, &secret_path, Some("right")).unwrap();
        assert_eq!(loaded.public_key, secret.public_key);
        assert_eq!(loaded.key_type, "ECDSA P-256");
        assert!(loaded.encrypted);
        // Saved again, it keeps its passphrase.
        let again = dir.path().join("again");
        save_private(&store, loaded.id, &again, false).unwrap();
        assert_eq!(load(&store, &again, Some("wrong")), Err(KeygenError::WrongPassphrase));
        assert_eq!(
            load(&store, &again, Some("right")).unwrap().public_key,
            secret.public_key
        );
    }

    #[cfg(unix)]
    #[test]
    fn loads_keys_made_by_ssh_keygen() {
        if ssh_keygen(&["-?"]).is_none() {
            eprintln!("ssh-keygen not installed, skipped");
            return;
        }
        let store = KeyStore::default();
        let dir = tempfile::tempdir().unwrap();
        for (args, label) in [
            (["-t", "rsa", "-b", "2048"], "RSA 2048"),
            (["-t", "ecdsa", "-b", "384"], "ECDSA P-384"),
            (["-t", "ed25519", "-a", "16"], "ED25519"),
        ] {
            let path = dir.path().join(label.replace(' ', "_"));
            let made = ssh_keygen(
                &[
                    &args[..],
                    &[
                        "-q",
                        "-N",
                        "pw",
                        "-C",
                        "made by ssh-keygen",
                        "-f",
                        path.to_str().unwrap(),
                    ],
                ]
                .concat(),
            )
            .unwrap();
            assert!(made.status.success(), "{made:?}");
            let public = std::fs::read_to_string(public_path(&path)).unwrap();
            assert_eq!(
                load(&store, &path, Some("bad")),
                Err(KeygenError::WrongPassphrase),
                "{label}"
            );
            let loaded = load(&store, &path, Some("pw")).unwrap();
            assert_eq!(loaded.public_key, public.trim(), "{label}");
            assert_eq!(loaded.key_type, label);
            assert_eq!(loaded.comment, "made by ssh-keygen");
        }
    }

    #[test]
    fn rejects_files_that_are_not_private_keys() {
        let store = KeyStore::default();
        let dir = tempfile::tempdir().unwrap();
        let key = generate(&store, KeyType::Ed25519, None, "").unwrap();
        let public = dir.path().join("k.pub");
        save_public(&store, key.id, &public, false).unwrap();
        let binary = dir.path().join("bin");
        std::fs::write(&binary, [0xff, 0xfe, 0x00]).unwrap();
        let huge = dir.path().join("huge");
        std::fs::write(&huge, "a".repeat(MAX_KEY_FILE as usize + 1)).unwrap();
        for path in [&public, &binary, &huge] {
            assert!(
                matches!(load(&store, path, None), Err(KeygenError::Invalid { .. })),
                "{path:?}"
            );
        }
        assert!(matches!(
            load(&store, &dir.path().join("missing"), None),
            Err(KeygenError::Io { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn changes_the_comment_and_passphrase_of_a_loaded_key() {
        let store = KeyStore::default();
        let dir = tempfile::tempdir().unwrap();
        let original = generate(&store, KeyType::Rsa2048, Some("old pw"), "old").unwrap();
        let path = dir.path().join("id_rsa");
        save_private(&store, original.id, &path, false).unwrap();
        let loaded = load(&store, &path, Some("old pw")).unwrap();

        let updated = update(&store, loaded.id, " new comment ", Some("new pw")).unwrap();
        assert_eq!(updated.id, loaded.id);
        assert_eq!(updated.comment, "new comment");
        assert!(updated.encrypted);
        assert_eq!(key_part(&updated.public_key), key_part(&original.public_key));
        assert!(updated.public_key.ends_with(" new comment"));
        assert_eq!(updated.fingerprint, original.fingerprint);
        save_private(&store, loaded.id, &path, true).unwrap();
        assert_eq!(load(&store, &path, Some("old pw")), Err(KeygenError::WrongPassphrase));
        let reloaded = load(&store, &path, Some("new pw")).unwrap();
        assert_eq!(reloaded.comment, "new comment");
        if let Some(derived) = ssh_keygen(&["-y", "-P", "new pw", "-f", path.to_str().unwrap()]) {
            assert!(derived.status.success(), "{derived:?}");
        }

        // An empty passphrase removes it.
        let cleared = update(&store, reloaded.id, "new comment", Some("")).unwrap();
        assert!(!cleared.encrypted);
        save_private(&store, reloaded.id, &path, true).unwrap();
        assert!(!load(&store, &path, None).unwrap().encrypted);

        assert!(matches!(
            update(&store, reloaded.id, "a\nb", None),
            Err(KeygenError::Invalid { .. })
        ));
        store.discard(reloaded.id);
        assert_eq!(update(&store, reloaded.id, "x", None), Err(KeygenError::UnknownKey));
    }

    #[cfg(unix)]
    #[test]
    fn saves_the_public_and_the_private_key_separately() {
        let store = KeyStore::default();
        let dir = tempfile::tempdir().unwrap();
        let key = generate(&store, KeyType::EcdsaP256, None, "c").unwrap();
        let public = dir.path().join("only.pub");
        save_public(&store, key.id, &public, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(&public).unwrap(),
            format!("{}\n", key.public_key)
        );
        assert_eq!(mode(&public), 0o644);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);

        let private = dir.path().join("only");
        save_private(&store, key.id, &private, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(&private).unwrap(),
            stored_private(&store, key.id)
        );
        assert_eq!(mode(&private), 0o600);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);

        // Never replaced unless asked; a replaced world-readable private file ends up 0600.
        for (path, save) in [
            (
                &public,
                save_public as fn(&KeyStore, u32, &Path, bool) -> Result<(), KeygenError>,
            ),
            (&private, save_private),
        ] {
            std::fs::write(path, "old").unwrap();
            assert_eq!(
                save(&store, key.id, path, false),
                Err(KeygenError::Exists {
                    path: path.display().to_string()
                })
            );
            assert_eq!(std::fs::read_to_string(path).unwrap(), "old");
        }
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        save_private(&store, key.id, &private, true).unwrap();
        save_public(&store, key.id, &public, true).unwrap();
        assert_eq!(mode(&private), 0o600);
        assert_eq!(
            std::fs::read_to_string(&public).unwrap(),
            format!("{}\n", key.public_key)
        );
        store.discard(key.id);
        assert_eq!(
            save_public(&store, key.id, &dir.path().join("x"), false),
            Err(KeygenError::UnknownKey)
        );
        assert_eq!(
            save_private(&store, key.id, &dir.path().join("y"), false),
            Err(KeygenError::UnknownKey)
        );
        assert_eq!(
            export_ppk(&store, key.id, &dir.path().join("z"), false),
            Err(KeygenError::UnknownKey)
        );
    }

    #[cfg(unix)]
    #[test]
    fn exports_putty_keys_that_load_back_with_their_passphrase() {
        let store = KeyStore::default();
        let dir = tempfile::tempdir().unwrap();
        for (kind, passphrase) in [
            (KeyType::Ed25519, Some("pw")),
            (KeyType::Rsa2048, None),
            (KeyType::EcdsaP521, Some("pw")),
        ] {
            let key = generate(&store, kind, passphrase, "putty me").unwrap();
            let path = dir.path().join(format!("{kind:?}.ppk"));
            export_ppk(&store, key.id, &path, false).unwrap();
            assert_eq!(mode(&path), 0o600);
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(text.starts_with("PuTTY-User-Key-File-3: "), "{text}");
            assert_eq!(text.contains("Encryption: aes256-cbc\n"), passphrase.is_some());
            if passphrase.is_some() {
                assert_eq!(load(&store, &path, None), Err(KeygenError::PassphraseRequired));
                assert_eq!(load(&store, &path, Some("nope")), Err(KeygenError::WrongPassphrase));
            }
            let loaded = load(&store, &path, passphrase).unwrap();
            assert_eq!(loaded.public_key, key.public_key);
            assert_eq!(loaded.comment, "putty me");
            assert_eq!(loaded.encrypted, passphrase.is_some());
            assert_eq!(
                export_ppk(&store, key.id, &path, false),
                Err(KeygenError::Exists {
                    path: path.display().to_string()
                })
            );
            // Loaded from the .ppk, it saves as an OpenSSH key ssh-keygen reads.
            let openssh = dir.path().join(format!("{kind:?}"));
            save_private(&store, loaded.id, &openssh, false).unwrap();
            if let Some(derived) = ssh_keygen(&["-y", "-P", passphrase.unwrap_or(""), "-f", openssh.to_str().unwrap()])
            {
                assert_eq!(
                    key_part(String::from_utf8_lossy(&derived.stdout).trim()),
                    key_part(&key.public_key)
                );
            }
        }
    }

    /// PuTTYgen files, version 2 and 3, encrypted ("123") and not.
    #[test]
    fn loads_puttygen_files_of_both_versions() {
        let store = KeyStore::default();
        for (name, passphrase, openssh) in [
            ("id_ed25519.ppk", None, "id_ed25519"),
            ("id_ed25519_enc.ppk", Some("123"), "id_ed25519"),
            ("id_ecdsa_p256.ppk", None, "id_ecdsa_p256"),
            ("id_ecdsa_p256_enc.ppk", Some("123"), "id_ecdsa_p256"),
            ("id_rsa_3072.ppk", None, "id_rsa_3072"),
            ("id_rsa_3072_enc.ppk", Some("123"), "id_rsa_3072"),
            ("id_rsa_3072.ppk2", None, "id_rsa_3072"),
            ("id_rsa_3072_enc.ppk2", Some("123"), "id_rsa_3072"),
        ] {
            let path = testdata(name);
            if passphrase.is_some() {
                assert_eq!(
                    load(&store, &path, None),
                    Err(KeygenError::PassphraseRequired),
                    "{name}"
                );
                assert_eq!(
                    load(&store, &path, Some("1234")),
                    Err(KeygenError::WrongPassphrase),
                    "{name}"
                );
            }
            let loaded = load(&store, &path, passphrase).unwrap();
            let expected = load(&store, &testdata(openssh), None).unwrap();
            assert_eq!(loaded.public_key, expected.public_key, "{name}");
            assert_eq!(loaded.comment, "user@example.com");
            assert_eq!(loaded.encrypted, passphrase.is_some(), "{name}");
        }
    }
}
