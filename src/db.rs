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

#[derive(Debug, PartialEq, Eq)]
pub struct Account {
    pub pubkey: String,
    pub tier: Tier,
}

pub struct Db {
    db: Database,
    primary: HashSet<String>,
}

impl Default for Db {
    fn default() -> Self {
        Self::new(HashSet::new())
    }
}

impl Db {
    pub fn new(primary: HashSet<String>) -> Self {
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

        Self { db, primary }
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
            let mut table = write_txn.open_multimap_table(EVENTTABLE)?;
            let keys: HashSet<String> = table.iter()?.map(|(x, _)| x.value().to_string()).collect();

            for k in keys {
                table.remove_all(k.as_str())?;
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

    fn add_follows(&self, pubkey: &str, contacts: &HashSet<String>) -> Result<(), Error> {
        let write_txn = self.db.begin_write()?;

        {
            let mut follows_table = write_txn.open_multimap_table(FOLLOWSTABLE)?;
            for f in contacts {
                debug!("Set follow {pubkey}, {f}");
                follows_table.insert(pubkey, f.as_str())?;
            }
        }
        write_txn.commit().unwrap();

        Ok(())
    }

    fn add_followers(&self, pubkey: &str, contacts: &HashSet<String>) -> Result<(), Error> {
        let write_txn = self.db.begin_write()?;

        {
            let mut followers_table = write_txn.open_multimap_table(FOLLOWERSTABLE)?;
            for f in contacts {
                debug!("Set follower {f}, {pubkey}");
                followers_table.insert(f.as_str(), pubkey)?;
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

    pub fn set_contact_list(&self, pubkey: &str, contacts: &HashSet<String>) -> Result<(), Error> {
        println!("Setting contacts");
        self.add_followers(pubkey, contacts)?;
        self.add_follows(pubkey, contacts)?;

        Ok(())
    }

    fn get_follows(&self, pubkey: &str) -> Result<HashSet<String>, Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_multimap_table(FOLLOWSTABLE)?;

        let result = table.get(pubkey)?;
        Ok(result.map(|e| e.value().to_string()).collect())
    }

    fn get_account_tiers(&self, accounts: HashSet<String>) -> Result<HashMap<String, Tier>, Error> {
        let mut accounts_with_tiers = HashMap::new();

        for account in accounts {
            let tier;
            if let Some(t) = self.read_account(account.as_str())? {
                tier = t.tier;
            } else {
                tier = Tier::Other;
            }

            accounts_with_tiers.insert(account, tier);
        }
        Ok(accounts_with_tiers)
    }

    fn get_followers(&self, pubkey: &str) -> Result<HashSet<String>, Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_multimap_table(FOLLOWERSTABLE)?;

        let followers: HashSet<String> = table.get(pubkey)?.map(|p| p.value().to_owned()).collect();

        debug!("{} followed by {:?}", pubkey, followers);

        Ok(followers)
    }

    fn update_account(&self, pubkey: &str, min_tier: Tier) -> Result<(), Error> {
        debug!("Update account: {pubkey}");
        let mut tier = min_tier;
        debug!("{tier:?}");

        // Get account followers
        let followers = self.get_followers(pubkey)?;
        debug!("Followers: {:?}", followers);
        let followers = self.get_account_tiers(followers)?;

        // Min tier of follower
        if self.primary.contains(pubkey) {
            tier = Tier::Primary;
        } else {
            // Minium tier based on followers
            let min_tier = followers.iter().min_by_key(|&(_, v)| v).map(|(_, v)| *v);
            debug!("Follower min tier: {min_tier:?}");
            if let Some(min_tier) = min_tier {
                let t = min_tier.raise_tier();
                if t < tier {
                    tier = t;
                }
            }
        }

        debug!("New tier: {tier:?}");

        let account = Account {
            pubkey: pubkey.to_string(),
            tier,
        };
        self.write_account(&account)
    }

    pub fn update_contact_list(
        &self,
        pubkey: &str,
        new_contacts: &HashSet<String>,
    ) -> Result<(), Error> {
        if let Some(account) = self.read_account(pubkey)? {
            // Get current list of follows
            let current_follows = self.get_follows(pubkey)?;
            self.set_contact_list(pubkey, new_contacts)?;
            debug!("current follows: {:?}", current_follows);
            debug!("new contact list {:?}", new_contacts);

            let new_follows: HashSet<String> =
                new_contacts.difference(&current_follows).cloned().collect();
            debug!("{} followed: {:?}", account.pubkey, new_follows);

            let unfollowed: HashSet<String> =
                current_follows.difference(new_contacts).cloned().collect();
            debug!("{} unfollowed {unfollowed:?}", account.pubkey);

            self.remove_follows(pubkey, &unfollowed)?;
            self.remove_followers(pubkey, &unfollowed)?;

            let new_follow_tier = account.tier.raise_tier();
            self.update_follows(new_follows, new_follow_tier)?;

            let unfollowed_tier = Tier::Other;
            self.update_follows(unfollowed, unfollowed_tier)?;
        }
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

#[cfg(test)]
mod tests {
    use crate::utils::unix_time;
    use serial_test::serial;

    // use tracing::{debug, error, info};
    // use tracing_test::traced_test;

    use super::*;

    #[test]
    #[serial]
    fn get_events() {
        let db = Db::new(HashSet::new());
        db.clear_tables().unwrap();
        let pubkey = "7995c67e4b40fcc88f7603fcedb5f2133a74b89b2678a332b21faee725f039f9";

        let timestamp = unix_time();
        db.write_event(pubkey, timestamp).unwrap();

        let events = db.get_events(pubkey).unwrap();

        assert_eq!(vec![timestamp], events);
    }

    #[test]
    #[serial]
    fn test_set_contacts() {
        let db = Db::new(HashSet::new());
        debug!("setting contacts");
        db.clear_tables().unwrap();
        let pubkey = "7995c67e4b40fcc88f7603fcedb5f2133a74b89b2678a332b21faee725f039f9".to_string();

        let follow_one =
            "d81eb632d2385c3e6bdc8da5a32b57275348819aebd39ff74613793f29694203".to_string();
        let follow_two =
            "7c27a04b7c27299f16dc07d3eb8f28544f188bc7a34982328b7d581edc405dc2".to_string();

        let follows = HashSet::from([follow_one.clone(), follow_two.clone()]);

        let primary_account = Account {
            pubkey: pubkey.clone(),
            tier: Tier::Primary,
        };
        db.write_account(&primary_account).unwrap();
        db.set_contact_list(&pubkey, &follows).unwrap();
        db.update_follows(follows.clone(), Tier::Secondary).unwrap();

        let db_follows = db.get_follows(&pubkey).unwrap();

        assert_eq!(follows, db_follows);

        let db_followers = db.get_followers(&follow_one).unwrap();
        assert_eq!(HashSet::from([pubkey.clone()]), db_followers);
        let db_followers = db.get_followers(&follow_two).unwrap();
        assert_eq!(HashSet::from([pubkey.clone()]), db_followers);

        assert_eq!(primary_account, db.read_account(&pubkey).unwrap().unwrap());
        let one_account = Account {
            pubkey: follow_one.clone(),
            tier: Tier::Secondary,
        };
        assert_eq!(one_account, db.read_account(&follow_one).unwrap().unwrap());
        let two_account = Account {
            pubkey: follow_two.clone(),
            tier: Tier::Secondary,
        };
        assert_eq!(
            two_account,
            db.read_account(&follow_two.clone()).unwrap().unwrap()
        );
    }

    // Test that a primary user can unfollow a user
    // #[traced_test]
    #[test]
    #[serial]
    fn test_primary_unfollow() {
        let db = Db::new(HashSet::new());
        db.clear_tables().unwrap();
        let pubkey = "7995c67e4b40fcc88f7603fcedb5f2133a74b89b2678a332b21faee725f039f9".to_string();

        let follow_one =
            "d81eb632d2385c3e6bdc8da5a32b57275348819aebd39ff74613793f29694203".to_string();
        let follow_two =
            "7c27a04b7c27299f16dc07d3eb8f28544f188bc7a34982328b7d581edc405dc2".to_string();

        let follows = HashSet::from([follow_one.clone(), follow_two.clone()]);

        let primary_account = Account {
            pubkey: pubkey.clone(),
            tier: Tier::Primary,
        };
        db.write_account(&primary_account).unwrap();
        db.set_contact_list(&pubkey, &follows.clone()).unwrap();
        db.update_follows(follows, Tier::Secondary).unwrap();

        let new_contacts = HashSet::from([follow_one.clone()]);

        db.update_contact_list(&pubkey, &new_contacts).unwrap();
        let db_follows = db.get_follows(&pubkey).unwrap();

        assert_eq!(new_contacts, db_follows);

        assert_eq!(primary_account, db.read_account(&pubkey).unwrap().unwrap());

        let one_account = Account {
            pubkey: follow_one.clone(),
            tier: Tier::Secondary,
        };
        assert_eq!(one_account, db.read_account(&follow_one).unwrap().unwrap());

        let two_account = Account {
            pubkey: follow_two.clone(),
            tier: Tier::Other,
        };
        assert_eq!(
            two_account,
            db.read_account(&follow_two.clone()).unwrap().unwrap()
        );
    }

    #[test]
    #[serial]
    // #[traced_test]
    fn primary_unfollow_with_tier() {
        let pubkey = "7995c67e4b40fcc88f7603fcedb5f2133a74b89b2678a332b21faee725f039f9".to_string();
        let db = Db::new(HashSet::from([pubkey.clone()]));
        db.clear_tables().unwrap();

        let follow_one =
            "d81eb632d2385c3e6bdc8da5a32b57275348819aebd39ff74613793f29694203".to_string();
        let follow_two =
            "7c27a04b7c27299f16dc07d3eb8f28544f188bc7a34982328b7d581edc405dc2".to_string();

        let follows = HashSet::from([follow_one.clone(), follow_two.clone()]);

        let primary_account = Account {
            pubkey: pubkey.clone(),
            tier: Tier::Primary,
        };
        db.write_account(&primary_account).unwrap();
        db.set_contact_list(&pubkey, &follows.clone()).unwrap();
        db.update_follows(follows, Tier::Secondary).unwrap();

        let t_follow =
            "5b3a49bcbdf41f511f5c9034dbe46240863b73b39a2fdfebfb611f23b88d3922".to_string();

        let t_follows = HashSet::from([t_follow.clone()]);
        db.set_contact_list(&follow_one, &t_follows).unwrap();
        db.update_follows(t_follows, Tier::Tertiary).unwrap();

        let t_account = Account {
            pubkey: t_follow.clone(),
            tier: Tier::Tertiary,
        };
        assert_eq!(
            t_account,
            db.read_account(&t_follow.clone()).unwrap().unwrap()
        );

        let new_contacts = HashSet::from([follow_two.clone()]);

        db.update_contact_list(&pubkey, &new_contacts).unwrap();
        let db_follows = db.get_follows(&pubkey).unwrap();

        assert_eq!(new_contacts, db_follows);

        assert_eq!(primary_account, db.read_account(&pubkey).unwrap().unwrap());

        let one_account = Account {
            pubkey: follow_one.clone(),
            tier: Tier::Other,
        };
        assert_eq!(one_account, db.read_account(&follow_one).unwrap().unwrap());

        let two_account = Account {
            pubkey: follow_two.clone(),
            tier: Tier::Secondary,
        };
        assert_eq!(
            two_account,
            db.read_account(&follow_two.clone()).unwrap().unwrap()
        );

        let t_account = Account {
            pubkey: t_follow.clone(),
            tier: Tier::Other,
        };
        assert_eq!(
            t_account,
            db.read_account(&t_follow.clone()).unwrap().unwrap()
        );
        // Unfollow sec should also set t to other
    }

    #[test]
    #[serial]
    // #[traced_test]
    fn primary_unfollow_with_tier_refollow() {
        let pubkey = "7995c67e4b40fcc88f7603fcedb5f2133a74b89b2678a332b21faee725f039f9".to_string();
        let db = Db::new(HashSet::from([pubkey.clone()]));
        db.clear_tables().unwrap();

        let follow_one =
            "d81eb632d2385c3e6bdc8da5a32b57275348819aebd39ff74613793f29694203".to_string();
        let follow_two =
            "7c27a04b7c27299f16dc07d3eb8f28544f188bc7a34982328b7d581edc405dc2".to_string();

        let follows = HashSet::from([follow_one.clone(), follow_two.clone()]);

        let primary_account = Account {
            pubkey: pubkey.clone(),
            tier: Tier::Primary,
        };
        db.write_account(&primary_account).unwrap();
        db.set_contact_list(&pubkey, &follows.clone()).unwrap();
        db.update_follows(follows.clone(), Tier::Secondary).unwrap();

        let t_follow =
            "5b3a49bcbdf41f511f5c9034dbe46240863b73b39a2fdfebfb611f23b88d3922".to_string();

        let t_follows = HashSet::from([t_follow.clone()]);
        db.set_contact_list(&follow_one, &t_follows).unwrap();
        db.update_follows(t_follows, Tier::Tertiary).unwrap();

        let t_account = Account {
            pubkey: t_follow.clone(),
            tier: Tier::Tertiary,
        };
        assert_eq!(
            t_account,
            db.read_account(&t_follow.clone()).unwrap().unwrap()
        );

        let new_contacts = HashSet::from([follow_two.clone()]);

        // Unfollow secondary
        db.update_contact_list(&pubkey, &new_contacts).unwrap();
        let db_follows = db.get_follows(&pubkey).unwrap();

        assert_eq!(new_contacts, db_follows);

        // Refollow secondary
        db.update_contact_list(&pubkey, &follows).unwrap();

        assert_eq!(primary_account, db.read_account(&pubkey).unwrap().unwrap());

        let one_account = Account {
            pubkey: follow_one.clone(),
            tier: Tier::Secondary,
        };
        assert_eq!(one_account, db.read_account(&follow_one).unwrap().unwrap());

        let two_account = Account {
            pubkey: follow_two.clone(),
            tier: Tier::Secondary,
        };
        assert_eq!(
            two_account,
            db.read_account(&follow_two.clone()).unwrap().unwrap()
        );

        let t_account = Account {
            pubkey: t_follow.clone(),
            tier: Tier::Tertiary,
        };
        assert_eq!(
            t_account,
            db.read_account(&t_follow.clone()).unwrap().unwrap()
        );
        // Unfollow sec should also set t to other
    }
}
