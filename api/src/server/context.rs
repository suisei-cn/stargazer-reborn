//! Context of the server. Contains the configuration and database handle.
use std::collections::HashSet;
use std::sync::Arc;

use color_eyre::Result;
use futures::future::try_join;
use futures::TryStreamExt;
use mongodb::{
    bson::{doc, to_document, Uuid},
    options::{FindOneAndUpdateOptions, ReturnDocument},
    Client, Collection, Database,
};
use url::Url;

use sg_auth::AuthClient;
use sg_core::models::{Entity, EventFilter, Group, Meta, Task, User};

use crate::model::{Entities, GetEntities};
use crate::{
    model::{AddTaskParam, Bot, UserQuery},
    rpc::{ApiError, ApiResult},
    server::{config::Config, Claims, JWTContext, Privilege},
};

/// Context being shared between handlers. This will be cloned every time a handler is called.
/// So all underlying data should be wrapped in Arc or similar shared reference thingy.
///
/// Since this is intended to be cloned everytime, `Option<Claims>` will be reset upon every request.
#[must_use]
#[derive(Clone)]
pub struct Context {
    /// Config.
    config: Arc<Config>,
    /// JWT
    jwt: Arc<JWTContext>,
    /// DB instance. Since DB is composed of [`Collection`](mongodb::Collection)s, cloning is cheap.
    db: Database,
    /// Auth context.
    auth: AuthClient,
    /// Claims that are extracted from the JWT token header by auth middleware, optionally.
    claims: Option<Claims>,
}

/// Context of the server. Contains the configuration and database handle.
impl Context {
    /// # Errors
    /// Fail on invalid database url.
    pub async fn new(jwt: Arc<JWTContext>, config: Arc<Config>) -> Result<Self> {
        let client = Client::with_uri_str(&config.mongo_uri).await?;
        let db = client.database(&config.mongo_db);

        Ok(Self::new_with_db(db, jwt, config))
    }

    #[inline]
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Construct self with preconnected database.
    #[inline]
    pub fn new_with_db(db: Database, jwt: Arc<JWTContext>, config: Arc<Config>) -> Self {
        let auth = AuthClient::new(db.collection(&config.auth_collection));
        Self {
            db,
            jwt,
            auth,
            config,
            claims: None,
        }
    }

    /// Get the claims from the JWT token header and assert its validity as an user. Admin and bots are not allowed.
    /// Only use this if trying to get user information from the token.
    ///
    /// # Errors
    /// Fails if the token is not present, or the token is not issued for a subscriber.
    pub fn assert_user_claims(&self) -> ApiResult<&Claims> {
        self.claims
            .as_ref()
            .ok_or_else(ApiError::unauthorized)
            .and_then(|c| {
                if c.as_bytes() == &[0; 16] {
                    Err(ApiError::unauthorized())
                } else {
                    Ok(c)
                }
            })
    }

    /// Get the claims from the JWT token header.
    #[inline]
    #[must_use]
    pub const fn claims(&self) -> Option<&Claims> {
        self.claims.as_ref()
    }

    /// Insert claims, if there's already one, return it
    #[inline]
    pub fn set_claims(&mut self, claims: Claims) -> Option<Claims> {
        self.claims.replace(claims)
    }

    /// Encode the user id and corresponding privilege into a JWT token.
    ///
    /// # Errors
    /// Fails when encoding failed. This is unlikely to happen, but if it does, it's a bug.
    #[inline]
    pub fn encode(&self, user_id: &Uuid, privilege: Privilege) -> ApiResult<(String, Claims)> {
        self.jwt.encode(user_id, privilege).map_err(|detail| {
            tracing::error!(?detail, "Failed to encode JWT token");
            ApiError::internal()
        })
    }

    #[inline]
    #[must_use]
    pub fn users(&self) -> Collection<User> {
        self.db.collection(&self.config.users_collection)
    }

    #[inline]
    #[must_use]
    pub fn tasks(&self) -> Collection<Task> {
        self.db.collection(&self.config.tasks_collection)
    }

    #[inline]
    #[must_use]
    pub fn entities(&self) -> Collection<Entity> {
        self.db.collection(&self.config.entities_collection)
    }

    #[inline]
    #[must_use]
    pub fn groups(&self) -> Collection<Group> {
        self.db.collection(&self.config.groups_collection)
    }

    #[inline]
    #[must_use]
    pub fn auth_db(&self) -> Collection<Bot> {
        self.db.collection(&self.config.auth_collection)
    }

    #[inline]
    #[must_use]
    pub const fn auth(&self) -> &AuthClient {
        &self.auth
    }

    /// # Errors
    /// Fail on database error or user not found
    pub async fn find_user(&self, query: &UserQuery) -> ApiResult<Option<User>> {
        self.users()
            .find_one(query.as_document(), None)
            .await
            .map_err(Into::into)
    }

    pub async fn add_user(
        &self,
        im: String,
        im_payload: String,
        avatar: Option<Url>,
        name: String,
    ) -> ApiResult<User> {
        if self
            .find_user(&UserQuery::ByIm {
                im: im.clone(),
                im_payload: im_payload.clone(),
            })
            .await?
            .is_some()
        {
            return Err(ApiError::user_already_exists(&im, &im_payload));
        };

        let user = User {
            im,
            im_payload,
            avatar,
            name,
            event_filter: EventFilter {
                entities: HashSet::default(),
                kinds: HashSet::default(),
            },
            id: Uuid::default(),
        };

        self.users().insert_one(&user, None).await?;
        Ok(user)
    }

    /// # Errors
    /// Fail on database error or user not found
    pub async fn del_user(&self, query: &UserQuery) -> ApiResult<User> {
        self.users()
            .find_one_and_delete(query.as_document(), None)
            .await?
            .ok_or_else(|| query.as_error())
    }

    /// # Errors
    /// Fail on database error or user not found
    pub async fn update_setting(&self, id: &Uuid, event_filter: &EventFilter) -> ApiResult<User> {
        let serialized = to_document(&event_filter)?;

        self.users()
            .find_one_and_update(
                doc! { "id": id },
                doc! { "$set": { "event_filter": serialized } },
                FindOneAndUpdateOptions::builder()
                    .return_document(ReturnDocument::After)
                    .build(),
            )
            .await?
            .ok_or_else(|| ApiError::user_not_found_with_id(id))
    }

    pub async fn add_entity(&self, meta: Meta, tasks: Vec<AddTaskParam>) -> ApiResult<Entity> {
        let mut ent = Entity {
            id: Uuid::new(),
            meta,
            tasks: vec![],
        };

        self.entities().insert_one(&ent, None).await?;

        ent.tasks = self
            .add_tasks(&ent.id, tasks.into_iter())
            .await?
            .into_iter()
            .map(|x| x.id)
            .collect();

        Ok(ent)
    }

    /// # Errors
    /// Fail on database error or entity not found
    pub async fn find_entity(&self, id: &Uuid) -> ApiResult<Entity> {
        self.entities()
            .find_one(doc! { "id": id }, None)
            .await?
            .ok_or_else(|| ApiError::entity_not_found(id))
    }

    /// # Errors
    /// Fail on database error, entity not found or failed to serialize meta
    pub async fn update_entity(&self, id: &Uuid, meta: &Meta) -> ApiResult<Entity> {
        self.entities()
            .find_one_and_update(
                doc! { "id": id },
                doc! { "meta": to_document(meta)? },
                FindOneAndUpdateOptions::builder()
                    .return_document(ReturnDocument::After)
                    .build(),
            )
            .await?
            .ok_or_else(|| ApiError::entity_not_found(id))
    }

    pub async fn del_entity(&self, id: &Uuid) -> ApiResult<Entity> {
        // Get the entity, make sure it exists and get all related tasks
        let entity = self
            .entities()
            .find_one_and_delete(doc! { "id": id }, None)
            .await?
            .ok_or_else(|| ApiError::entity_not_found(&id))?;

        // Delete all related tasks
        self.tasks()
            .delete_many(doc! { "id": { "$in": &entity.tasks } }, None)
            .await?;

        Ok(entity)
    }

    pub async fn get_entities(&self) -> ApiResult<Entities> {
        let (vtbs, groups) = try_join(
            async { self.entities().find(None, None).await?.try_collect().await },
            async { self.groups().find(None, None).await?.try_collect().await },
        )
        .await?;

        Ok(Entities { vtbs, groups })
    }

    /// # Errors
    /// Fail on database error or task not found
    pub async fn add_task(&self, entity_id: &Uuid, task: Task) -> ApiResult<Task> {
        if self
            .entities()
            .update_one(
                doc! { "id": entity_id },
                doc! { "$push": { "tasks": task.id } },
                None,
            )
            .await?
            .modified_count
            == 0
        {
            Err(ApiError::entity_not_found(entity_id))
        } else {
            self.tasks().insert_one(&task, None).await?;
            Ok(task)
        }
    }

    /// # Errors
    /// Fail on database error
    pub async fn add_tasks(
        &self,
        entity_id: &Uuid,
        tasks: impl Iterator<Item = AddTaskParam> + Send,
    ) -> ApiResult<Vec<Task>> {
        let tasks = tasks
            .map(|x| x.into_task_with(*entity_id))
            .collect::<Vec<_>>();

        self.tasks().insert_many(&tasks, None).await?;
        Ok(tasks)
    }

    /// # Errors
    /// Fail on database error or task not found
    pub async fn del_task(&self, task_id: &Uuid) -> ApiResult<Task> {
        // Make sure this exists
        let task = self
            .tasks()
            .find_one_and_delete(doc! { "id": task_id }, None)
            .await?
            .ok_or_else(|| ApiError::task_not_found(task_id))?;

        // Delete the task from the entity that holds it
        self.entities()
            .update_one(
                doc! { "id": task.entity },
                doc! { "tasks": { "$pull": task_id } },
                None,
            )
            .await?;

        Ok(task)
    }

    pub async fn get_interest(
        &self,
        entity_id: Uuid,
        kind: &str,
        im: &str,
    ) -> ApiResult<Vec<User>> {
        Ok(self
            .users()
            .find(
                doc! {
                  "event_filter.entities": entity_id,
                  "event_filter.kinds": kind,
                  "im": im,
                },
                None,
            )
            .await?
            .try_collect()
            .await?)
    }

    /// # Errors
    /// Fail on bad token, database error, the uuid is "nil" or user not exist.
    ///
    /// An uuid is "nil" if all bytes are set to 0, which at here represents that
    /// the token is issued to an admin or bot, who does not represent a subscriber.
    pub async fn find_and_assert_claim(&self) -> ApiResult<Option<User>> {
        let user_id = self.assert_user_claims()?.id();
        if user_id.bytes() == [0; 16] {
            return Err(ApiError::unauthorized());
        }
        self.find_user(&UserQuery::ById { user_id }).await
    }
}
