//! "Bench" mongo db performance when fetching user with EventFilter.
//!
//! This is tested with:
//!
//! - 32 entities
//! - 9 kinds
//! - 10 - 1600 users
//! - 10 iters per test case (user count as the variable)
//!
//! # Result
//!
//! ```text
//! [user count] Avg / Min / Max
//! [10] 11.342ms / 51.211ms / 4.596ms
//! [20] 19.064ms / 101.484ms / 7.066ms
//! [30] 14.401ms / 40.909ms / 8.571ms
//! [40] 16.180ms / 53.359ms / 10.516ms
//! [50] 11.979ms / 30.528ms / 8.048ms
//! [60] 25.573ms / 99.527ms / 11.373ms
//! [70] 19.489ms / 61.426ms / 12.115ms
//! [80] 17.269ms / 30.525ms / 13.419ms
//! [90] 25.834ms / 101.540ms / 9.781ms
//! [100] 15.384ms / 32.321ms / 11.723ms
//! [110] 25.366ms / 58.803ms / 11.579ms
//! [120] 21.216ms / 39.423ms / 9.916ms
//! [130] 37.533ms / 181.314ms / 12.477ms
//! [140] 25.487ms / 102.258ms / 13.303ms
//! [150] 24.793ms / 63.914ms / 12.740ms
//! [160] 24.134ms / 53.481ms / 15.391ms
//! [170] 23.749ms / 42.994ms / 17.424ms
//! [180] 37.758ms / 181.989ms / 16.893ms
//! [190] 32.825ms / 103.958ms / 18.065ms
//! [200] 48.579ms / 246.094ms / 19.020ms
//! [210] 35.451ms / 104.932ms / 19.619ms
//! [220] 22.923ms / 32.909ms / 19.652ms
//! [230] 19.980ms / 21.855ms / 17.046ms
//! [240] 21.212ms / 26.151ms / 17.469ms
//! [250] 22.845ms / 27.864ms / 20.507ms
//! [260] 22.475ms / 24.434ms / 20.351ms
//! [270] 23.340ms / 34.674ms / 19.895ms
//! [280] 26.183ms / 36.957ms / 22.802ms
//! [290] 25.414ms / 35.593ms / 21.954ms
//! [300] 27.348ms / 37.424ms / 23.398ms
//! [310] 24.757ms / 26.873ms / 21.972ms
//! [320] 23.518ms / 27.008ms / 21.372ms
//! [330] 28.866ms / 36.532ms / 23.263ms
//! [340] 35.250ms / 42.504ms / 27.822ms
//! [350] 32.166ms / 37.637ms / 27.145ms
//! [360] 29.981ms / 35.174ms / 23.912ms
//! [370] 31.635ms / 39.358ms / 24.122ms
//! [380] 36.977ms / 42.303ms / 25.412ms
//! [390] 36.250ms / 41.359ms / 28.416ms
//! [400] 36.629ms / 41.060ms / 29.302ms
//! [600] 48.356ms / 49.829ms / 44.002ms
//! [800] 62.681ms / 73.087ms / 54.388ms
//! [1000] 76.279ms / 82.286ms / 71.134ms
//! [1200] 86.602ms / 98.147ms / 79.686ms
//! [1400] 92.888ms / 101.721ms / 85.319ms
//! [1600] 105.987ms / 118.933ms / 96.213ms
//! ```

use std::{collections::HashMap, env};

use color_eyre::Result;
use fake::{faker::name::en::Name as FakeName, Fake, Faker};
use futures::StreamExt;
use mongodb::{bson::doc, Collection};
use rand::{
    prelude::{SliceRandom, ThreadRng},
    thread_rng, Rng,
};
use sg_core::models::{Entity, EventFilter, Meta, Name, User};
use tokio::time::Instant;

const KINDS: &[&str] = &[
    "twitter/new_tweet",
    "twitter/retweet",
    "bilibili/live_start",
    "bilibili/new_dynamic",
    "bilibili/forward_dynamic",
    "youtube/new_video",
    "youtube/live_start",
    "youtube/broadcast_scheduled",
    "youtube/30_min_before_broadcast",
];

fn gen_user(event_filter: EventFilter) -> User {
    let mut rng = thread_rng();
    let id: uuid::Uuid = Faker.fake();

    User {
        id: id.into(),
        name: FakeName().fake(),
        event_filter,
        avatar: "http://placekitten.com/114/514".parse().ok(),
        im: ["tg", "qq"].choose(&mut rng).unwrap().to_owned().to_owned(),
        im_payload: Faker.fake(),
    }
}

fn gen_entity() -> Entity {
    let en = isolanguage_1::LanguageCode::En;
    let id: uuid::Uuid = Faker.fake();
    let name = Name {
        name: HashMap::from_iter([(en, FakeName().fake())]),
        default_language: en,
    };
    let meta = Meta { name, group: None };
    Entity {
        id: id.into(),
        meta,
        tasks: vec![],
    }
}

fn gen_ef(rng: &mut ThreadRng, entities: &[uuid::Uuid]) -> EventFilter {
    let kinds_len = rng.gen_range(2..KINDS.len());
    let entities_len = rng.gen_range(2..entities.len());
    let kinds = KINDS
        .choose_multiple(rng, kinds_len)
        .map(|x| (*x).to_owned())
        .collect();
    let entities = entities
        .choose_multiple(rng, entities_len)
        .map(|x| (*x).into())
        .collect();
    EventFilter { kinds, entities }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();
    color_eyre::install()?;

    let client = mongodb::Client::with_uri_str(env::var("MONGODB_URI")?).await?;
    let db = client.database("stargazer-reborn");

    // for count in (10..=400).step_by(10).chain((600..=1600).step_by(200)) {
    if env::var("gen_user").is_ok() {
        tracing::info!("Generating users");
        let users = db.collection::<User>("users");
        let count = 100;
        let stat = get_avg_time_with_user_count(users.clone(), count).await?;
        let avg = stat.iter().sum::<u64>() as f64 / (10.0 * 1000.0);
        let max = *stat.iter().max().unwrap() as f64 / 1000.0;
        let min = *stat.iter().min().unwrap() as f64 / 1000.0;
        tracing::info!("{:-4}] {:.3}ms / {:.3}ms / {:.3}ms", count, avg, max, min);
    }

    if env::var("gen_ent").is_ok() {
        tracing::info!("Generating entities");
        let entities = db.collection::<Entity>("entities");
        let count = 100;
        let ent = std::iter::repeat(())
            .take(count)
            .map(|_| gen_entity())
            .collect::<Vec<_>>();
        entities.insert_many(&ent, None).await?;
    }

    Ok(())
}

async fn get_avg_time_with_user_count(
    users: Collection<User>,
    user_count: u32,
) -> Result<Vec<u64>> {
    let mut rng = thread_rng();

    users.drop(None).await?;

    let entities: [uuid::Uuid; 32] = Faker.fake();

    let data: Vec<User> = (0..user_count)
        .map(|_| gen_user(gen_ef(&mut rng, entities.as_slice())))
        .collect();

    users.insert_many(data, None).await?;

    let mut stat = vec![];

    for _ in 0..10 {
        let id = entities.choose(&mut rng).unwrap();
        let kind = KINDS.choose(&mut rng).unwrap();
        let start = Instant::now();
        let res = users
            .find(
                doc! {
                  "event_filter.entities": id,
                  "event_filter.kinds": kind,
                },
                None,
            )
            .await
            .unwrap();
        let len = res.collect::<Vec<_>>().await.len();
        let dur = start.elapsed();
        stat.push(dur.as_micros() as u64);
        tracing::debug!(len);
    }
    Ok(stat)
}
