use crate::{
    rpc::{
        models::{AddUser, DelUser, Entities, GetEntities, GetUser, Null},
        ApiError, Request, Response, ResponseObject,
    },
    server::{serve_with_config, Config},
};

use std::{sync::Once, time::Duration};

use color_eyre::{eyre::Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use sg_core::models::{EventFilter, User};
use tracing::metadata::LevelFilter;

static INITIATED: Once = Once::new();

fn prep() {
    INITIATED.call_once(|| {
        tracing_subscriber::fmt()
            .with_max_level(LevelFilter::INFO)
            .init();

        tracing::info!("Initializing test suite");

        color_eyre::install().unwrap();

        // Spawn a server into background, which ideally will be destroyed when all tests are done.
        std::thread::spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .unwrap()
                .block_on(serve_with_config(Config {
                    bind: "127.0.0.1:8080".parse().unwrap(),
                    session_timeout: Duration::from_secs(0),
                    mongo_uri: "mongodb://192.168.1.53:27017".to_owned(),
                    ..Default::default()
                }))
        });
    });

    std::thread::sleep(Duration::from_secs(1));
}

fn call<B: Request + Serialize, R: Response + DeserializeOwned>(
    body: B,
) -> Result<ResponseObject<R>> {
    let res = reqwest::blocking::Client::builder()
        .build()?
        .post("http://127.0.0.1:8080/v1")
        .body(body.packed().to_json())
        .header("Content-Type", "application/json")
        .send()
        .wrap_err("Failed to send request")?
        .text()
        .wrap_err("Failed to read response")?;

    tracing::info!(res = res.as_str());
    serde_json::from_str(&res).wrap_err("Failed to deserialize")
}

#[test]
fn test_new_user() {
    prep();

    let req = AddUser {
        im: "tg".to_owned(),
        avatar: "http://placekitten.com/114/514".parse().unwrap(),
        password: "TEST".to_owned(),
        name: "Pop".to_owned(),
    };

    let res1 = call::<_, User>(req).unwrap();

    let User {
        id,
        im,
        name,
        avatar,
        event_filter,
    } = &res1.data;

    assert_eq!(im, "tg");
    assert_eq!(name, "Pop");
    assert_eq!(avatar.as_str(), "http://placekitten.com/114/514");
    assert_eq!(
        event_filter,
        &EventFilter {
            entities: Default::default(),
            kinds: Default::default(),
        }
    );

    tracing::info!(id = ?id, "New user added");

    // Verify that the user is in the database
    let req = GetUser {
        user_id: id.to_owned().into(),
    };

    let res2 = call::<_, User>(req).unwrap();

    assert_eq!(res1.data, res2.data);

    // Delete the new user
    let req = DelUser {
        user_id: id.to_owned().into(),
        password: "TEST".to_owned(),
    };

    let res = call::<_, Null>(req).unwrap().is_success();
    assert!(res)
}

#[test]
fn test_new_user_wrong_password() {
    prep();

    let req = AddUser {
        im: "tg".to_owned(),
        avatar: "http://placekitten.com/114/514".parse().unwrap(),
        password: "WRONG_PASSWORD".to_owned(),
        name: "Pop".to_owned(),
    };

    let obj = call::<_, ApiError>(req).unwrap();

    assert!(!obj.success);
    assert!(obj.data.error.contains(&"Wrong password".to_owned()));
}

#[test]
fn test_get_entities() {
    prep();

    let req = GetEntities {};

    let res = call::<_, Entities>(req).unwrap();

    assert!(res.success);
    tracing::info!(entities = ?res.data);
}

#[test]
fn test_delete_nonexist_user() {
    prep();

    let id = "eee29278-273e-4de9-a794-0a3de92f5c4b";

    let req = DelUser {
        user_id: id.parse().unwrap(),
        password: "TEST".to_owned(),
    };

    let res = call::<_, ApiError>(req).unwrap();
    assert!(res.is_error());
    assert!(res
        .data
        .error
        .contains(&format!("Cannot find user with ID `{id}`")));
}
