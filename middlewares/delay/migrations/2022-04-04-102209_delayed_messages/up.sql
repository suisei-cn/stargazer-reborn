-- Your SQL goes here
CREATE TABLE delayed_messages
(
    id          BIGINT PRIMARY KEY NOT NULL ON CONFLICT REPLACE,
    middlewares TEXT               NOT NULL,
    body        TEXT               NOT NULL,
    created_at  TIMESTAMP          NOT NULL,
    deliver_at  TIMESTAMP          NOT NULL
)