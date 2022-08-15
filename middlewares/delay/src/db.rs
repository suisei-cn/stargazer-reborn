#![allow(clippy::extra_unused_lifetimes)]

use std::{fmt::Debug, io::Write};

use chrono::{NaiveDateTime, Utc};
use diesel::{
    backend::Backend,
    deserialize::FromSql,
    serialize::{Output, ToSql},
    sql_types,
    AsExpression,
    FromSqlRow,
    Insertable,
    Queryable,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sg_core::{models::Event, mq::Middlewares};

use crate::schema::delayed_messages;

#[derive(Debug, Clone, Queryable, Insertable)]
#[table_name = "delayed_messages"]
pub struct DelayedMessage {
    pub id: i64,
    pub middlewares: MiddlewaresWrapper,
    pub body: Json<Event>,
    pub created_at: NaiveDateTime,
    pub deliver_at: NaiveDateTime,
}

impl DelayedMessage {
    pub fn new(id: i64, middlewares: Middlewares, body: Event, deliver_at: NaiveDateTime) -> Self {
        Self {
            id,
            middlewares: MiddlewaresWrapper(middlewares),
            body: Json(body),
            created_at: Utc::now().naive_utc(),
            deliver_at,
        }
    }
}

#[derive(FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Clone)]
#[serde(transparent)]
#[sql_type = "sql_types::Text"]
pub struct Json<T: Sized>(pub T);

impl<T, DB> FromSql<sql_types::Text, DB> for Json<T>
where
    T: DeserializeOwned,
    DB: Backend,
    String: FromSql<sql_types::Text, DB>,
{
    fn from_sql(bytes: Option<&DB::RawValue>) -> diesel::deserialize::Result<Self> {
        let s = String::from_sql(bytes)?;
        Ok(Self(serde_json::from_str(&s)?))
    }
}

impl<T, DB> ToSql<sql_types::Text, DB> for Json<T>
where
    T: Serialize + Debug,
    DB: Backend,
    String: ToSql<sql_types::Text, DB>,
{
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> diesel::serialize::Result {
        let s = serde_json::to_string(&self.0)?;
        s.to_sql(out)
    }
}

#[derive(FromSqlRow, AsExpression, Debug, Clone)]
#[sql_type = "sql_types::Text"]
pub struct MiddlewaresWrapper(pub Middlewares);

impl<DB> FromSql<sql_types::Text, DB> for MiddlewaresWrapper
where
    DB: Backend,
    String: FromSql<sql_types::Text, DB>,
{
    fn from_sql(bytes: Option<&DB::RawValue>) -> diesel::deserialize::Result<Self> {
        let s = String::from_sql(bytes)?;
        Ok(Self(s.parse().unwrap()))
    }
}

impl<DB> ToSql<sql_types::Text, DB> for MiddlewaresWrapper
where
    DB: Backend,
    String: ToSql<sql_types::Text, DB>,
{
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> diesel::serialize::Result {
        let s = self.0.to_string();
        s.to_sql(out)
    }
}
