//! Worker trait and manager logic.

use std::collections::HashMap;

use eyre::Result;
use foca::Notification;
use futures::StreamExt;
use futures::{pin_mut, stream, Stream, TryStreamExt};
use tokio::net::{TcpListener, ToSocketAddrs};
use tokio_stream::wrappers::BroadcastStream;
use tokio_tungstenite::tungstenite::http::Uri;
use tracing::{debug, error};
use uuid::Uuid;

use sg_core::models::Task;

use crate::db::db_events;
use crate::resolver::StdResolver;
use crate::ring::{Migrated, Ring};
use crate::runtime::{start_foca, TokioFocaCtl};
use crate::transport::ws_transport;
use crate::{Certificates, ID};

pub enum Event {
    NodeUp(Uri),
    NodeDown(Uri),
    TaskAdd(Task),
    TaskRemove(Uuid),
}

pub trait Worker {
    // TODO &self or &mut self?
    fn add_task(&self, task: Task) -> bool;
    fn remove_task(&self, id: Uuid) -> bool;
}

trait WorkerLogExt {
    fn add_task_logged(&self, task: Task);
    fn remove_task_logged(&self, id: Uuid);
}

impl<W: Worker> WorkerLogExt for W {
    fn add_task_logged(&self, task: Task) {
        let task_id = task.id;
        if self.add_task(task) {
            debug!(%task_id, "Task added.");
        } else {
            error!(%task_id, "Task already exists.");
        }
    }

    fn remove_task_logged(&self, id: Uuid) {
        if self.remove_task(id) {
            debug!(task_id = %id, "Task removed.");
        } else {
            error!(task_id = %id, "Task does not exist.");
        }
    }
}

pub struct NodeConfig<A> {
    announce: Option<Uri>,
    bind: A,
    base_uri: Uri,
    certificates: Certificates,
    ident: ID,
    db: DBConfig,
}

pub struct DBConfig {
    uri: String,
    name: String,
    collection: String,
}

pub async fn foca_events(foca: &TokioFocaCtl) -> Result<impl Stream<Item = Result<Event>>> {
    let rx_foca = foca.recv().await;
    let nodes: Vec<_> = *foca
        .with(|foca| {
            foca.iter_members()
                .map(|member| member.addr().clone())
                .collect()
        })
        .await;
    Ok(
        stream::iter(nodes.into_iter().map(|node| Ok(Event::NodeUp(node)))).chain(
            BroadcastStream::new(rx_foca)
                .try_filter_map(|notification| async move {
                    Ok(match notification {
                        Notification::MemberUp(id) => Some(Event::NodeUp(id.addr().clone())),
                        Notification::MemberDown(id) => Some(Event::NodeDown(id.addr().clone())),
                        _ => None,
                    })
                })
                .map_err(|e| e.into()),
        ),
    )
}

pub async fn start_worker<A: ToSocketAddrs>(
    worker: impl Worker,
    config: NodeConfig<A>,
) -> Result<()> {
    let kind = config.ident.kind().to_string();
    let listener = TcpListener::bind(config.bind).await?;
    let (stream, sink) = ws_transport(
        listener,
        config.certificates,
        config.base_uri.clone(),
        StdResolver,
    )
    .await;
    let foca = start_foca(config.ident, stream, sink, None);
    if let Some(announce) = config.announce {
        foca.announce(ID::new(announce, kind));
    }

    let foca_stream = foca_events(&foca).await?;
    let db_stream = db_events(&config.db.uri, &config.db.name, &config.db.collection).await?;
    let event_stream = stream::select(foca_stream, db_stream);
    pin_mut!(event_stream);

    let this_node = config.base_uri;

    worker_task(worker, event_stream, this_node).await
}

async fn worker_task(
    worker: impl Worker,
    mut event_stream: impl Stream<Item = Result<Event>> + Unpin,
    this_node: Uri,
) -> Result<()> {
    let mut ring: Ring<Uri, Uuid> = Ring::default();
    let mut id_task_map: HashMap<Uuid, Task> = HashMap::new();
    if let Some(event) = event_stream.try_next().await? {
        match event {
            Event::NodeUp(node) => {
                if ring.is_empty() {
                    // Special case: add all existing tasks to the worker.
                    for task in id_task_map.values() {
                        worker.add_task_logged(task.clone());
                    }
                } else {
                    let migrations = ring.insert_node(node);
                    merge_migrations(&*migrations, &id_task_map, &this_node, &worker);
                }
            }
            Event::NodeDown(node) => {
                let migrations = ring.remove_node(&node);
                merge_migrations(&*migrations, &id_task_map, &this_node, &worker);
            }
            Event::TaskAdd(task) => {
                id_task_map.insert(task.id.into(), task.clone());
                if ring.insert_key(task.id.into()) == Some(&this_node) {
                    worker.add_task_logged(task);
                }
            }
            Event::TaskRemove(id) => {
                id_task_map.remove(&id);
                if ring.remove_key(&id) == Some(&this_node) {
                    worker.remove_task_logged(id);
                }
            }
        }
    }
    Ok(())
}

fn merge_migrations(
    migrations: &[Migrated<Uri, Uuid>],
    id_task_map: &HashMap<Uuid, Task>,
    this_node: &Uri,
    worker: &impl Worker,
) {
    migrations
        .iter()
        .find(|migration| migration.src() == this_node)
        .map(|removed| removed.keys())
        .into_iter()
        .flatten()
        .for_each(|task_to_remove| worker.remove_task_logged(*task_to_remove));

    migrations
        .iter()
        .find(|migration| migration.dst() == this_node)
        .map(|added| added.keys())
        .into_iter()
        .flatten()
        .map(|id| {
            id_task_map
                .get(id)
                .expect("INV: task must be in map")
                .clone()
        })
        .for_each(|task_to_add| worker.add_task_logged(task_to_add));
}
