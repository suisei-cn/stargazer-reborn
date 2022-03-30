use std::collections::{HashMap, HashSet};

use crate::rpc::{
    models::{
        AddEntity, AddTask, AddTaskParam, AddUser, AuthUser, DelUser, GetEntities, NewSession,
        Session, UpdateSetting,
    },
    ApiError, ApiResult, Request, ResponseObject,
};

use color_eyre::{eyre::Context, Result};
use isolanguage_1::LanguageCode;
use mongodb::bson::{doc, Uuid};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sg_core::models::{Entity, EventFilter, Meta, Name, User};

mod prep {
    use std::{sync::Once, thread::available_parallelism, time::Duration,env};

    use tracing::metadata::LevelFilter;

    use crate::server::{serve_with_config, Config};

    static INIT: Once = Once::new();

    pub fn prep() {
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
                    .block_on(serve_with_config(Config {
                        bind: "127.0.0.1:8080".parse().unwrap(),
                        token_timeout: Duration::from_secs(0),
                        mongo_uri: env::var("MONGODB_URL").expect("MONGODB_URL is not se"),
                        ..Default::default()
                    }))
            });
        });
    }
}

use prep::prep;

fn new_session(user_id: Uuid) -> Result<ApiResult<Session>> {
    call(NewSession {
        password: "TEST".to_owned(),
        user_id,
    })
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
        is_admin,
        event_filter,
    } = &res1;

    assert_eq!(im, "tg");
    assert_eq!(name, "Pop");
    assert_eq!(avatar.as_str(), "http://placekitten.com/114/514");
    assert!(!is_admin);
    assert_eq!(
        event_filter,
        &EventFilter {
            entities: Default::default(),
            kinds: Default::default(),
        }
    );

    tracing::info!(id = ?id, "New user added");

    let token = new_session(*id).unwrap().unwrap().token;

    // Verify that the user is in the database
    let req = AuthUser {
        user_id: id.to_owned(),
        token: token.clone(),
    };

    let res2 = call(req).unwrap().unwrap().user;

    assert_eq!(res1, res2);

    // Delete the new user
    let req = DelUser {
        user_id: id.to_owned(),
        password: "TEST".to_owned(),
    };

    let res = call(req).unwrap().is_ok();
    assert!(res);

    // Verify that the user is no longer in the database
    assert!(call(AuthUser {
        user_id: id.to_owned(),
        token
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

    let res = call(req).unwrap().unwrap();

    tracing::info!(entities = ?res);
}

#[test]
fn test_delete_nonexist_user() {
    prep();

    let id = "eee29278-273e-4de9-a794-0a3de92f5c4b";

    let req = DelUser {
        user_id: Uuid::parse_str(id).unwrap(),
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

    let user_id = call(user).unwrap().unwrap().id;

    let token = call(NewSession {
        password: "TEST".to_owned(),
        user_id,
    })
    .unwrap()
    .unwrap()
    .token;

    let event_filter = EventFilter {
        entities: HashSet::from_iter([
            Uuid::parse_str("a1e28c88-be24-48b0-b18a-81531e669905").unwrap()
        ]),
        kinds: HashSet::from_iter(["twitter/new_tweet".to_owned()]),
    };

    let update = UpdateSetting {
        token,
        event_filter: event_filter.clone(),
    };

    let res = call(update).unwrap();

    assert!(res.is_ok());
    let token = new_session(user_id).unwrap().unwrap().token;

    let user = call(AuthUser { user_id, token }).unwrap().unwrap().user;

    assert_eq!(user.event_filter, event_filter);
}

#[test]
fn test_admin() {
    prep();

    let admin_id = Uuid::parse_str("7f04280b-1840-1006-ca6d-064b9bf680cd").unwrap();

    // Get admin token
    let token = call(NewSession {
        password: "TEST".to_owned(),
        user_id: admin_id,
    })
    .unwrap()
    .unwrap()
    .token;

    let name = Name {
        name: HashMap::from_iter([(LanguageCode::En, "Test".to_owned())]),
        default_language: LanguageCode::En,
    };

    let new_ent = call(AddEntity {
        meta: Meta { name, group: None },
        token: token.clone(),
        tasks: vec![AddTaskParam::Youtube {
            channel_id: "TestChannel".to_owned(),
        }],
    })
    .unwrap()
    .unwrap();

    tracing::info!(?new_ent);

    let new_task = call(AddTask {
        entity_id: new_ent.id,
        token,
        param: AddTaskParam::Bilibili {
            uid: "Test".to_owned(),
        },
    })
    .unwrap()
    .unwrap();

    tracing::info!(?new_task);

    // new_ent.tasks.push(new_task);

    let ent_in_db = call(GetEntities {})
        .unwrap()
        .unwrap()
        .vtbs
        .into_iter()
        .find(|x| x.id == new_ent.id)
        .unwrap();

    assert_eq!(ent_in_db.tasks.len(), 2);
    assert!(ent_in_db.tasks.contains(&new_task.id));
    assert_eq!(ent_in_db.meta, new_ent.meta);
}

#[tokio::test]
async fn test_get_entity_from_db() {
    let col = mongodb::Client::with_uri_str(std::env::var("MONGODB_URL").unwrap())
        .await
        .unwrap()
        .database("stargazer-reborn")
        .collection::<Entity>("entities");

    let res = col
        .find_one(None, None)
        .await
        .unwrap()
        // .try_collect::<Vec<_>>()
        // .await
        .unwrap();

    println!("{:?}", res);
}
