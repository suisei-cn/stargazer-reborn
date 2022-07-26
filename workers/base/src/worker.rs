//! Worker trait and manager logic.

use std::collections::HashMap;

use eyre::Result;
use futures::{pin_mut, stream, Stream, TryStreamExt};
use tokio::net::{TcpListener, ToSocketAddrs};
use tokio_tungstenite::tungstenite::http::Uri;
use uuid::Uuid;

use crate::{
    change_events::{db::db_events, gossip::foca_events},
    common::{Event, Worker, WorkerLogExt},
    config::NodeConfig,
    gossip::{ident::ID, resolver::StdResolver, runtime::start_foca, transport::ws_transport},
    ring::{Migrated, Ring},
};

/// Start a new worker task.
///
/// # Errors
/// Returns error if failed to bind to the given address, or initial connection to database failed.
pub async fn start_worker<A: ToSocketAddrs + Send>(
    worker: impl Worker,
    config: NodeConfig<A>,
) -> Result<()> {
    // Bind to the configured address and start transport layer.
    let listener = TcpListener::bind(config.bind).await?;
    let (stream, sink) = ws_transport(
        listener,
        config.certificates,
        config.base_uri.clone(),
        StdResolver,
    )
    .await;

    // Start the Foca runtime.
    let kind = config.ident.kind().to_string();
    let foca = start_foca(config.ident, stream, sink, None);
    for announce_peer in config.announce {
        foca.announce(ID::new(announce_peer, kind.clone()));
    }

    // Prepare change stream.
    let foca_stream = foca_events(&foca).await;
    let db_stream = db_events(&config.db.uri, &config.db.db, &config.db.collection).await?;
    let event_stream = stream::select(foca_stream, db_stream);
    pin_mut!(event_stream);

    // Main loop.
    let this_node = config.base_uri;
    worker_task(worker, event_stream, this_node).await
}

/// Main worker task logic.
async fn worker_task(
    worker: impl Worker,
    mut event_stream: impl Stream<Item = Result<Event>> + Send + Unpin,
    this_node: Uri,
) -> Result<()> {
    // Prepare consistent hash ring.
    let mut ring: Ring<Uri, Uuid> = Ring::default();
    // Only IDs are stored in hash ring so we need to maintain an ID-to-Task
    // mapping.
    let mut id_task_map: HashMap<Uuid, Task> = HashMap::new();

    if let Some(event) = event_stream.try_next().await? {
        match event {
            Event::NodeUp(node) => {
                // A node has joined the cluster.
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
                // A node has left the cluster.
                let migrations = ring.remove_node(&node);
                merge_migrations(&*migrations, &id_task_map, &this_node, &worker);
            }
            Event::TaskAdd(task) => {
                // A new task has been added.
                id_task_map.insert(task.id.into(), task.clone());
                if ring.insert_key(task.id.into()) == Some(&this_node) {
                    // The added task is assigned to this node, add it to the worker.
                    worker.add_task_logged(task);
                }
            }
            Event::TaskRemove(id) => {
                // A task has been removed.
                id_task_map.remove(&id);
                if ring.remove_key(&id) == Some(&this_node) {
                    // The removed task belongs to this node, remove it from the worker.
                    worker.remove_task_logged(id);
                }
            }
        }
    }
    Ok(())
}

/// Merge related part of cluster member migrations into the worker.
fn merge_migrations(
    migrations: &[Migrated<Uri, Uuid>],
    id_task_map: &HashMap<Uuid, Task>,
    this_node: &Uri,
    worker: &impl Worker,
) {
    // Remove tasks that have been migrated from this node.
    migrations
        .iter()
        .find(|migration| migration.src() == this_node)
        .map(Migrated::keys)
        .into_iter()
        .flatten()
        .for_each(|task_to_remove| worker.remove_task_logged(*task_to_remove));

    // Add tasks that have been migrated to this node.
    migrations
        .iter()
        .find(|migration| migration.dst() == this_node)
        .map(Migrated::keys)
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
