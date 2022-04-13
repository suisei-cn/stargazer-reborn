# RPC

All RPC methods are can be found in the `/api/src/rpc/model/mod.rs` file. They are defined by the `methods` macro.
This macro does several things, including generating the request struct, derive serde traits, implement `Request` trait, add a `new` function
for it, generate a response struct if necessary, derive serde traits, implement `Response` trait, add a `new` function for it, and add that
function to both blocking and non-blocking client.

## Traits

Two main traits are defined:

- `Request`
- `Response`

### Request

Used to define a request object which represents a request body sent from client to server.
To invoke the method, send a `POST` http request to `/v1/:method_name` with request param serialized into `JSON` as the body.

A `Request` is always bind with a `Response` type.
Handler for this request will return the corresponding `Response` object,
or an `ApiError` object represent an error during handling the request.

### Response

Used to define a response payload sent from server to client.
All response should be wrapped in `ResponseObject`, which includes extra information about the response,
e.g. time it's being processed and whether it's successful.

To construct a `ResponseObject`, method `Response::packed` should be used.
It's automatically implemented by `Response`.
