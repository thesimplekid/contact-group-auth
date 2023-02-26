use thiserror::Error;

use nostr_sdk::nostr::secp256k1::Error as Secp256k1Error;
use nostr_sdk::prelude::id::Error as IdError;
use nostr_sdk::prelude::tag::Error as TagError;

#[derive(Error, Debug)]
pub enum Error {
    #[error("DB Error")]
    DBError(redb::Error),
    #[error("Not in db")]
    NotFound,
    #[error("Serde error")]
    SerdeError(serde_json::Error),
    #[error("Nostr error")]
    NostrError(nostr_sdk::client::Error),
    #[error("Join error")]
    JoinError(tokio::task::JoinError),
    #[error("Tag error: {0}")]
    TagError(#[from] TagError),
    #[error("Secp256k1 error: {0}")]
    Secp256k1Error(#[from] Secp256k1Error),
    #[error("ID error: {0}")]
    IdError(#[from] IdError),
}

impl From<redb::Error> for Error {
    fn from(err: redb::Error) -> Self {
        Self::DBError(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::SerdeError(err)
    }
}

impl From<nostr_sdk::client::Error> for Error {
    fn from(err: nostr_sdk::client::Error) -> Self {
        Self::NostrError(err)
    }
}

impl From<tokio::task::JoinError> for Error {
    fn from(err: tokio::task::JoinError) -> Self {
        Self::JoinError(err)
    }
}
