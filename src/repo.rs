use crate::config::Limitation;
use crate::db::Db;
use crate::db::{Account, Tier};
use crate::error::Error;
use crate::utils::unix_time;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use nostr_sdk::prelude::*;
use tracing::info;

#[derive(Clone)]
pub struct Repo {
    db: Arc<Mutex<Db>>,
}

impl Default for Repo {
    fn default() -> Self {
        Self::new(HashSet::new())
    }
}

impl Repo {
    pub fn new(primary: HashSet<String>) -> Self {
        Repo {
            db: Arc::new(Mutex::new(Db::new(primary))),
        }
    }

    pub async fn set_tier(&self, keys: &HashSet<String>, tier: Tier) -> Result<(), Error> {
        self.db.lock().unwrap().set_tier(keys, tier)
    }

    pub async fn set_contact_list(
        &self,
        contacts: &HashMap<String, HashSet<String>>,
    ) -> Result<(), Error> {
        self.db.lock().unwrap().set_contact_list(contacts)
    }

    pub fn get_account(&self, pubkey: &str) -> Result<Option<Account>, Error> {
        self.db.lock().unwrap().read_account(pubkey)
    }

    pub fn add_account(&self, account: Account) -> Result<(), Error> {
        self.db.lock().unwrap().write_account(&account)
    }

    pub fn get_account_tier(&self, pubkey: &str) -> Result<Tier, Error> {
        if let Some(account) = self.get_account(pubkey)? {
            Ok(account.tier)
        } else {
            Ok(Tier::Other)
        }
    }

    pub fn get_all_accounts(&self) -> Result<(), Error> {
        self.db.lock().unwrap().read_all_accounts()
    }

    pub fn add_event(&self, author: &str) -> Result<(), Error> {
        self.db.lock().unwrap().write_event(author, unix_time())
    }

    pub async fn check_rate_limits(
        &self,
        limits: &Limitation,
        pubkey: &str,
    ) -> Result<(bool, Option<String>), Error> {
        let events = self.db.lock().unwrap().get_events(pubkey)?;
        let past_day = count_events_in_range(&events, 86400);
        let past_hour = count_events_in_range(&events, 3600);

        info!("Events past hour: {past_hour} for {pubkey}");
        info!("Events past day: {past_day} for {pubkey}");

        if let Some(max_per_day) = limits.events_per_day {
            if past_day > max_per_day {
                return Ok((false, Some("24 hours limit exhausted".to_string())));
            }
        }

        if let Some(max_per_hour) = limits.events_per_hour {
            if past_hour > max_per_hour {
                return Ok((false, Some("Hour limit exhausted".to_string())));
            }
        }

        Ok((true, None))
    }

    pub async fn update_contacts(
        &self,
        contacts: HashMap<String, HashSet<String>>,
    ) -> Result<(), Error> {
        self.db.lock().unwrap().update_contact_list(&contacts)
    }

    /// Clears account tables
    pub async fn clear_accounts(&self) -> Result<(), Error> {
        self.db.lock().unwrap().clear_tables()
    }
}

fn count_events_in_range(events: &[u64], range: u64) -> usize {
    let since_time = unix_time() - range;
    events.iter().filter(|&t| *t > since_time).count()
}
