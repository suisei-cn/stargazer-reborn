//! Test suite
//!
//! This test will temporarily generate a record in auth database with full access privilege, which
//! will be cleaned up after the test.
//!
//! Username: "test"
//! Password: "test"
mod prep {
    use std::{
        ops::{Deref, DerefMut},
        sync::atomic::{AtomicBool, AtomicU16, Ordering},
        time::Duration,
    };

    use once_cell::sync::OnceCell;
    use sg_auth::{AuthClient, PermissionRecord, PermissionSet};
    use tokio::{runtime::Runtime, time::timeout};
    use tracing::{info, metadata::LevelFilter};

    use crate::{
        client::blocking::Client,
        server::{make_app_with, Config},
    };

    static CURRENT: OnceCell<(Runtime, AuthClient)> = OnceCell::new();
    static INITIALIZED: AtomicBool = AtomicBool::new(false);
    static TEST_RUNNING: AtomicU16 = AtomicU16::new(0);

    pub struct TestGuard {
        client: Client,
    }

    impl TestGuard {
        pub fn new(client: Client) -> Self {
            TEST_RUNNING.fetch_add(1, Ordering::Acquire);
            Self { client }
        }
    }

    impl Drop for TestGuard {
        fn drop(&mut self) {
            if TEST_RUNNING.fetch_sub(1, Ordering::Release) != 1 {
                return;
            }

            info!("Last running test stopped, start cleaning");

            let (rt, auth) = CURRENT.get().unwrap();
            rt.block_on(async move {
                auth.delete_record("test").await.unwrap();
            });
        }
    }

    impl Deref for TestGuard {
        type Target = Client;
        fn deref(&self) -> &Self::Target {
            &self.client
        }
    }

    impl DerefMut for TestGuard {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.client
        }
    }

    /// One time initialization of the test suite.
    ///
    /// This will spin up a runtime, register test admin and start the server
    fn init() -> (Runtime, AuthClient) {
        tracing_subscriber::fmt()
            .with_max_level(LevelFilter::INFO)
            .init();

        color_eyre::install().unwrap();

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        let (server, app, auth) = rt.block_on(async {
            let mongo_uri = std::env::var("MONGODB_URI")
                .unwrap_or_else(|_| "mongodb://localhost:27017".to_owned());

            info!(%mongo_uri, "Connecting to mongodb");

            let db = mongodb::Client::with_uri_str(&mongo_uri)
                .await
                .unwrap()
                .database("stargazer-reborn");
            let col = db.collection::<PermissionRecord>("auth");

            let auth = AuthClient::new(col);
            timeout(
                Duration::from_secs(1),
                auth.new_record("test", "test", PermissionSet::FULL),
            )
            .await
            .expect("Failed to connect to mongodb")
            .unwrap();

            let server = axum::Server::bind(&"127.0.0.1:8080".parse().unwrap());

            let app = make_app_with(
                Config {
                    token_timeout: Duration::from_secs(0),
                    mongo_uri,
                    ..Config::default()
                },
                Some(db),
            )
            .await
            .unwrap()
            .into_make_service();

            (server, app, auth)
        });

        rt.spawn(async move {
            tracing::info!("Server starting");

            INITIALIZED.store(true, Ordering::Release);

            server.serve(app).await.unwrap();

            tracing::info!("Server stopped");
        });

        (rt, auth)
    }

    pub fn prep() -> TestGuard {
        CURRENT.get_or_init(init);

        let start = std::time::Instant::now();

        while !INITIALIZED.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(100));
            assert!(
                start.elapsed().as_secs() <= 3,
                "Initialize test suite timeout"
            );
        }

        let mut c = Client::new("http://127.0.0.1:8080/v1/").unwrap();
        c.login_and_store("test", "test").unwrap();
        TestGuard::new(c)
    }
}

use std::collections::HashSet;

use crate::model::UserQuery;

use mongodb::bson::Uuid;
use once_cell::sync::Lazy;
use prep::prep;
use rand::Rng;
use reqwest::Url;
use sg_core::models::{EventFilter, User};

static URL: Lazy<Url> = Lazy::new(|| Url::parse("http://placekitten.com/114/514").unwrap());

fn gen_payload() -> String {
    rand::thread_rng()
        .gen_range(-100_000_000..100_000_000_i64)
        .to_string()
}

#[test]
fn test_new_user() {
    let mut c = prep();
    let payload = gen_payload();

    let res1 = c
        .add_user(
            "tg".to_owned(),
            payload.clone(),
            URL.clone(),
            "Pop".to_owned(),
        )
        .unwrap();

    let User {
        id,
        im,
        im_payload,
        name,
        avatar,
        event_filter,
    } = &res1;

    assert_eq!(im, "tg");
    assert_eq!(im_payload, &payload);
    assert_eq!(name, "Pop");
    assert_eq!(
        avatar.as_ref().map(Url::as_str).unwrap(),
        "http://placekitten.com/114/514"
    );
    assert_eq!(
        event_filter,
        &EventFilter {
            entities: HashSet::default(),
            kinds: HashSet::default(),
        }
    );

    tracing::info!(id = ?id, "New user added");

    // Make sure duplicate users are not allowed
    let err = c
        .add_user("tg", payload, URL.clone(), "SomeOtherName")
        .unwrap_err();
    match err {
        crate::client::Error::Api(err) => {
            assert_eq!(err.error_reason(), Some("Conflict"));
        }
        _ => panic!("Unexpected error: {:?}", err),
    }

    let token = c.new_token(UserQuery::ById { user_id: *id }).unwrap().token;

    // Pretend we are the new user
    let admin_token = c.set_token(token).unwrap();

    // Verify that the user is in the database
    let res2 = c.auth_user().unwrap().user;

    assert_eq!(res1, res2);

    // Delete the new user
    c.set_token(admin_token).unwrap();
    let res3 = c.del_user(UserQuery::ById { user_id: *id }).unwrap();

    assert_eq!(res2, res3);

    // Verify that the user is no longer in the database
    drop(c.auth_user().unwrap_err());
}

#[test]
fn test_get_entities() {
    let c = prep();

    c.get_entities().unwrap();
}

#[test]
fn test_delete_nonexist_user() {
    let c = prep();

    let id = "eee29278-273e-4de9-a794-0a3de92f5c4b";

    let res = c
        .del_user(UserQuery::ById {
            user_id: Uuid::parse_str(id).unwrap(),
        })
        .unwrap_err();

    match res {
        crate::client::Error::Api(e) => {
            assert!(e.errors()[1].contains(&format!("Cannot find user with ID `{id}`")));
        }
        _ => panic!("Unexpected error: {:?}", res),
    }
}

#[test]
fn test_update_user_settings() {
    let mut c = prep();

    // Generate a new user
    let user_id = c
        .add_user(
            "tg".to_owned(),
            gen_payload(),
            URL.clone(),
            "Pop".to_owned(),
        )
        .unwrap()
        .id;

    // Get a token with current admin privilege
    let token = c.new_token(UserQuery::ById { user_id }).unwrap().token;

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
    c.update_setting(event_filter.clone()).unwrap();

    // Get new user info
    let user = c.auth_user().unwrap().user;

    // Assert they are the equal
    assert_eq!(user.event_filter, event_filter);
}
