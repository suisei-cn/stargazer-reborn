crate::methods! {
    "getUser" :=
    GetUser {
        user_id: String
    } -> User {
        user_id: String,
        user_info: String
    },
    "getUserSettings" :=
    GetUserSettings {
        user_id: String
    } -> UserSettings {}
}
