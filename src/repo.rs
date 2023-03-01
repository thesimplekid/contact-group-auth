use crate::config::Limitation;
use crate::db::Db;
use crate::db::{Account, Tier};
use crate::error::Error;
use crate::utils::unix_time;

use std::collections::HashSet;
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
        pubkey: &str,
        contacts: &HashSet<String>,
    ) -> Result<(), Error> {
        self.db.lock().unwrap().set_contact_list(pubkey, contacts)
    }

    pub fn add_account(&self, account: &Account) -> Result<(), Error> {
        self.db.lock().unwrap().write_account(account)
    }

    pub fn get_account(&self, pubkey: &str) -> Result<Option<Account>, Error> {
        self.db.lock().unwrap().read_account(pubkey)
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
        if limits.events_per_day.is_some() || limits.events_per_day.is_some() {
            let events = self.db.lock().unwrap().get_events(pubkey)?;
            if let Some(max_per_day) = limits.events_per_day {
                let past_day = count_events_in_range(&events, 86400);
                info!("Events past day: {past_day} for {pubkey}");
                if past_day > max_per_day {
                    return Ok((false, Some("24 hours limit exhausted".to_string())));
                }
            }

            if let Some(max_per_hour) = limits.events_per_hour {
                let past_hour = count_events_in_range(&events, 3600);
                info!("Events past hour: {past_hour} for {pubkey}");
                if past_hour > max_per_hour {
                    return Ok((false, Some("Hour limit exhausted".to_string())));
                }
            }
        }

        Ok((true, None))
    }

    pub async fn update_contacts(
        &self,
        pubkey: &str,
        contacts: HashSet<String>,
    ) -> Result<(), Error> {
        self.db
            .lock()
            .unwrap()
            .update_contact_list(pubkey, &contacts)
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

#[cfg(test)]
mod tests {

    use serial_test::serial;

    use super::*;

    #[test]
    #[serial]
    fn test_set_get_account() {
        let _primary_acounts = HashSet::from([
            "7995c67e4b40fcc88f7603fcedb5f2133a74b89b2678a332b21faee725f039f9".to_string(),
        ]);
        let repo = Repo::new(HashSet::new());
        let pubkey = "7995c67e4b40fcc88f7603fcedb5f2133a74b89b2678a332b21faee725f039f9";
        let account = Account {
            pubkey: pubkey.to_string(),
            tier: Tier::Primary,
        };
        repo.add_account(&account).unwrap();
        let read_account = repo.get_account(pubkey).unwrap().unwrap();

        assert_eq!(account, read_account);
    }

    #[test]
    #[serial]
    fn test_get_account_tier() {
        let repo = Repo::new(HashSet::new());
        let pubkey = "7995c67e4b40fcc88f7603fcedb5f2133a74b89b2678a332b21faee725f039f9";
        let account = Account {
            pubkey: pubkey.to_string(),
            tier: Tier::Primary,
        };
        repo.add_account(&account).unwrap();
        let account_tier = repo.get_account_tier(pubkey).unwrap();

        assert_eq!(Tier::Primary, account_tier);
    }
}
