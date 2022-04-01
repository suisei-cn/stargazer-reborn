use mongodb::bson::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bot {
    /// UUID of the bot
    id: Uuid,
    /// UUID of the admin who created the bot
    created_by: Uuid,
}
