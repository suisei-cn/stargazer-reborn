# Configuration

All configuration is done by the environment variables, with each executable using their own unique "namespace", as prefix of env variables. For example, the [`api`](./api.md) module uses the `API_` prefix. Don't forget to append the prefix before each variable.

## Api (server)

**Prefix**: `API_`

**Definition**: `/api/src/server/config.rs`

| Variable              | Type         | Default                   | Description                                                                                       |
| --------------------- | ------------ | ------------------------- | ------------------------------------------------------------------------------------------------- |
| `BIND`                | `SocketAddr` | 127.0.0.1:8000            | Bind address for API server.                                                                      |
| `TOKEN_TIMEOUT`       | `Duration`   | 600 Seconds               | Duration the session(token) is valid.                                                             |
| `MONGO_URI`           | `String`     | mongodb://localhost:27017 | MongoDB connection string.                                                                        |
| `MONGO_DB`            | `String`     | stargazer-reborn          | MongoDB database name.                                                                            |
| `BOT_PASSWORD`        | `String`     | TEST                      | Secret password used to authenticate API requests from bot. This is also used to sign JWT tokens. |
| `USERS_COLLECTION`    | `String`     | users                     | MongoDB collection name for `Users`.                                                              |
| `TASKS_COLLECTION`    | `String`     | tasks                     | MongoDB collection name for `Tasks`.                                                              |
| `ENTITIES_COLLECTION` | `String`     | entities                  | MongoDB collection name for `VTBs`.                                                               |
| `GROUPS_COLLECTION`   | `String`     | groups                    | MongoDB collection name for `Groups`.                                                             |
| `AUTH_COLLECTION`     | `String`     | auth                      | MongoDB collection name for `Auth`.                                                               |

## Coordinator

**Prefix**: `COORDINATOR_`

**Definition**: `/coordinator/src/config.rs`

| Variable           | Type         | Default                   | Description                                            |
| ------------------ | ------------ | ------------------------- | ------------------------------------------------------ |
| `BIND`             | `SocketAddr` | 127.0.0.1:7000            | Bind address for coordinator.                          |
| `PING_INTERVAL`    | `Duration`   | 10 Seconds                | Determine how often coordinator sends ping to workers. |
| `MONGO_URI`        | `String`     | mongodb://localhost:27017 | MongoDB connection string.                             |
| `MONGO_DB`         | `String`     | stargazer-reborn          | MongoDB database name.                                 |
| `MONGO_COLLECTION` | `String`     | tasks                     | MongoDB collection name for `Tasks`.                   |

## Middlewares

**Prefix**: `MIDDLEWARE_`

**Definition**: `/middleware/**/src/config.rs`

**Available middlewares**: `translate`, `delay`

| Variable           | Type     | Default                           | Middleware  | Description                 |
| ------------------ | -------- | --------------------------------- | ----------- | --------------------------- |
| `AMQP_URL`         | `String` | amqp://guest:guest@localhost:5672 |             | AMQP connection url.        |
| `AMQP_EXCHANGE`    | `String` | stargazer-reborn                  |             | AMQP exchange name.         |
| `DATABASE_URL`     | `String` |                                   | `delay`     | Database connection url.    |
| `BAIDU_APP_ID`     | `usize`  |                                   | `translate` | Baidu translate app id.     |
| `BAIDU_APP_SECRET` | `String` |                                   | `translate` | Baidu translate app secret. |
| `DEBUG`            | `bool`   | false                             | `translate` | Debug only.                 |

## Workers

**Prefix**: `WORKER_`

**Definition**: `/workers/**/src/config.rs`

**Available workers**: `bililive`, `twitter`

| Variable          | Type       | Default                           | Worker    | Description                        |
| ----------------- | ---------- | --------------------------------- | --------- | ---------------------------------- |
| `ID`              | `Uuid`     |                                   |           | Unique worker ID.                  |
| `AMQP_URL`        | `String`   | amqp://guest:guest@localhost:5672 |           | AMQP connection url.               |
| `AMQP_EXCHANGE`   | `String`   | stargazer-reborn                  |           | AMQP exchange name.                |
| `COORDINATOR_URL` | `String`   | ws://127.0.0.1:7000               |           | The coordinator url to connect to. |
| `POLL_INTERVAL`   | `Duration` | 60 Second                         | `twitter` | Interval between twitter polls.    |
| `TWITTER_TOKEN`   | `String`   |                                   | `twitter` | Twitter API token.                 |

## Bots

**Prefix**: `BOT_`

**Definition**: `/bots/**/src/config.rs`

**Available bots**: `telegram`

| Variable        | Type     | Default                           | Bot        | Description          |
| --------------- | -------- | --------------------------------- | ---------- | -------------------- |
| `AMQP_URL`      | `String` | amqp://guest:guest@localhost:5672 |            | AMQP connection url. |
| `AMQP_EXCHANGE` | `String` | stargazer-reborn                  |            | AMQP exchange name.  |
| `API_URL`       | `Url`    | http://127.0.0.1:8000/v1/         |            | Api url.             |
| `API_USERNAME`  | `String` |                                   |            | Api username.        |
| `API_PASSWORD`  | `String` |                                   |            | Api password.        |
| `TG_TOKEN`      | `String` |                                   | `telegram` | Telegram bot token.  |
