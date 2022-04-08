mod prep {
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            Once,
        },
        thread::available_parallelism,
        time::Duration,
    };

    use sg_auth::{AuthClient, PermissionRecord, PermissionSet};
    use tracing::metadata::LevelFilter;

    use crate::{
        client::blocking::Client,
        server::{serve_with_config, Config},
    };

    static INIT: Once = Once::new();
    static WAITED: AtomicBool = AtomicBool::new(false);

    pub fn prep() -> Client {
        INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_max_level(LevelFilter::INFO)
                .init();

            tracing::info!("Initializing test suite");

            color_eyre::install().unwrap();

            // Spawn a server into background, which ideally will be destroyed when all tests are finished.
            std::thread::spawn(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(available_parallelism().unwrap().into())
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async {
                        let mongo_uri = std::env::var("MONGODB_URI")
                            .unwrap_or_else(|_| "mongodb://localhost:27017".to_owned());

                        let col = mongodb::Client::with_uri_str(&mongo_uri)
                            .await
                            .unwrap()
                            .database("stargazer-reborn")
                            .collection::<PermissionRecord>("auth");

                        AuthClient::new(col)
                            .new_record("test", "test", PermissionSet::FULL)
                            .await
                            .unwrap();

                        serve_with_config(Config {
                            bind: "127.0.0.1:8080".parse().unwrap(),
                            token_timeout: Duration::from_secs(0),
                            mongo_uri,
                            ..Config::default()
                        })
                        .await
                        .unwrap();
                    });
            });
        });

        if !WAITED.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_secs(2));
            WAITED.store(true, Ordering::Release);
        }

        let mut c = Client::new("http://127.0.0.1:8080/v1/").unwrap();
        c.login_and_store("test", "test").unwrap().unwrap();
        c
    }
}

use std::collections::HashSet;

use crate::model::UserQuery;

use mongodb::bson::Uuid;
use prep::prep;
use sg_core::models::{EventFilter, User};

#[test]
fn test_new_user() {
    let mut c = prep();

    let res1 = c
        .add_user(
            "tg".to_owned(),
            "TEST".to_owned(),
            "http://placekitten.com/114/514".parse().unwrap(),
            "Pop".to_owned(),
        )
        .unwrap()
        .unwrap();

    let User {
        id,
        im,
        name,
        avatar,
        event_filter,
        ..
    } = &res1;

    assert_eq!(im, "tg");
    assert_eq!(name, "Pop");
    assert_eq!(avatar.as_str(), "http://placekitten.com/114/514");
    assert_eq!(
        event_filter,
        &EventFilter {
            entities: HashSet::default(),
            kinds: HashSet::default(),
        }
    );

    tracing::info!(id = ?id, "New user added");

    let token = c
        .new_token(UserQuery::ById { user_id: *id })
        .unwrap()
        .unwrap()
        .token;

    // Pretend we are the new user
    let admin_token = c.set_token(token).unwrap();

    // Verify that the user is in the database
    let res2 = c.auth_user().unwrap().unwrap().user;

    assert_eq!(res1, res2);

    // Delete the new user
    c.set_token(admin_token).unwrap();
    let res3 = c
        .del_user(UserQuery::ById { user_id: *id })
        .unwrap()
        .unwrap();

    assert_eq!(res2, res3);

    // Verify that the user is no longer in the database
    drop(c.auth_user().unwrap().unwrap_err());
}

#[test]
fn test_get_entities() {
    let c = prep();

    let res = c.get_entities().unwrap().unwrap();

    tracing::info!(entities = ?res);
}

#[test]
fn test_delete_nonexist_user() {
    let c = prep();

    let id = "eee29278-273e-4de9-a794-0a3de92f5c4b";

    let res = c
        .del_user(UserQuery::ById {
            user_id: Uuid::parse_str(id).unwrap(),
        })
        .unwrap();

    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .error
        .contains(&format!("Cannot find user with ID `{id}`")));
}

#[test]
fn test_update_user_settings() {
    let mut c = prep();

    // Generate a new user
    let user_id = c
        .add_user(
            "tg".to_owned(),
            "TEST".to_owned(),
            "http://placekitten.com/114/514".parse().unwrap(),
            "Pop".to_owned(),
        )
        .unwrap()
        .unwrap()
        .id;

    // Get a token with current admin privilege
    let token = c
        .new_token(UserQuery::ById { user_id })
        .unwrap()
        .unwrap()
        .token;

    // change to this user
    c.set_token(token).unwrap();

    // New event filter, a.k.a. setting
    let event_filter = EventFilter {
        entities: HashSet::from_iter([
            Uuid::parse_str("a1e28c88-be24-48b0-b18a-81531e669905").unwrap()
        ]),
        kinds: HashSet::from_iter(["twitter/new_tweet".to_owned()]),
    };

    // Update setting on behalf of this user
    c.update_setting(event_filter.clone()).unwrap().unwrap();

    // Get new user info
    let user = c.auth_user().unwrap().unwrap().user;

    // Assert they are the equal
    assert_eq!(user.event_filter, event_filter);
}
