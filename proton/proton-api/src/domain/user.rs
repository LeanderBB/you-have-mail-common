use serde::{Deserialize, Deserializer};
use serde_repr::Deserialize_repr;
use std::fmt::{Display, Formatter};

/// Represents an API User ID.
#[derive(Debug, Deserialize, Eq, PartialEq, Hash, Clone)]
pub struct Id(pub(crate) String);

impl AsRef<str> for Id {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Represent an user's API key ID.
#[derive(Debug, Deserialize, Eq, PartialEq, Hash, Clone)]
pub struct KeyId(pub(crate) String);

impl Display for KeyId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Represents an API user
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct User {
    #[serde(rename = "ID")]
    pub id: Id,
    pub name: String,
    pub display_name: String,
    pub email: String,
    pub used_space: i64,
    pub max_space: i64,
    pub max_upload: i64,
    pub credit: i64,
    pub currency: String,
    pub keys: Vec<Key>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Key {
    #[serde(rename = "ID")]
    pub id: KeyId,
    pub private_key: String,
    pub token: Option<String>,
    pub signature: Option<String>,
    #[serde(deserialize_with = "bool_from_integer")]
    pub primary: bool,
    #[serde(deserialize_with = "bool_from_integer")]
    pub active: bool,
    pub flags: Option<KeyState>,
}

#[derive(Deserialize_repr, Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum KeyState {
    None = 0,
    Trusted = 1,
    Active = 2,
}

/// Deserialize bool from integer
fn bool_from_integer<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    if i64::deserialize(deserializer)? == 0i64 {
        Ok(false)
    } else {
        Ok(true)
    }
}
