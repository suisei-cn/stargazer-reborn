table! {
    delayed_messages (id) {
        id -> BigInt,
        middlewares -> Text,
        body -> Text,
        created_at -> Timestamp,
        deliver_at -> Timestamp,
    }
}
