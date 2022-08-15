//! Application state.
use std::{
    collections::HashMap,
    error::Error,
    ops::Deref,
    result::Result as StdResult,
    str::FromStr,
    sync::Arc,
};

use eyre::Result;
use sg_core::models::Task;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tokio_tungstenite::tungstenite::{
    handshake::server::{ErrorResponse, Request, Response},
    http::{HeaderMap, StatusCode},
};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::{
    config::Config,
    worker::{Worker, WorkerGroup},
};

/// The application state.
#[derive(Debug, Clone)]
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
    pub async fn serve(self) -> Result<()> {
        info!("Listening on {}", self.config.bind);

        let socket = TcpListener::bind(self.config.bind).await?;
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

impl Deref for App {
    type Target = AppImpl;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Implementation of the application state.
#[derive(Debug)]
pub struct AppImpl {
    /// Worker groups.
    pub worker_groups: Mutex<HashMap<String, WorkerGroup>>,
    config: Config,
}

struct WorkerMeta {
    id: Uuid,
    kind: String,
}

impl TryFrom<&HeaderMap> for WorkerMeta {
    type Error = Box<dyn Error>;

    fn try_from(headers: &HeaderMap) -> StdResult<Self, Box<dyn Error>> {
        let id = Uuid::from_str(
            headers
                .get("Sg-Worker-ID")
                .ok_or("missing header: Sg-Worker-ID")?
                .to_str()?,
        )?;
        let kind = headers
            .get("Sg-Worker-Kind")
            .ok_or("missing header: Sg-Worker-Kind")?
            .to_str()?
            .to_string();
        Ok(Self { id, kind })
    }
}

impl AppImpl {
    /// Create a new application state impl.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            worker_groups: Default::default(),
            config,
        }
    }

    /// Add a task to worker group of its kind.
    pub async fn add_task(&self, task: Task) {
        self.worker_groups
            .lock()
            .await
            .entry(task.kind.clone())
            .or_insert_with(WorkerGroup::new)
            .with(|group| group.add_task(task))
            .await;
    }

    /// Remove a task from worker groups.
    pub async fn remove_task(&self, id: Uuid) {
        for group in self.worker_groups.lock().await.values_mut() {
            group.with(|group| group.remove_task(id)).await;
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
        let mut worker_groups = self.worker_groups.lock().await;
        let worker_group = worker_groups
            .entry(worker_meta.kind)
            .or_insert_with(WorkerGroup::new);
        let worker = Worker::new(worker_meta.id, stream, worker_group.weak(), &self.config);
        worker_group
            .with(|worker_group| worker_group.add_worker(worker))
            .await;

        Ok(())
    }
}
