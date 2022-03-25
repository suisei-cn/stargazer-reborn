use crate::{
    rpc::{
        models::{AddUser, DelUser, GetEntities, GetUser, NewSession, UpdateUserSetting},
        ApiError, ApiResult, Request, ResponseObject,
    },
    server::{serve_with_config, Config},
};

use std::{collections::HashSet, sync::Once, thread::available_parallelism, time::Duration};

use color_eyre::{eyre::Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
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

        // Spawn a server into background, which ideally will be destroyed when all tests are finished.
        std::thread::spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(available_parallelism().unwrap().into())
                .enable_all()
                .build()
                .unwrap()
                .block_on(serve_with_config(Config {
                    bind: "127.0.0.1:8080".parse().unwrap(),
                    token_timeout: Duration::from_secs(0),
                    mongo_uri: "mongodb://192.168.1.53:27017".to_owned(),
                    ..Default::default()
                }))
        });
    });
}

fn call<R: Request + Serialize>(body: R) -> Result<ApiResult<R::Res>>
where
    R::Res: DeserializeOwned,
{
    #[derive(Serialize, Deserialize)]
    #[serde(untagged)]
    enum Shim<R> {
        Ok(R),
        Err(ApiError),
    }

    impl<T> From<Shim<T>> for ApiResult<T> {
        fn from(shim: Shim<T>) -> Self {
            match shim {
                Shim::Ok(res) => ApiResult::Ok(res),
                Shim::Err(err) => ApiResult::Err(err),
            }
        }
    }

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
    let serialized = serde_json::from_str::<ResponseObject<Shim<R::Res>>>(&res)
        .wrap_err("Failed to deserialize")
        .unwrap();

    Ok(serialized.data.into())
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

    let res1 = call(req).unwrap().unwrap();

    let User {
        id,
        im,
        name,
        avatar,
        event_filter,
    } = &res1;

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

    let res2 = call(req).unwrap().unwrap();

    assert_eq!(res1, res2);

    // Delete the new user
    let req = DelUser {
        user_id: id.to_owned().into(),
        password: "TEST".to_owned(),
    };

    let res = call(req).unwrap().is_ok();
    assert!(res);

    // Verify that the user is no longer in the database
    assert!(call(GetUser {
        user_id: id.to_owned().into(),
    })
    .unwrap()
    .is_err());
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

    let obj = call(req).unwrap();

    assert!(obj.is_err());
    assert!(obj
        .unwrap_err()
        .error
        .contains(&"Wrong password".to_owned()));
}

#[test]
fn test_get_entities() {
    prep();

    let req = GetEntities {};

    let res = call(req).unwrap();

    assert!(res.is_ok());
    tracing::info!(entities = ?res);
}

#[test]
fn test_delete_nonexist_user() {
    prep();

    let id = "eee29278-273e-4de9-a794-0a3de92f5c4b";

    let req = DelUser {
        user_id: id.parse().unwrap(),
        password: "TEST".to_owned(),
    };

    let res = call(req).unwrap();
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .error
        .contains(&format!("Cannot find user with ID `{id}`")));
}

#[test]
fn test_update_user_settings() {
    prep();

    let user = AddUser {
        im: "tg".to_owned(),
        avatar: "http://placekitten.com/114/514".parse().unwrap(),
        password: "TEST".to_owned(),
        name: "Pop".to_owned(),
    };

    let User { id, .. } = call(user).unwrap().unwrap();

    let token = call(NewSession {
        password: "TEST".to_owned(),
        user_id: id.into(),
    })
    .unwrap()
    .unwrap()
    .token;

    let event_filter = EventFilter {
        entities: HashSet::from_iter(["a1e28c88-be24-48b0-b18a-81531e669905"
            .parse::<uuid::Uuid>()
            .unwrap()
            .into()]),
        kinds: HashSet::from_iter(["twitter/new_tweet".to_owned()]),
    };

    let update = UpdateUserSetting {
        user_id: id.into(),
        token,
        event_filter: event_filter.clone(),
    };

    let res = call(update).unwrap();

    assert!(res.is_ok());

    let user = call(GetUser { user_id: id.into() }).unwrap().unwrap();

    assert_eq!(user.event_filter, event_filter);
}
