//! Saved SSH passwords and AI keys, kept only in the system keyring (Secret Service on Linux,
//! Credential Manager on Windows), ported from PlexusTerm v0.2.15 `src-tauri/src/secrets.rs`
//! (PRD DATA-3).
//!
//! Secrets are addressed by the saved session id (or `ai:<provider>`) under the service
//! [`SERVICE`], so items saved by v0.2.15 are read as they are.

use std::collections::HashMap;
use std::sync::Arc;

/// Keyring service under which every PlexusTerm secret is stored.
pub const SERVICE: &str = "dev.plexusterm.app";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretError {
    Unavailable,
    Keyring { message: String },
}

impl std::fmt::Display for SecretError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => f.write_str("the system keyring is not available"),
            Self::Keyring { message } => write!(f, "system keyring error: {message}"),
        }
    }
}

impl std::error::Error for SecretError {}

/// Where saved passwords live (the port). Implementations may block (D-Bus round trips), so
/// async code goes through [`run`].
pub trait SecretStore: Send + Sync {
    /// Whether the store can be used at all (e.g. a Secret Service is running).
    fn available(&self) -> bool;
    fn get(&self, id: &str) -> Result<Option<String>, SecretError>;
    /// Creates or replaces the secret of `id`; `label` is what keyring UIs show.
    fn set(&self, id: &str, label: &str, secret: &str) -> Result<(), SecretError>;
    /// Deletes the secret of `id`; deleting a missing secret succeeds.
    fn delete(&self, id: &str) -> Result<(), SecretError>;
}

pub type SharedSecrets = Arc<dyn SecretStore>;

/// Runs a store operation on the blocking pool so the async runtime never waits on D-Bus.
pub async fn run<T: Send + 'static>(
    store: &SharedSecrets,
    op: impl FnOnce(&dyn SecretStore) -> Result<T, SecretError> + Send + 'static,
) -> Result<T, SecretError> {
    let store = store.clone();
    tokio::task::spawn_blocking(move || op(&*store))
        .await
        .map_err(|err| SecretError::Keyring {
            message: err.to_string(),
        })?
}

/// A store that is never available (the default until a real one is configured).
pub struct NoStore;

impl SecretStore for NoStore {
    fn available(&self) -> bool {
        false
    }
    fn get(&self, _id: &str) -> Result<Option<String>, SecretError> {
        Ok(None)
    }
    fn set(&self, _id: &str, _label: &str, _secret: &str) -> Result<(), SecretError> {
        Err(SecretError::Unavailable)
    }
    fn delete(&self, _id: &str) -> Result<(), SecretError> {
        Ok(())
    }
}

/// The user's keyring through the platform store (the adapter).
pub struct KeyringStore {
    service: String,
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::with_service(SERVICE)
    }
}

/// Maps keyring errors to messages that never carry secret bytes.
pub fn keyring_error(err: keyring_core::Error) -> SecretError {
    let message = match err {
        keyring_core::Error::BadEncoding(_) | keyring_core::Error::BadDataFormat(..) => {
            "the saved secret is malformed".to_owned()
        }
        keyring_core::Error::Ambiguous(items) => format!("{} keyring items match this session", items.len()),
        keyring_core::Error::NoDefaultStore => return SecretError::Unavailable,
        other => other.to_string(),
    };
    SecretError::Keyring { message }
}

impl KeyringStore {
    pub fn with_service(service: &str) -> Self {
        Self {
            service: service.to_owned(),
        }
    }

    fn entry(&self, id: &str, label: Option<&str>) -> Result<keyring_core::Entry, SecretError> {
        // Installs the platform's store as keyring-core's default store, once.
        if keyring::Entry::store_status().is_err() {
            return Err(SecretError::Unavailable);
        }
        let modifiers = label.and_then(|label| entry_modifiers(std::env::consts::OS, label));
        let entry = match modifiers {
            Some(modifiers) => keyring_core::Entry::new_with_modifiers(&self.service, id, &modifiers),
            None => keyring_core::Entry::new(&self.service, id),
        };
        entry.map_err(keyring_error)
    }
}

/// The modifiers that give a new keyring item `label` on platform `os`. Only the Secret
/// Service store (Unix except Apple and Android) knows `label`; the Windows and Apple stores
/// reject unknown modifiers, so their items go without one.
pub fn entry_modifiers<'a>(os: &str, label: &'a str) -> Option<HashMap<&'static str, &'a str>> {
    match os {
        "windows" | "macos" | "ios" | "android" => None,
        _ => Some(HashMap::from([("label", label)])),
    }
}

impl SecretStore for KeyringStore {
    fn available(&self) -> bool {
        keyring::Entry::store_status().is_ok()
    }

    fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
        match self.entry(id, None)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(err) => Err(keyring_error(err)),
        }
    }

    fn set(&self, id: &str, label: &str, secret: &str) -> Result<(), SecretError> {
        // Updated in place: deleting first could lose the old secret if the write then failed.
        self.entry(id, Some(label))?.set_password(secret).map_err(keyring_error)
    }

    fn delete(&self, id: &str) -> Result<(), SecretError> {
        match self.entry(id, None)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(err) => Err(keyring_error(err)),
        }
    }
}

/// `user@host:port` a saved password belongs to, normalized the same way when it is saved and
/// when it is used.
pub fn endpoint(username: &str, host: &str, port: u16) -> String {
    format!("{}@{}:{port}", username.trim(), host.trim())
}

const SAVED_PASSWORD_VERSION: u32 = 1;

/// Keyring value of a saved password: the password bound to the endpoint it was typed for, so it
/// is never sent anywhere else (e.g. after the session was edited). Deliberately not `Debug`.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SavedPassword {
    v: u32,
    endpoint: String,
    password: String,
}

impl SavedPassword {
    /// The keyring value, `{"v":1,"endpoint":"…","password":"…"}` as v0.2.15 writes it.
    pub fn encode(endpoint: String, password: String) -> String {
        let saved = Self {
            v: SAVED_PASSWORD_VERSION,
            endpoint,
            password,
        };
        // A struct of strings always serializes; an empty value is simply never matched.
        serde_json::to_string(&saved).unwrap_or_default()
    }

    /// The password when `value` was saved for `endpoint`. Values saved for another endpoint,
    /// in another format or as plain text (older builds) yield `None`.
    pub fn password_for(value: &str, endpoint: &str) -> Option<String> {
        let saved: Self = serde_json::from_str(value).ok()?;
        (saved.v == SAVED_PASSWORD_VERSION && saved.endpoint == endpoint).then_some(saved.password)
    }
}

#[cfg(feature = "test-support")]
mod test_support {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, PoisonError, mpsc};

    use super::{SecretError, SecretStore};

    /// In-memory store for tests; it also records labels.
    #[derive(Default)]
    pub struct MemoryStore {
        entries: Mutex<HashMap<String, (String, String)>>,
    }

    impl MemoryStore {
        pub fn shared() -> Arc<Self> {
            Arc::new(Self::default())
        }

        pub fn label(&self, id: &str) -> Option<String> {
            self.entries
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(id)
                .map(|(label, _)| label.clone())
        }
    }

    impl SecretStore for MemoryStore {
        fn available(&self) -> bool {
            true
        }
        fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
            Ok(self
                .entries
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(id)
                .map(|(_, secret)| secret.clone()))
        }
        fn set(&self, id: &str, label: &str, secret: &str) -> Result<(), SecretError> {
            self.entries
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(id.into(), (label.into(), secret.into()));
            Ok(())
        }
        fn delete(&self, id: &str) -> Result<(), SecretError> {
            self.entries.lock().unwrap_or_else(PoisonError::into_inner).remove(id);
            Ok(())
        }
    }

    /// A keyring that never answers writes until the returned sender is dropped, like a locked
    /// Secret Service waiting on its unlock dialog.
    pub struct BlockingStore {
        release: Mutex<mpsc::Receiver<()>>,
    }

    impl BlockingStore {
        pub fn new() -> (Arc<Self>, mpsc::Sender<()>) {
            let (tx, rx) = mpsc::channel();
            (
                Arc::new(Self {
                    release: Mutex::new(rx),
                }),
                tx,
            )
        }

        fn wait(&self) -> Result<(), SecretError> {
            // Returns once the sender is dropped (or sends).
            let _ = self.release.lock().unwrap_or_else(PoisonError::into_inner).recv();
            Err(SecretError::Keyring {
                message: "released".into(),
            })
        }
    }

    impl SecretStore for BlockingStore {
        fn available(&self) -> bool {
            true
        }
        fn get(&self, _id: &str) -> Result<Option<String>, SecretError> {
            Ok(None)
        }
        fn set(&self, _id: &str, _label: &str, _secret: &str) -> Result<(), SecretError> {
            self.wait()
        }
        fn delete(&self, _id: &str) -> Result<(), SecretError> {
            self.wait()
        }
    }
}

#[cfg(feature = "test-support")]
pub use test_support::{BlockingStore, MemoryStore};
