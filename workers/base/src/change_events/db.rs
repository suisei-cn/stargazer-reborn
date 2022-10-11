//! Database provider.
use std::collections::HashMap;

use eyre::Result;
use futures::{future, stream, Stream, StreamExt, TryStreamExt};
use mongodb::{
    bson,
    bson::oid::ObjectId,
    change_stream::event::{ChangeStreamEvent, OperationType},
    options::{ChangeStreamOptions, FullDocumentType},
    Client,
    Collection,
};
use sg_core::models::{InDB, Task};
use tracing::{error, info, info_span, instrument};
use tracing_futures::Instrument;
use uuid::Uuid;

use crate::common::Event;

type TaskCollection = Collection<InDB<Task>>;
type ChangeEvent = ChangeStreamEvent<InDB<Task>>;

/// Load existing tasks from the database.
#[instrument]
async fn fetch_existing_tasks(
    coll: &TaskCollection,
) -> Result<(HashMap<ObjectId, Uuid>, Vec<Task>)> {
    let tasks = coll.find(None, None).await?;

    let (oid_map, tasks, count) = tasks
        .try_fold(
            (HashMap::new(), Vec::new(), 0usize),
            |(mut oid_map, mut tasks, count), task| async move {
                oid_map.insert(task.id(), task.id.into());
                tasks.push(task.inner());
                Ok((oid_map, tasks, count + 1))
            },
        )
        .await?;

    info!("{} task(s) loaded from database", count);
    Ok((oid_map, tasks))
}

/// Convert db change events to task events.
#[allow(clippy::cognitive_complexity)]
fn match_event(event: ChangeEvent, oid_map: &mut HashMap<ObjectId, Uuid>) -> Vec<Event> {
    match event.operation_type {
        OperationType::Insert => {
            let task = event
                .full_document
                .expect("Full document must be available");

            info!(task_id = %task.id, "Task added");

            oid_map.insert(task.id(), task.id.into());
            vec![Event::TaskAdd(task.inner())]
        }
        OperationType::Update => {
            let task = event
                .full_document
                .expect("Full document must be available");

            info!(task_id = %task.id, "Task updated");

            vec![
                Event::TaskRemove(task.id.into()),
                Event::TaskAdd(task.inner()),
            ]
        }
        OperationType::Replace => {
            let task = event
                .full_document
                .expect("Full document must be available");

            info!(task_id = %task.id, "Task updated");

            vec![
                Event::TaskRemove(task.id.into()),
                Event::TaskAdd(task.inner()),
            ]
        }
        OperationType::Delete => {
            let task: InDB<()> =
                bson::from_document(event.document_key.expect("DocumentKey must be available"))
                    .expect("_id must be available");

            oid_map.remove(&task.id()).map_or_else(
                || {
                    error!("Task not found in oid map: {:?}.", task.id());
                    vec![]
                },
                |id| {
                    info!(task_id = %id, "Task removed");
                    vec![Event::TaskRemove(id)]
                },
            )
        }
        OperationType::Invalidate => {
            error!("Change stream invalidated.");
            vec![]
        }
        ty => {
            error!("Unexpected event type: {:?}", ty);
            vec![]
        }
    }
}

/// Change stream from database.
///
/// Provides tasks changes.
pub async fn db_events(
    uri: &str,
    db: &str,
    coll: &str,
) -> Result<impl Stream<Item = Result<Event>>> {
    let client = Client::with_uri_str(uri).await?;
    let db = client.database(db);
    let collection = db.collection(coll);

    info!("Loading existing tasks from database");
    let (mut oid_map, initial_tasks) = fetch_existing_tasks(&collection).await?;

    info!("Start watching database for task changes");
    let stream = collection
        .watch(
            None,
            ChangeStreamOptions::builder()
                .full_document(Some(FullDocumentType::UpdateLookup))
                .build(),
        )
        .await?;
    let changes = stream
        .map_ok(move |event| match_event(event, &mut oid_map))
        .flat_map(|try_event| match try_event {
            Ok(events) => stream::iter(events).map(Ok).boxed(),
            Err(e) => stream::once(future::ready(Err(e.into()))).boxed(),
        })
        .instrument(info_span!("change_stream"));

    Ok(stream::iter(
        initial_tasks
            .into_iter()
            .map(|task| Ok(Event::TaskAdd(task))),
    )
    .chain(changes))
}
