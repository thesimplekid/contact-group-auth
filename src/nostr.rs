use nostr::event::tag::Tag;
use nostr_sdk::prelude::schnorr::Signature;
use nostr_sdk::prelude::*;

use crate::nauthz_grpc::event::TagEntry;

use crate::error::Error;
use crate::utils::{create_client, handle_keys};

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use crate::nauthz_grpc;

#[derive(Clone)]
pub struct Nostr {
    client: Client,
}

impl Nostr {
    pub async fn new(relay_url: &str, key: &Option<String>) -> Result<Self, Error> {
        let key = key.to_owned();
        let keys = handle_keys(key).unwrap();

        let client = create_client(&keys, vec![relay_url.to_string()])
            .await
            .unwrap();

        Ok(Self { client })
    }

    /// Accepts a list of keys
    /// Returns lists of all keys followed by at least one of past list key
    pub async fn get_contact_lists(
        &self,
        keys: &HashSet<String>,
    ) -> Result<HashMap<String, HashSet<String>>, Error> {
        let authors: Vec<XOnlyPublicKey> = keys
            .iter()
            .flat_map(|a| XOnlyPublicKey::from_str(a.as_str()))
            .collect();

        let events: Vec<Event> = self
            .client
            .get_events_of(
                vec![SubscriptionFilter {
                    ids: None,
                    authors: Some(authors),
                    kinds: Some(vec![Kind::ContactList]),
                    events: None,
                    pubkeys: None,
                    hashtags: None,
                    references: None,
                    search: None,
                    since: None,
                    until: None,
                    limit: None,
                }],
                None,
            )
            .await?;

        Ok(events
            .iter()
            .map(|e| {
                let follows = follows_from_event(e);
                (e.pubkey.to_string(), follows)
            })
            .collect())

        // Ok(follows_from_events(events))

        // Create query for the contact list of each key in list
    }
}

pub fn follows_from_event(event: &Event) -> HashSet<String> {
    event
        .tags
        .iter()
        .map(|tag| tag.as_vec()[1].clone())
        .collect()
}

impl From<nauthz_grpc::Event> for Event {
    fn from(event: nauthz_grpc::Event) -> Event {
        let id = EventId::from_slice(&event.id).unwrap();
        let pubkey = XOnlyPublicKey::from_slice(&event.pubkey).unwrap();
        let sig = Signature::from_slice(&event.sig).unwrap();
        let tags = event
            .tags
            .iter()
            .map(|t| <TagEntry as Into<Tag>>::into(t.clone()))
            .collect();

        Event {
            id,
            pubkey,
            created_at: event.created_at.into(),
            kind: Kind::from(event.kind),
            content: event.content,
            sig,
            ots: None,
            tags,
        }
    }
}

impl From<TagEntry> for Tag {
    fn from(tag: TagEntry) -> Tag {
        Tag::parse(tag.values).unwrap()
    }
}
