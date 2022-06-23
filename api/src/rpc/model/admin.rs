use mongodb::bson::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Admin {
    id: Uuid,
    bots: Vec<Uuid>,
}
