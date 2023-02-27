use std::collections::{HashMap, HashSet};

use redb::{
    Database, MultimapTableDefinition, ReadableMultimapTable, ReadableTable, TableDefinition,
    WriteStrategy,
};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::Error;
// key is hex pubkey value is name
const ACCOUNTTABLE: TableDefinition<&str, u8> = TableDefinition::new("account");
const EVENTTABLE: MultimapTableDefinition<&str, u64> = MultimapTableDefinition::new("event");
// Key pubkey value is pubkey of who they follow
const FOLLOWSTABLE: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new("follows");
// Key is pubkey value is pubkey of who follows that pubkey
const FOLLOWERSTABLE: MultimapTableDefinition<&str, &str> =
    MultimapTableDefinition::new("followers");

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub enum Tier {
    Primary = 0,
    Secondary = 1,
    Tertiary = 2,
    Quaternary = 3,
    Other = 4,
}

impl From<u8> for Tier {
    fn from(value: u8) -> Self {
        match value {
            0 => Tier::Primary,
            1 => Tier::Secondary,
            2 => Tier::Tertiary,
            3 => Tier::Quaternary,
            _ => Tier::Other,
        }
    }
}

impl Tier {
    // Its like golf
    // lowest tier has most permission
    /*
    fn lower_tier(&self) -> Tier {
        match self {
            Tier::Primary => Tier::Primary, // Can't move up from the first variant
            Tier::Secondary => Tier::Primary,
            Tier::Tertiary => Tier::Secondary,
            Tier::Quaternary => Tier::Tertiary,
            Tier::Other => Tier::Quaternary,
        }
    }
    */

    fn raise_tier(&self) -> Tier {
        match self {
            Tier::Primary => Tier::Secondary,
            Tier::Secondary => Tier::Tertiary,
            Tier::Tertiary => Tier::Quaternary,
            Tier::Quaternary => Tier::Other,
            Tier::Other => Tier::Other,
        }
    }
}

pub struct Account {
    pub pubkey: String,
    pub tier: Tier,
}

pub struct Db {
    db: Database,
}

impl Default for Db {
    fn default() -> Self {
        Self::new()
    }
}

impl Db {
    pub fn new() -> Self {
        debug!("Creating DB");
        let db = Database::create("my_db.redb").unwrap();
        db.set_write_strategy(WriteStrategy::TwoPhase).unwrap();
        let write_txn = db.begin_write().unwrap();
        {
            // Opens the table to create it
            let _ = write_txn.open_table(ACCOUNTTABLE).unwrap();
            let _ = write_txn.open_multimap_table(EVENTTABLE).unwrap();
            let _ = write_txn.open_multimap_table(FOLLOWSTABLE).unwrap();
            let _ = write_txn.open_multimap_table(FOLLOWERSTABLE).unwrap();
        }
        write_txn.commit().unwrap();

        Self { db }
    }

    pub fn write_account(&self, account: &Account) -> Result<(), Error> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(ACCOUNTTABLE)?;
            table.insert(account.pubkey.as_str(), account.tier as u8)?;
        }
        write_txn.commit().unwrap();
        Ok(())
    }

    pub fn read_account(&self, pubkey: &str) -> Result<Option<Account>, Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ACCOUNTTABLE)?;
        if let Some(account_info) = table.get(pubkey)? {
            let account = Account {
                pubkey: pubkey.to_string(),
                tier: Tier::from(account_info.value()),
            };
            return Ok(Some(account));
        }
        Ok(None)
    }

    pub fn read_all_accounts(&self) -> Result<(), Error> {
        debug!("Registered accounts");
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ACCOUNTTABLE)?;

        for a in table.iter()? {
            debug!("{:?}, {}", a.0.value(), a.1.value());
        }
        Ok(())
    }

    pub fn write_event(&self, pubkey: &str, timestamp: u64) -> Result<(), Error> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_multimap_table(EVENTTABLE)?;
            table.insert(pubkey, timestamp)?;
        }
        write_txn.commit().unwrap();
        Ok(())
    }

    pub fn get_events(&self, pubkey: &str) -> Result<Vec<u64>, Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_multimap_table(EVENTTABLE)?;

        let result = table.get(pubkey)?;

        Ok(result.map(|e| e.value()).collect())
    }

    pub fn set_tier(&self, keys: &HashSet<String>, tier: Tier) -> Result<(), Error> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table = write_txn.open_table(ACCOUNTTABLE)?;
            for k in keys {
                table.insert(k.as_str(), tier as u8)?;
            }
        }
        write_txn.commit().unwrap();

        Ok(())
    }

    pub fn clear_tables(&self) -> Result<(), Error> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table = write_txn.open_table(ACCOUNTTABLE)?;
            while table.len()? > 0 {
                let _ = table.pop_first();
            }
            let mut table = write_txn.open_multimap_table(FOLLOWSTABLE)?;
            let keys: HashSet<String> = table.iter()?.map(|(x, _)| x.value().to_string()).collect();

            for k in keys {
                table.remove_all(k.as_str())?;
            }
            let mut table = write_txn.open_multimap_table(FOLLOWERSTABLE)?;
            let keys: HashSet<String> = table.iter()?.map(|(x, _)| x.value().to_string()).collect();

            for k in keys {
                table.remove_all(k.as_str())?;
            }
        }
        write_txn.commit().unwrap();

        Ok(())
    }

    fn add_follows(&self, contacts: &HashMap<String, HashSet<String>>) -> Result<(), Error> {
        let write_txn = self.db.begin_write()?;

        {
            let mut follows_table = write_txn.open_multimap_table(FOLLOWSTABLE)?;
            for (key, contact_list) in contacts {
                for f in contact_list {
                    debug!("Set follow {key}, {f}");
                    follows_table.insert(key.as_str(), f.as_str())?;
                }
            }
        }
        write_txn.commit().unwrap();

        Ok(())
    }

    fn add_followers(&self, contacts: &HashMap<String, HashSet<String>>) -> Result<(), Error> {
        let write_txn = self.db.begin_write()?;

        {
            let mut followers_table = write_txn.open_multimap_table(FOLLOWERSTABLE)?;
            for (key, contact_list) in contacts {
                for f in contact_list {
                    debug!("Set follower {f}, {key}");
                    followers_table.insert(f.as_str(), key.as_str())?;
                }
            }
        }
        write_txn.commit().unwrap();

        Ok(())
    }

    fn remove_follows(&self, pubkey: &str, follows: &HashSet<String>) -> Result<(), Error> {
        let write_txn = self.db.begin_write()?;
        {
            let mut follows_table = write_txn.open_multimap_table(FOLLOWSTABLE)?;
            for follow in follows {
                debug!("remove follows {pubkey}, {follow}");
                follows_table.remove(pubkey, follow.as_str())?;
            }
        }
        write_txn.commit().unwrap();

        Ok(())
    }

    fn remove_followers(&self, pubkey: &str, followers: &HashSet<String>) -> Result<(), Error> {
        let write_txn = self.db.begin_write()?;
        {
            let mut followers_table = write_txn.open_multimap_table(FOLLOWERSTABLE)?;
            for follower in followers {
                debug!("remove followers {follower}, {pubkey}");
                followers_table.remove(follower.as_str(), pubkey)?;
            }
        }
        write_txn.commit().unwrap();

        Ok(())
    }

    pub fn set_contact_list(
        &self,
        contacts: &HashMap<String, HashSet<String>>,
    ) -> Result<(), Error> {
        self.add_followers(contacts)?;
        self.add_follows(contacts)?;

        Ok(())
    }

    fn get_follows(&self, pubkey: &str) -> Result<HashSet<String>, Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_multimap_table(FOLLOWSTABLE)?;

        let result = table.get(pubkey)?;
        Ok(result.map(|e| e.value().to_string()).collect())
    }

    fn get_followers(&self, pubkey: &str) -> Result<HashMap<String, Tier>, Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_multimap_table(FOLLOWERSTABLE)?;

        // let result: HashSet<&str> = table.get(pubkey)?.map(|p| p.value().clone()).collect();
        let result: HashSet<String> = table.get(pubkey)?.map(|p| p.value().to_owned()).collect();
        let account_table = read_txn.open_table(ACCOUNTTABLE)?;

        // This is unreadble
        let followers = result
            .iter()
            .map(|p| (p, account_table.get(&**p)))
            .filter_map(|(p, a)| a.ok().filter(|a| a.is_some()).map(|a| (p, a.unwrap())))
            .map(|(p, t)| (p.to_string(), Tier::from(t.value())))
            .collect();

        Ok(followers)
    }

    fn update_account(&self, pubkey: &str, min_tier: Tier) -> Result<(), Error> {
        debug!("Update account: {pubkey}");
        let mut tier = min_tier;
        debug!("{tier:?}");
        // Get account followers
        let followers = self.get_followers(pubkey)?;
        debug!("Followers: {:?}", followers);

        // Min tier of follower
        let min_value = followers.iter().min_by_key(|&(_, v)| v).map(|(_, v)| *v);

        debug!("{min_tier:?}");
        if let Some(min_tier) = min_value {
            let t = min_tier.raise_tier();
            if t > tier {
                tier = t;
            }
        }
        let account = Account {
            pubkey: pubkey.to_string(),
            tier,
        };
        self.write_account(&account)
    }

    pub fn update_contact_list(
        &mut self,
        contacts: &HashMap<String, HashSet<String>>,
    ) -> Result<(), Error> {
        for l in contacts {
            if let Some(account) = self.read_account(l.0)? {
                // Get current list of follows
                let current_follows = self.get_follows(l.0)?;
                debug!("current follows: {:?}", current_follows);
                debug!("new contact list {:?}", l.1);

                let new_follows: HashSet<String> =
                    l.1.difference(&current_follows).cloned().collect();
                debug!("{} followed: {:?}", account.pubkey, new_follows);

                let unfollowed: HashSet<String> =
                    current_follows.difference(l.1).cloned().collect();
                debug!("{} unfollowed {unfollowed:?}", account.pubkey);

                self.remove_follows(l.0, &unfollowed)?;
                self.remove_followers(l.0, &unfollowed)?;

                let new_follow_tier = account.tier.raise_tier();
                self.update_follows(new_follows, new_follow_tier)?;

                let unfollowed_tier = Tier::Other;
                self.update_follows(unfollowed, unfollowed_tier)?;
            }
        }
        self.set_contact_list(contacts)?;
        Ok(())
    }

    /// For the each follow in Set passed get their follows
    /// Updated follow and each of their follow
    fn update_follows(&self, follows: HashSet<String>, min_tier: Tier) -> Result<(), Error> {
        for f in follows {
            let follows_followers = self.get_follows(&f)?;
            self.update_account(&f, min_tier)?;

            for f_f in follows_followers {
                let f_f_tier = min_tier.raise_tier();
                self.update_account(&f_f, f_f_tier)?;
            }
        }
        Ok(())
    }
}
