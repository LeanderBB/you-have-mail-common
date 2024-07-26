//! State management of accounts in the database.

use crate::db::{Pool, Transaction};
use crate::encryption::Key;
use http::Proxy;
use rusqlite::OptionalExtension;
use rusqlite_from_row::FromRow;
use secrecy::{ExposeSecret, Secret};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Serialization: {0}")]
    Serialization(#[from] serde_json::error::Error),
    #[error("Decryption: {0}")]
    Decryption(anyhow::Error),
    #[error("Encryption: {0}")]
    Encryption(anyhow::Error),
    #[error("Db: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Other: {0}")]
    Other(anyhow::Error),
}

/// Represents a stored account.
///
/// To add a new account to the system, you must provide an implementation of [`IntoAccount`] when
/// interacting with [`crate::Yhm`].
///
/// Accounts can have a regular non-encrypted state, which can be set using [`set_state`] and a
/// secret encrypted state with [`set_secret`].
///
/// Since the authentication tokens are stored in the secret state, an account is considered logged
/// in if there is a secret value. If no such value is present, it is treated as logged out.
///
#[derive(FromRow)]
pub struct Account {
    email: String,
    backend: String,
    secret: Option<Vec<u8>>,
    state: Option<Vec<u8>>,
    proxy: Option<Vec<u8>>,
}

impl Account {
    pub fn new(email: String, backend: String) -> Self {
        Self {
            email,
            backend,
            secret: None,
            state: None,
            proxy: None,
        }
    }

    /// Get the account's email.
    pub fn email(&self) -> &str {
        &self.email
    }

    /// Get the account's backend name.
    pub fn backend(&self) -> &str {
        &self.backend
    }

    /// Get the account state.
    ///
    /// # Errors
    ///
    /// Return error if the state construction failed.
    pub fn state<T: DeserializeOwned>(&self) -> Result<Option<T>, Error> {
        let Some(state) = &self.state else {
            return Ok(None);
        };

        Ok(Some(serde_json::from_slice::<T>(state)?))
    }

    /// Get the secret state.
    ///
    /// # Errors
    ///
    /// Return error if the state construction failed.
    pub fn secret<T: DeserializeOwned>(&self, key: &Key) -> Result<Option<T>, Error> {
        let Some(secret) = &self.secret else {
            return Ok(None);
        };

        Ok(Some(secret_from_bytes::<T>(key, secret)?))
    }

    /// Get the proxy configuration.
    ///
    /// # Errors
    ///
    /// Return error if the state construction failed.
    pub fn proxy(&self, key: &Key) -> Result<Option<Proxy>, Error> {
        let Some(proxy) = &self.proxy else {
            return Ok(None);
        };

        Ok(Some(secret_from_bytes(key, proxy)?))
    }

    /// Update the account with new `state`.
    ///
    /// This state is not encrypted. To store state encrypted see [`set_secret`].
    ///
    /// If `state` is `None`, existing state will be erased.
    ///
    /// # Errors
    ///
    /// Return error if the state construction failed.
    pub fn set_state<T: Serialize>(&mut self, state: Option<&T>) -> Result<(), Error> {
        let Some(state) = state else {
            self.state = None;
            return Ok(());
        };
        self.state = Some(serde_json::to_vec(state)?);
        Ok(())
    }

    /// Update the account with new `secret` state.
    ///
    /// Secret state is always stored encrypted. For non encrypted state see [`set_state`].
    ///
    /// If `secret` is `None`, existing secret will be erased.
    ///
    /// # Errors
    ///
    /// Return error if the state construction failed.
    pub fn set_secret<T: Serialize>(&mut self, key: &Key, secret: Option<&T>) -> Result<(), Error> {
        self.secret = match secret {
            None => None,
            Some(secret) => Some(secret_to_bytes(key, secret)?),
        };
        Ok(())
    }

    /// Update the account with new `proxy` config.
    ///
    /// # Errors
    ///
    /// Return error if the state construction failed.
    pub fn set_proxy(&mut self, key: &Key, proxy: Option<&Proxy>) -> Result<(), Error> {
        match proxy {
            Some(p) => {
                self.proxy = Some(secret_to_bytes(key, p)?);
            }
            None => {
                self.proxy = None;
            }
        }
        Ok(())
    }

    /// Check whether the account is logged in.
    ///
    /// An account is considered logged in if there is some value in the secret state.
    pub fn is_logged_out(&self) -> bool {
        self.secret.is_none()
    }
}

/// Conversion trait for new accounts.
pub trait IntoAccount {
    fn into_account(self, encryption_key: &Key) -> Result<Account, Error>;
}

impl IntoAccount for Account {
    fn into_account(self, _: &Key) -> Result<Account, Error> {
        Ok(self)
    }
}

/// Contains all state serialized in the database.
pub struct State {
    pool: Arc<Pool>,
    encryption_key: Secret<Key>,
}

impl State {
    /// Create a new state with database at `db_path` and with the given `encryption_key`.
    ///
    /// # Errors
    ///
    /// Returns errors if we failed to create the tables.
    pub fn new(db_path: PathBuf, encryption_key: Secret<Key>) -> Result<Arc<Self>, Error> {
        let pool = Pool::new(db_path);
        let mut conn = pool.connection()?;
        conn.with_transaction(create_tables)?;
        Ok(Arc::new(Self {
            pool,
            encryption_key,
        }))
    }

    /// Create a new state with database at `db_path` and with the given `encryption_key` without
    /// initializing the database tables.
    ///
    /// # Errors
    ///
    /// Returns errors if we failed to create the tables.
    pub fn without_init(db_path: PathBuf, encryption_key: Secret<Key>) -> Arc<Self> {
        let pool = Pool::new(db_path);
        Arc::new(Self {
            pool,
            encryption_key,
        })
    }

    /// Get the encryption key.
    pub fn encryption_key(&self) -> &Secret<Key> {
        &self.encryption_key
    }

    /// Get all accounts recorded in the database.
    ///
    /// # Errors
    ///
    /// Returns error if the query failed.
    pub fn accounts(&self) -> Result<Vec<Account>, Error> {
        self.pool.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM yhm ORDER BY email")?;
            let rows = stmt.query_map((), Account::try_from_row)?;
            let mut result = Vec::new();
            for row in rows {
                result.push(row?);
            }
            Ok(result)
        })
    }

    /// Get a single account by `email`.
    ///
    /// # Errors
    ///
    /// Returns error if the query failed.
    pub fn account(&self, email: &str) -> Result<Option<Account>, Error> {
        self.pool.with_connection(|conn| {
            Ok(conn
                .query_row(
                    "SELECT * FROM yhm WHERE email=? LIMIT 1",
                    [email],
                    Account::try_from_row,
                )
                .optional()?)
        })
    }

    /// Get the number of registered accounts.
    ///
    /// # Errors
    ///
    /// Returns error if the query failed.
    pub fn account_count(&self) -> Result<usize, Error> {
        self.pool.with_connection(|conn| {
            Ok(conn.query_row("SELECT count(*) FROM yhm", (), |r| r.get(0))?)
        })
    }

    /// Check if account with `email` exists.
    pub fn has_account(&self, email: &str) -> Result<bool, Error> {
        self.pool.with_connection(|conn| {
            Ok(conn
                .query_row("SELECT 1 FROM yhm WHERE email=? LIMIT 1", [email], |r| {
                    r.get::<usize, i32>(0)
                })
                .optional()?
                .is_some())
        })
    }

    /// Create or update `account`.
    ///
    /// # Errors
    ///
    /// Return error it the query failed.
    pub fn store_account(&self, account: &Account) -> Result<(), Error> {
        self.pool.with_transaction(|tx| {
            tx.execute(
                r"
INSERT INTO yhm VALUES (
    ?,?,?,?,?
) ON CONFLICT(email) DO UPDATE SET
    secret=excluded.secret,
    state=excluded.state,
    proxy=excluded.proxy
",
                (
                    &account.email,
                    &account.backend,
                    &account.secret,
                    &account.state,
                    &account.proxy,
                ),
            )?;
            Ok(())
        })
    }

    /// Delete account with `email`.
    ///
    /// # Errors
    ///
    /// Return error it the query failed.
    pub fn delete_account(&self, email: &str) -> Result<(), Error> {
        self.pool.with_transaction(|tx| {
            tx.execute("DELETE FROM yhm WHERE email=?", [email])?;
            Ok(())
        })
    }

    /// Update `proxy` config for account with `email`.
    ///
    /// # Errors
    ///
    /// Returns error if the operation failed.
    pub fn set_proxy(&self, email: &str, proxy: Option<&Proxy>) -> Result<(), Error> {
        let bytes = match proxy {
            None => None,
            Some(proxy) => Some(secret_to_bytes(self.encryption_key.expose_secret(), proxy)?),
        };

        self.pool.with_transaction(|tx| -> Result<(), Error> {
            tx.execute("UPDATE yhm SET proxy=? WHERE email=?", (bytes, email))?;
            Ok(())
        })
    }

    /// Update the `secret` state of account with `email`
    ///
    /// # Errors
    ///
    /// Return error it the query failed or the state failed to serialize.
    pub fn update_secret_state<T: Serialize>(&self, email: &str, secret: &T) -> Result<(), Error> {
        let bytes = secret_to_bytes(self.encryption_key.expose_secret(), secret)?;
        self.pool.with_transaction(|tx| {
            tx.execute("UPDATE yhm SET secret=? WHERE email=?", (bytes, email))?;
            Ok(())
        })
    }

    /// Remove the secret state of the account with `email`.
    ///
    /// # Errors
    ///
    /// Return error it the query failed.
    pub fn delete_secret_state(&self, email: &str) -> Result<(), Error> {
        self.pool.with_transaction(|tx| {
            tx.execute("UPDATE yhm SET secret=NULL WHERE email=?", [email])?;
            Ok(())
        })
    }

    /// Update the `state` of account with `email`
    ///
    /// # Errors
    ///
    /// Return error it the query failed or the state failed to serialize.
    pub fn update_account_state<T: Serialize>(&self, email: &str, state: &T) -> Result<(), Error> {
        let bytes = serde_json::to_vec(state)?;
        self.pool.with_transaction(|tx| {
            tx.execute("UPDATE yhm SET state=? WHERE email=?", (bytes, email))?;
            Ok(())
        })
    }

    /// Update the `proxy` config of the account with `email`.
    ///
    /// # Errors
    ///
    /// Return error it the query failed.
    pub fn update_proxy(&self, key: &Key, email: &str, proxy: Option<&Proxy>) -> Result<(), Error> {
        let bytes = match proxy {
            None => None,
            Some(proxy) => Some(secret_to_bytes(key, proxy)?),
        };

        self.pool.with_transaction(|tx| {
            tx.execute("UPDATE yhm SET proxy=? WHERE email=?", (bytes, email))?;
            Ok(())
        })
    }

    /// Delete account with `email`.
    ///
    /// # Errors
    ///
    /// Returns error if the operation failed.
    pub fn delete(&self, email: &str) -> Result<(), Error> {
        self.pool.with_transaction(|tx| {
            tx.execute("DELETE FROM yhm WHERE email=?", [email])?;
            Ok(())
        })
    }

    /// Get poll interval setting.
    ///
    /// # Errors
    ///
    /// Return error if the operation failed.
    pub fn poll_interval(&self) -> Result<Duration, Error> {
        self.pool.with_connection(|conn| {
            let interval: u64 = conn.query_row(
                "SELECT poll_interval FROM yhm_settings WHERE id=? LIMIT 1",
                [SETTINGS_ID],
                |r| r.get(0),
            )?;
            Ok(Duration::from_secs(interval))
        })
    }

    /// Set the poll interval setting.
    ///
    /// # Errors
    ///
    /// Return error if the operation failed.
    pub fn set_poll_interval(&self, duration: Duration) -> Result<(), Error> {
        self.pool.with_transaction(|tx| {
            tx.execute(
                "UPDATE yhm_settings SET poll_interval=? WHERE id=?",
                (duration.as_secs(), SETTINGS_ID),
            )?;
            Ok(())
        })
    }
}

fn create_tables(tx: &mut Transaction) -> rusqlite::Result<()> {
    tx.execute(
        r"
CREATE TABLE IF NOT EXISTS yhm (
    email TEXT PRIMARY KEY,
    backend TEXT NOT NULL,
    secret BLOB DEFAULT NULL,
    state BLOD DEFAULT NULL,
    proxy BLOB DEFAULT NULL
)
",
        (),
    )?;

    tx.execute(
        r"
CREATE TABLE IF NOT EXISTS yhm_settings (
    id PRIMARY KEY,
    poll_interval INTEGER NOT NULL DEFAULT 300
)
",
        (),
    )?;

    tx.execute(
        "INSERT OR IGNORE INTO yhm_settings VALUES (?,?)",
        (SETTINGS_ID, DEFAULT_POLL_INTERVAL_SECONDS),
    )?;

    Ok(())
}

/// Decrypted and deserialize secret.
///
/// # Errors
///
/// Returns error if the decryption or deserialization failed.
fn secret_from_bytes<T: DeserializeOwned>(key: &Key, bytes: &[u8]) -> Result<T, Error> {
    let decrypted = Secret::new(key.decrypt(bytes).map_err(Error::Decryption)?);
    Ok(serde_json::from_slice::<T>(decrypted.expose_secret())?)
}

/// Serialize and encrypt secret.
///
/// # Errors
///
/// Returns error if the encryption or serialization failed.
fn secret_to_bytes<T: Serialize>(key: &Key, value: &T) -> Result<Vec<u8>, Error> {
    let serialized = Secret::new(serde_json::to_vec(value)?);
    let encrypted = key
        .encrypt(serialized.expose_secret())
        .map_err(Error::Encryption)?;
    Ok(encrypted)
}

const SETTINGS_ID: i64 = 1;
const DEFAULT_POLL_INTERVAL_SECONDS: i64 = 300;
