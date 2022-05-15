use color_eyre::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use teloxide::{
    prelude::{Request, Requester},
    types::Message,
};
use tracing::trace;

use crate::Bot;

fn match_url(text: &str) -> Option<&str> {
    static PATTERN: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"<img class="tgme_page_photo_image" src="(.*?)""#).unwrap());

    Some(PATTERN.captures(text)?.get(1)?.as_str())
}

pub async fn get_chat_avatar(username: &str) -> Result<Option<reqwest::Url>> {
    let html = reqwest::get(format!("https://t.me/{}", username))
        .await?
        .text()
        .await?;
    trace!("{}", html);
    Ok(match_url(&html).and_then(|url| url.parse().ok()))
}

pub async fn is_admin(msg: &Message, bot: &Bot) -> Result<bool> {
    if msg.chat.is_private() {
        return Ok(true);
    }

    let id = match msg.from() {
        Some(u) => u.id,
        None => return Ok(false),
    };

    let ret = bot
        .get_chat_administrators(msg.chat.id)
        .send()
        .await?
        .into_iter()
        .any(|admin| admin.user.id == id);
    Ok(ret)
}

#[tokio::test]
async fn test_get_chat_avatar() {
    let url = get_chat_avatar("durov").await.unwrap().unwrap();
    println!("{}", url);
}
