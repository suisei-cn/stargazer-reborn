use std::{
    fmt::Display,
    time::{Duration, SystemTime},
};

use color_eyre::Result;
use humantime::FormattedDuration;
use sg_api::model::Token;
use url::Url;

use crate::use_config;

#[allow(clippy::module_name_repetitions)]
pub trait TokenExt {
    type FormattedValidUntil: Display;
    fn valid_until_formatted(&self) -> Result<Self::FormattedValidUntil>;

    fn as_url(&self) -> Url;
}

impl TokenExt for Token {
    type FormattedValidUntil = FormattedDuration;
    fn valid_until_formatted(&self) -> Result<Self::FormattedValidUntil> {
        let valid_for = self.valid_until.duration_since(SystemTime::now())?;
        Ok(humantime::format_duration(Duration::from_secs(
            valid_for.as_secs(),
        )))
    }

    fn as_url(&self) -> Url {
        let mut u = use_config().frontend_url.clone();
        u.set_query(Some(&format!("token={}", &self.token)));
        u
    }
}
