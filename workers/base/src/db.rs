//! Database access.

use std::collections::HashMap;

use eyre::Result;
use futures::{future, stream, Stream, StreamExt, TryStreamExt};
use mongodb::bson::oid::ObjectId;
use mongodb::change_stream::event::{ChangeStreamEvent, OperationType};
use mongodb::options::{ChangeStreamOptions, FullDocumentType};
use mongodb::{bson, Client, Collection};
use tracing::{error, info, info_span, instrument};
use tracing_futures::Instrument;
use uuid::Uuid;

use sg_core::models::{InDB, Task};

use crate::worker::Event;

type TaskCollection = Collection<InDB<Task>>;

#[instrument]
async fn populate_existing_tasks(
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

type ChangeEvent = ChangeStreamEvent<InDB<Task>>;

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

            if let Some(id) = oid_map.remove(&task.id()) {
                info!(task_id = %id, "Task removed");

                vec![Event::TaskRemove(id)]
            } else {
                error!("Task not found in oid map: {:?}.", task.id());
                vec![]
            }
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

pub async fn db_events(
    uri: &str,
    db: &str,
    coll: &str,
) -> Result<impl Stream<Item = Result<Event>>> {
    let client = Client::with_uri_str(uri).await?;
    let db = client.database(db);
    let collection = db.collection(coll);

    info!("Loading existing tasks from database");
    let (mut oid_map, initial_tasks) = populate_existing_tasks(&collection).await?;

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
