use async_trait::async_trait;
use eyre::{ContextCompat, Result};
use reqwest::Client;
use serde_json::Value;
use sg_core::models::Event;
use tracing::warn;

#[async_trait]
pub trait Translator: Send + Sync {
    async fn translate_event(&self, mut event: Event) -> Result<Event> {
        let translate_fields: Vec<_> = event
            .fields
            .remove("x-translate-fields")
            .wrap_err("Missing `x-translate-fields`")?
            .as_array()
            .wrap_err("Not an array")?
            .iter()
            .map(|v| Ok(v.as_str().wrap_err("Not a string")?.to_string()))
            .collect::<Result<_>>()?;

        let mut fields = Value::Object(event.fields);
        for field in translate_fields {
            if let Some(Value::String(src)) = fields.pointer_mut(&field) {
                match self.translate_text(src).await {
                    Ok(t) => {
                        *src = t;
                    }
                    Err(error) => {
                        warn!(?error, %src, "Failed to translate text");
                    }
                }
            } else {
                warn!(?fields, %field, "Field not found in event");
            }
        }

        event.fields = match fields {
            Value::Object(o) => o,
            _ => unreachable!(),
        };
        Ok(event)
    }
    async fn translate_text(&self, text: &str) -> Result<String>;
}

pub struct BaiduTranslator {
    client: Client,
    app_id: usize,
    app_secret: String,
}

impl BaiduTranslator {
    pub fn new(app_id: usize, app_secret: String) -> Self {
        Self {
            client: Client::new(),
            app_id,
            app_secret,
        }
    }
}

#[async_trait]
impl Translator for BaiduTranslator {
    async fn translate_text(&self, text: &str) -> Result<String> {
        let salt: usize = rand::random();
        let pre_sign = format!("{}{}{}{}", self.app_id, text, salt, self.app_secret);
        let sign = format!("{:x}", md5::compute(pre_sign));
        let resp: Value = self
            .client
            .get("https://fanyi-api.baidu.com/api/trans/vip/translate")
            .query(&[("q", text), ("from", "auto"), ("to", "zh")])
            .query(&[("appid", self.app_id)])
            .query(&[("salt", salt)])
            .query(&[("sign", sign)])
            .query(&[("action", 1)])
            .send()
            .await?
            .json()
            .await?;
        Ok(resp
            .pointer("/trans_result/0/dst")
            .wrap_err("invalid response")?
            .as_str()
            .wrap_err("not a string")?
            .to_string())
    }
}

pub struct MockTranslator;

#[async_trait]
impl Translator for MockTranslator {
    async fn translate_text(&self, text: &str) -> Result<String> {
        Ok(format!("test{}", text))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sg_core::models::Event;
    use uuid::Uuid;

    use crate::translate::{BaiduTranslator, MockTranslator, Translator};

    #[tokio::test]
    async fn must_translate_fields() {
        let e = Event {
            id: Uuid::nil().into(),
            kind: "".to_string(),
            entity: Uuid::nil().into(),
            fields: json!({
                "a": "a",
                "b": ["b1", "b2"],
                "c": {
                    "cc": "d"
                },
                "x-translate-fields": ["/a", "/b/0", "/c/cc"]
            })
            .as_object()
            .unwrap()
            .clone(),
        };
        let translator = MockTranslator;
        let translated = translator.translate_event(e).await.unwrap();
        assert_eq!(
            translated,
            Event {
                id: Uuid::nil().into(),
                kind: "".to_string(),
                entity: Uuid::nil().into(),
                fields: json!({
                    "a": "testa",
                    "b": ["testb1", "b2"],
                    "c": {
                        "cc": "testd"
                    }
                })
                .as_object()
                .unwrap()
                .clone(),
            }
        );
    }

    #[tokio::test]
    async fn test_baidu_translate() {
        if let (Some(app_id), Some(app_secret)) = (
            option_env!("TEST_BAIDU_APP_ID"),
            option_env!("TEST_BAIDU_APP_SECRET"),
        ) {
            let translator = BaiduTranslator::new(app_id.parse().unwrap(), app_secret.to_string());
            let translated = translator
                .translate_text("Apples are good for our health.")
                .await
                .unwrap();
            assert!(!translated.is_empty());
        }
    }

    #[tokio::test]
    async fn test_baidu_translate_custom_dict() {
        if let (Some(app_id), Some(app_secret)) = (
            option_env!("TEST_BAIDU_APP_ID"),
            option_env!("TEST_BAIDU_APP_SECRET"),
        ) {
            let translator = BaiduTranslator::new(app_id.parse().unwrap(), app_secret.to_string());
            let translated = translator
                .translate_text(
                    "Hoshimachi Suisei is a Japanese virtual YouTuber. She began posting videos \
                     as an independent creator in March 2018.",
                )
                .await
                .unwrap();
            assert!(translated.contains("星街彗星"));
        }
    }
}
