# Api

API is used to interact with the system, manage entities, tasks, user preferences, etc. It is defined in an RPC manner
with methods, which are functions map a request to a response. All methods have three properties:

- Method name, in `snake_case`, e.g., `get_entities`
- Request type, in `PascalCase`, e.g., `GetEntities`
- Response type, in `PascalCase`, e.g., `Entities`

Response can be either the defined value or an `ApiError` object, represented as `ApiResult` in rust.

Besides defined RPC, this module also implements a server driven by `Axum` and client that comes with both blocking and
non-blocking version, driven by `Reqwest`. To enable these, use `server`, `client` and/or `client-blocking` features.

```toml
# Cargo.toml

sg_api = { package = "api", path = "../api", features = ["server", "client", "client-blocking"] }
```
