use nostr_sdk::prelude::*;
use nostr_sdk::Client;
use std::time::SystemTime;

use tracing::{debug, info};

/// Seconds since 1970.
#[must_use]
pub fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|x| x.as_secs())
        .unwrap_or(0)
}

// Creates the websocket client that is used for communicating with relays
// Copyright (c) 2022 0xtr MIT License
pub async fn create_client(keys: &Keys, relays: Vec<String>) -> Result<Client> {
    let opts = Options::new().wait_for_send(true);
    let client = Client::new_with_opts(keys, opts);
    let relays = relays.iter().map(|url| (url, None)).collect();
    client.add_relays(relays).await?;
    client.connect().await;
    Ok(client)
}

// Parses a private key string and returns a keypair if valid.
// If the private_key is None, a new keypair will be generated
// Copyright (c) 2022 0xtr MIT License
pub fn handle_keys(private_key: Option<String>) -> Result<Keys> {
    // Parse and validate private key
    let keys = match private_key {
        Some(pk) => {
            // create a new identity using the provided private key
            Keys::from_sk_str(pk.as_str())?
        }
        None => {
            // create a new identity with a new keypair
            info!("No private key provided, creating new identity");
            Keys::generate()
        }
    };

    debug!("Private key: {}", keys.secret_key()?.to_bech32()?);
    debug!("Public key: {}", keys.public_key().to_bech32()?);
    Ok(keys)
}
