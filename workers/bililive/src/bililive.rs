use eyre::Result;
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::{Deserialize, Serialize};

static HTTP: Lazy<Client> = Lazy::new(Client::new);

#[derive(Debug, Deserialize)]
struct Raw {
    data: RawLiveRoom,
}

#[derive(Debug, Deserialize)]
struct RawLiveRoom {
    title: String,
    user_cover: String,
    room_id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LiveRoom {
    title: String,
    link: String,
    cover: Option<String>,
}

impl LiveRoom {
    pub async fn new(room_id: u64) -> Result<Self> {
        let resp = HTTP
            .get("https://api.live.bilibili.com/room/v1/Room/get_info")
            .query(&[("room_id", room_id)])
            .send()
            .await?;
        let room: Raw = resp.json().await?;
        Ok(Self {
            title: room.data.title,
            link: format!("https://live.bilibili.com/{}", room.data.room_id),
            cover: if room.data.user_cover.is_empty() {
                None
            } else {
                Some(room.data.user_cover)
            },
        })
    }
}
