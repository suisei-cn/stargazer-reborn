use uuid::Uuid;

use crate::error::SerializedError;
use crate::StringError;

#[tarpc::service]
trait WorkerRpc {
    async fn ping(id: u64) -> u64;
}

#[tarpc::service]
trait CoordinatorRpc {
    async fn register(id: Uuid, ty: String) -> Result<(), SerializedError>;
}
