//! Application state.
use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::result::Result as StdResult;
use std::str::FromStr;
use std::sync::Arc;

use eyre::Result;
use parking_lot::Mutex;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::{HeaderMap, StatusCode};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::config::Config;
use crate::worker::{Worker, WorkerGroup};

/// The application state.
#[derive(Debug, Clone, Default)]
pub struct App(Arc<AppImpl>);

impl App {
    /// Create a new application state.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self(Arc::new(AppImpl::new(config)))
    }
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
    #[must_use]
    pub fn inner(&self) -> &AppImpl {
        &self.0
    }
}

/// Implementation of the application state.
#[derive(Debug, Default)]
pub struct AppImpl {
    pub worker_groups: Mutex<HashMap<String, WorkerGroup>>,
    config: Config,
}

struct WorkerMeta {
    id: Uuid,
    ty: String,
}

impl TryFrom<&HeaderMap> for WorkerMeta {
    type Error = Box<dyn Error>;

    fn try_from(headers: &HeaderMap) -> StdResult<Self, Box<dyn Error>> {
        let id = Uuid::from_str(
            headers
                .get("sg-worker-id")
                .ok_or("missing header: sg-worker-id")?
                .to_str()?,
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
    /// Create a new application state impl.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }
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
                    worker_meta = Some(WorkerMeta::try_from(req.headers()).map_err(|e| {
                        error!("Invalid header: {}", e);
                        let mut resp = ErrorResponse::new(Some(e.to_string()));
                        *resp.status_mut() = StatusCode::BAD_REQUEST;
                        resp
                    })?);
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
        let worker = Worker::new(worker_meta.id, stream, worker_group.weak(), self.config);
        worker_group.with(|worker_group| worker_group.add_worker(worker));

        Ok(())
    }
}
