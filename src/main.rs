use db::Tier;
use nostr_sdk::prelude::hex::ToHex;
use tonic::{transport::Server, Request, Response, Status};

use nauthz_grpc::authorization_server::{Authorization, AuthorizationServer};
use nauthz_grpc::{Decision, EventReply, EventRequest};

use crate::config::{Limitation, Settings};
use crate::error::Error;
use crate::nostr::Nostr;
use crate::repo::Repo;

use crate::nostr::follows_from_event;

use tracing::{debug, info};

pub mod nauthz_grpc {
    tonic::include_proto!("nauthz");
}

pub mod config;
pub mod db;
pub mod error;
pub mod nostr;
pub mod repo;
pub mod utils;

pub struct EventAuthz {
    pub repo: Repo,
    pub settings: Settings,
    pub nos: Nostr,
}

#[tonic::async_trait]
impl Authorization for EventAuthz {
    async fn event_admit(
        &self,
        request: Request<EventRequest>,
    ) -> Result<Response<EventReply>, Status> {
        let reply;
        let req = request.into_inner();
        let event = req.clone().event.unwrap();
        let content_prefix: String = event.content.chars().take(40).collect();
        info!("recvd event, [kind={}, origin={:?}, nip05_domain={:?}, tag_count={}, content_sample={:?}]",
                 event.kind, req.origin, req.nip05.as_ref().map(|x| x.domain.clone()), event.tags.len(), content_prefix);

        let author = match req.auth_pubkey {
            Some(_) => req.auth_pubkey(),
            None => &event.pubkey,
        };

        let author = author.to_hex();

        let tier = self.repo.get_account_tier(&author).unwrap();

        // Check that tier against limits
        let limitation = get_limitation(&self.settings, &tier).await;

        if limitation.can_publish {
            match self.repo.check_rate_limits(&limitation, &author).await {
                Ok((true, msg)) => {
                    // Record event in db
                    self.repo.add_event(&author).unwrap();

                    if event.kind.eq(&3) {
                        let _nos = self.nos.clone();
                        // Spawn task to update contact list
                        let repo = self.repo.clone();
                        // let handle: task::JoinHandle<Result<(), Error>> = task::spawn(async move {

                        let nos_event = event.try_into().unwrap();
                        let contacts = follows_from_event(&nos_event);

                        debug!("New contacts: {:?}", contacts);
                        repo.update_contacts(&nos_event.pubkey.to_hex(), contacts)
                            .await
                            .unwrap();

                        repo.get_all_accounts().unwrap();

                        //Ok(())
                        // });

                        // drop(handle)
                        // handle.await.unwrap().unwrap();
                    }

                    reply = nauthz_grpc::EventReply {
                        decision: Decision::Permit as i32,
                        message: msg,
                    };
                }
                Ok((false, msg)) => {
                    reply = nauthz_grpc::EventReply {
                        decision: Decision::Deny as i32,
                        message: msg,
                    };
                }
                Err(_) => {
                    reply = nauthz_grpc::EventReply {
                        decision: Decision::Deny as i32,
                        message: Some("Error".to_string()),
                    };
                }
            }
        } else {
            reply = nauthz_grpc::EventReply {
                decision: Decision::Deny as i32,
                message: Some("Not allowed to publish".to_string()),
            };
        }

        Ok(Response::new(reply))
    }
}

async fn get_limitation(settings: &Settings, tier: &Tier) -> Limitation {
    match tier {
        Tier::Primary => settings.primary,
        Tier::Secondary => settings.secondary,
        Tier::Tertiary => settings.tertiary,
        Tier::Quaternary => settings.quaternary,
        Tier::Other => settings.other,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();

    tracing_subscriber::fmt::try_init().unwrap();

    let settings = config::Settings::new(&None);

    debug!("{:?}", settings);

    let nos = Nostr::new(&settings.info.relay_url, &settings.info.nostr_key).await?;

    let repo = Repo::new(settings.info.primary_keys.clone());

    init(&settings, &repo, &nos).await?;

    repo.get_all_accounts()?;

    let checker = EventAuthz {
        repo,
        settings,
        nos,
    };

    info!("EventAuthz Server listening on {addr}");
    // Start serving
    Server::builder()
        .add_service(AuthorizationServer::new(checker))
        .serve(addr)
        .await?;
    Ok(())
}

async fn init(settings: &Settings, repo: &Repo, nos: &Nostr) -> Result<(), Error> {
    repo.clear_accounts().await?;
    let primary = settings.info.primary_keys.to_owned();

    repo.set_tier(&primary, Tier::Primary).await?;
    debug!("{} primary accounts set", primary.len());

    let nos_clone = nos.clone();
    let primary_contacts = nos_clone.get_contact_lists(&primary).await?;
    // Add primary keys to DB

    // Filters out events that already have a higher status
    // 1let one: HashMap<String, HashSet<String>> = one.into_iter().filter(|(k, _)| !primary.contains(k)).collect();
    let primary_follows = primary_contacts
        .iter()
        .flat_map(|(_k, f)| f.clone())
        .filter(|k| !primary.contains(k))
        .collect();

    info!("Prima");
    // TODO: Spawn this so next request can start
    repo.set_tier(&primary_follows, Tier::Secondary).await?;
    info!("{} secondary accounts set", primary_follows.len());

    for (pubkey, contacts) in primary_contacts {
        repo.update_contacts(&pubkey, contacts).await?;
    }

    // Add keys from contacts lists to db as One
    let mut secondary_contacts = nos.get_contact_lists(&primary_follows).await?;
    secondary_contacts.retain(|k, _| !primary.contains(k) || !primary_follows.contains(k));
    let secondary_follows = &secondary_contacts
        .iter()
        .flat_map(|(_k, f)| f.clone())
        .filter(|k| !primary.contains(k) || !primary_follows.contains(k))
        .collect();

    // TODO: Spawn this so next request can start
    repo.set_tier(secondary_follows, Tier::Tertiary).await?;
    info!("{} tertiary accounts set", secondary_follows.len());
    for (pubkey, contacts) in secondary_contacts {
        repo.update_contacts(&pubkey, contacts).await?;
    }

    /*

    let mut tertiary_contacts = nos.get_contact_lists(secondary_follows).await?;
    tertiary_contacts.retain(|k, _| !primary.contains(k) || !primary_follows.contains(k) || !secondary_follows.contains(k));
    let tertiary_follows = tertiary_contacts
        .iter()
        .flat_map(|(_k, f)| f.clone())
        .filter(|k| {
            !primary.contains(k) || !primary_follows.contains(k) || !secondary_follows.contains(k) || !tertiary_contacts.contains_key(k)
        })
        .collect();

    // TODO: Spawn this so next request can start
    repo.set_tier(&tertiary_follows, Tier::Quaternary).await?;
    for (pubkey, contacts) in tertiary_contacts {
        // repo.set_contact_list(&pubkey, &contacts).await?;
        debug!("Updating");
        repo.update_contacts(&pubkey, contacts).await.unwrap();
        debug!("Updated");
    }
    */

    info!("Accounts set");
    Ok(())
}
