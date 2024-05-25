pub mod models;
pub mod schema;
use diesel::mysql::MysqlConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use std::env;
use std::future::Future;

pub fn establish_connection() -> MysqlConnection {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    MysqlConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

pub async fn send_email(
    recipient: &str,
    subject: &str,
    body: &str,
) -> impl Future<Output = Result<String, reqwest::Error>> {
    let mut map = std::collections::HashMap::new();
    map.insert("to", recipient);
    map.insert("subject", subject);
    map.insert("content", body);
    map.insert(
        "auth",
        "ziVGVjaIriAADFTMHhintrIqB9qrkyR75tuocO27SeL4XldiIIBUtHoGUK45A1jm",
    );

    let client = reqwest::Client::new();
    let res = client
        .post("http://bridi.altervista.org/mail_api.php")
        .json(&map)
        .send()
        .await
        .unwrap();

    res.text()
}
