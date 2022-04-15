use std::collections::HashMap;
use std::mem;
use std::time::Duration;

use eyre::Result;
use once_cell::sync::Lazy;
use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use tracing::error;
use url::Url;
use uuid::Uuid;

use sg_core::utils::ScopedJoinHandle;

use crate::models::{Mode, SubscribeForm, Verify};
use crate::Config;

static HTTP: Lazy<ClientWithMiddleware> = Lazy::new(|| {
    ClientBuilder::new(Client::new())
        .with(RetryTransientMiddleware::new_with_policy(
            ExponentialBackoff::builder().build_with_max_retries(5),
        ))
        .build()
});

pub struct Registry {
    config: Config,

    channels: HashMap<Uuid, Channel>,
    channels_rev: HashMap<String, Uuid>,
}

impl Registry {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            channels: HashMap::new(),
            channels_rev: HashMap::new(),
        }
    }
    pub fn add_channel(&mut self, id: Uuid, channel_id: String) -> bool {
        if self.channels.contains_key(&id) {
            return false;
        }

        let mut callback_url = self.config.base_url.clone();
        callback_url.path_segments_mut().unwrap().push("callback");
        self.channels.insert(
            id,
            Channel::new(&channel_id, &callback_url, self.config.lease),
        );
        self.channels_rev.insert(channel_id, id);

        true
    }
    pub fn remove_channel(&mut self, id: Uuid) -> bool {
        if !self.channels.contains_key(&id) {
            return false;
        }

        let channel = self.channels.remove(&id).unwrap();
        self.channels_rev.remove(&channel.channel_id);

        true
    }
    pub fn id_by_channel_id(&self, channel_id: &str) -> Option<Uuid> {
        self.channels_rev.get(channel_id).copied()
    }
    pub fn contains_id(&self, id: Uuid) -> bool {
        self.channels.contains_key(&id)
    }
    pub fn contains_channel(&self, channel_id: &str) -> bool {
        self.channels_rev.contains_key(channel_id)
    }
}

struct Channel {
    channel_id: String,
    callback: String,
    lease_duration: Duration,
    handle: ScopedJoinHandle<()>,
}

impl Channel {
    fn new(channel_id: &str, callback: &Url, lease_duration: Duration) -> Self {
        Self {
            channel_id: channel_id.to_string(),
            callback: callback.to_string(),
            lease_duration,
            handle: {
                let channel_id = channel_id.to_string();
                let callback = callback.to_string();
                ScopedJoinHandle(tokio::spawn(async move {
                    let mut interval = tokio::time::interval(lease_duration / 2);
                    loop {
                        interval.tick().await;
                        if let Err(e) =
                            register(&channel_id, &callback, lease_duration, Mode::Subscribe).await
                        {
                            error!(?channel_id, "failed to register channel: {}", e);
                        }
                    }
                }))
            },
        }
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        let channel_id = mem::take(&mut self.channel_id);
        let callback = mem::take(&mut self.callback);
        let lease_duration = self.lease_duration;
        tokio::spawn(async move {
            if let Err(e) =
                register(&channel_id, &callback, lease_duration, Mode::Unsubscribe).await
            {
                error!(?channel_id, "failed to unregister channel: {}", e);
            }
        });
    }
}

async fn register(id: &str, callback: &str, lease_duration: Duration, mode: Mode) -> Result<()> {
    drop(
        HTTP.post("https://pubsubhubbub.appspot.com/subscribe")
            .form(&SubscribeForm {
                callback: callback.to_string(),
                mode,
                topic: format!(
                    "https://www.youtube.com/xml/feeds/videos.xml?channel_id={}",
                    id
                ),
                verify: Verify::Async,
                lease_seconds: lease_duration.as_secs(),
            })
            .send()
            .await?
            .error_for_status()?,
    );
    Ok(())
}
