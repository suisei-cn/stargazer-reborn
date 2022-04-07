mod prep {
    use std::{
        env,
        sync::{
            atomic::{AtomicBool, Ordering},
            Once,
        },
        thread::available_parallelism,
        time::Duration,
    };

    use once_cell::sync::Lazy;
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
                            ..Default::default()
                        })
                        .await
                        .unwrap();
                    });
            });
        });

        if !WAITED.load(Ordering::Acquire) {
            WAITED.store(true, Ordering::Release);
            std::thread::sleep(Duration::from_secs(2));
        }

        let mut c = Client::new("http://127.0.0.1:8080/v1/").unwrap();
        c.login_and_store("test", "test").unwrap().unwrap();
        c
    }
}

use std::collections::{HashMap, HashSet};

use crate::{
    model::UserQuery,
    rpc::{
        model::{
            AddEntity, AddTask, AddTaskParam, AddUser, AuthUser, DelUser, GetEntities, NewToken,
            Token, UpdateSetting,
        },
        ApiError, ApiResult, Request, ResponseObject,
    },
};

use color_eyre::{eyre::Context, Result};
use isolanguage_1::LanguageCode;
use mongodb::bson::{doc, Uuid};
use prep::prep;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sg_core::models::{Entity, EventFilter, Meta, Name, User};

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
        is_admin,
        event_filter,
        ..
    } = &res1;

    assert_eq!(im, "tg");
    assert_eq!(name, "Pop");
    assert_eq!(avatar.as_str(), "http://placekitten.com/114/514");
    assert!(!is_admin);
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

// #[test]
// fn test_new_user_wrong_password() {
//     prep();

//     let req = AddUser {
//         im: "tg".to_owned(),
//         avatar: "http://placekitten.com/114/514".parse().unwrap(),
//         password: "WRONG_PASSWORD".to_owned(),
//         name: "Pop".to_owned(),
//     };

//     let obj = call(req).unwrap();

//     assert!(obj.is_err());
//     assert!(obj
//         .unwrap_err()
//         .error
//         .contains(&"Wrong password".to_owned()));
// }

// #[test]
// fn test_get_entities() {
//     prep();

//     let req = GetEntities {};

//     let res = call(req).unwrap().unwrap();

//     tracing::info!(entities = ?res);
// }

// #[test]
// fn test_delete_nonexist_user() {
//     prep();

//     let id = "eee29278-273e-4de9-a794-0a3de92f5c4b";

//     let req = DelUser {
//         user_id: Uuid::parse_str(id).unwrap(),
//         password: "TEST".to_owned(),
//     };

//     let res = call(req).unwrap();
//     assert!(res.is_err());
//     assert!(res
//         .unwrap_err()
//         .error
//         .contains(&format!("Cannot find user with ID `{id}`")));
// }

// #[test]
// fn test_update_user_settings() {
//     prep();

//     let user = AddUser {
//         im: "tg".to_owned(),
//         avatar: "http://placekitten.com/114/514".parse().unwrap(),
//         password: "TEST".to_owned(),
//         name: "Pop".to_owned(),
//     };

//     let user_id = call(user).unwrap().unwrap().id;

//     let token = call(NewToken {
//         password: "TEST".to_owned(),
//         user_id,
//     })
//     .unwrap()
//     .unwrap()
//     .token;

//     let event_filter = EventFilter {
//         entities: HashSet::from_iter([
//             Uuid::parse_str("a1e28c88-be24-48b0-b18a-81531e669905").unwrap()
//         ]),
//         kinds: HashSet::from_iter(["twitter/new_tweet".to_owned()]),
//     };

//     let update = UpdateSetting {
//         token,
//         event_filter: event_filter.clone(),
//     };

//     let res = call(update).unwrap();

//     assert!(res.is_ok());
//     let token = new_session(user_id).unwrap().unwrap().token;

//     let user = call(AuthUser { user_id, token }).unwrap().unwrap().user;

//     assert_eq!(user.event_filter, event_filter);
// }

// #[test]
// fn test_admin() {
//     prep();

//     let admin_id = Uuid::parse_str("7f04280b-1840-1006-ca6d-064b9bf680cd").unwrap();

//     // Get admin token
//     let token = call(NewToken {
//         password: "TEST".to_owned(),
//         user_id: admin_id,
//     })
//     .unwrap()
//     .unwrap()
//     .token;

//     let name = Name {
//         name: HashMap::from_iter([(LanguageCode::En, "Test".to_owned())]),
//         default_language: LanguageCode::En,
//     };

//     let new_ent = call(AddEntity {
//         meta: Meta { name, group: None },
//         token: token.clone(),
//         tasks: vec![AddTaskParam::Youtube {
//             channel_id: "TestChannel".to_owned(),
//         }],
//     })
//     .unwrap()
//     .unwrap();

//     tracing::info!(?new_ent);

//     let new_task = call(AddTask {
//         entity_id: new_ent.id,
//         token,
//         param: AddTaskParam::Bilibili {
//             uid: "Test".to_owned(),
//         },
//     })
//     .unwrap()
//     .unwrap();

//     tracing::info!(?new_task);

//     // new_ent.tasks.push(new_task);

//     let ent_in_db = call(GetEntities {})
//         .unwrap()
//         .unwrap()
//         .vtbs
//         .into_iter()
//         .find(|x| x.id == new_ent.id)
//         .unwrap();

//     assert_eq!(ent_in_db.tasks.len(), 2);
//     assert!(ent_in_db.tasks.contains(&new_task.id));
//     assert_eq!(ent_in_db.meta, new_ent.meta);
// }

// #[tokio::test]
// async fn test_get_entity_from_db() {
//     let col = mongodb::Client::with_uri_str(std::env::var("MONGODB_URI").unwrap())
//         .await
//         .unwrap()
//         .database("stargazer-reborn")
//         .collection::<Entity>("entities");

//     let res = col
//         .find_one(None, None)
//         .await
//         .unwrap()
//         // .try_collect::<Vec<_>>()
//         // .await
//         .unwrap();

//     println!("{:?}", res);
// }
