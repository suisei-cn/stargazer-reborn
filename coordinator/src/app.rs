//! Application state.
use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::result::Result as StdResult;
use std::sync::Arc;

use eyre::Result;
use parking_lot::Mutex;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::HeaderMap;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::worker::{Worker, WorkerGroup};

/// The application state.
#[derive(Debug, Clone, Default)]
pub struct App(Arc<AppImpl>);

impl App {
    /// Serve the application.
    ///
    /// # Errors
    /// Return error if failed to bind to the given address.
    pub async fn serve(&self, addr: SocketAddr) -> Result<()> {
        let socket = TcpListener::bind(&addr).await?;
        info!("Listening on {}", addr);
        loop {
            if let Ok((socket, addr)) = socket.accept().await {
                info!(addr = %addr, "Accepting connection");
                let this = self.0.clone();
                tokio::spawn(async move {
                    if let Err(e) = this.accept_connection(socket).await {
                        error!(addr = %addr, "Failed to accept websocket connection: {}", e);
                    }
                });
            }
        }
    }
}

/// Implementation of the application state.
#[derive(Debug, Default)]
pub struct AppImpl {
    worker_groups: Mutex<HashMap<String, WorkerGroup>>,
}

struct WorkerMeta {
    id: Uuid,
    ty: String,
}

impl TryFrom<&HeaderMap> for WorkerMeta {
    type Error = Box<dyn Error>;

    fn try_from(headers: &HeaderMap) -> StdResult<Self, Box<dyn Error>> {
        let id = Uuid::from_slice(
            headers
                .get("sg-worker-id")
                .ok_or("missing header: sg-worker-id")?
                .as_bytes(),
        )?;
        let ty = headers
            .get("sg-worker-ty")
            .ok_or("missing header: sg-worker-ty")?
            .to_str()?
            .to_string();
        Ok(Self { id, ty })
    }
}

impl AppImpl {
    /// Accept a new worker.
    ///
    /// # Errors
    /// Forward error if failed to accept websocket connection.
    ///
    /// # Panics
    /// Panic if internal state is poisoned.
    pub async fn accept_connection(&self, socket: TcpStream) -> Result<()> {
        // Accept stream and extract metadata from HTTP headers.
        let (worker_meta, stream) = {
            let mut worker_meta = None;
            let stream = tokio_tungstenite::accept_hdr_async(
                socket,
                |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
                    worker_meta = Some(
                        WorkerMeta::try_from(req.headers())
                            .map_err(|e| ErrorResponse::new(Some(e.to_string())))?,
                    );
                    Ok(resp)
                },
            )
            .await?;
            (worker_meta.unwrap(), stream)
        };

        debug!(worker_id = %worker_meta.id, "Worker accepted");

        // Spawn worker and add worker to a worker group.
        let mut worker_groups = self.worker_groups.lock();
        let worker_group = worker_groups
            .entry(worker_meta.ty)
            .or_insert_with(WorkerGroup::new);
        let worker = Worker::new(worker_meta.id, stream, worker_group.weak());
        worker_group.with(|worker_group| worker_group.add_worker(worker));

        Ok(())
    }
}
