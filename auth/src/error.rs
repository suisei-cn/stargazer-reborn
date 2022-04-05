use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("MongoDB error: {0}")]
    Mongo(#[from] mongodb::error::Error),

    #[error("BSON serialize error: {0}")]
    Bson(#[from] mongodb::bson::ser::Error),

    #[error("Argon error: {0}")]
    Pbkdf2(#[from] argon2::password_hash::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
