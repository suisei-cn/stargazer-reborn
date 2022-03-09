use sg_core::models::User;

crate::methods! {
    "getUser" :=
    GetUser {
        user_id: String
    } -> User,
    "getUserSettings" :=
    GetUserSettings {
        user_id: String
    } -> UserSettings {}
}
