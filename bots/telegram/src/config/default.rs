use reqwest::Url;

pub fn amqp_url() -> String {
    "amqp://guest:guest@localhost:5672".to_owned()
}

pub fn amqp_exchange() -> String {
    String::from("stargazer-reborn")
}

pub fn api_url() -> Url {
    Url::parse("http://127.0.0.1:8000/v1/").unwrap()
}
