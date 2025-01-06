use chrono::NaiveDateTime;
use serde::Deserialize;
use serde::Serialize;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::{Client, NoTls};
use uuid::Uuid;

use crate::config::settings::PostgresConfig;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct UserRow {
    pub user_id: Uuid,
    pub trial: bool,
    pub password: String,
    pub cluster: String,
    pub created: NaiveDateTime,
}

pub async fn postgres_client(
    pg_settings: PostgresConfig,
) -> Result<Arc<Mutex<Client>>, Box<dyn Error>> {
    let connection_line = format!(
        "host={} user={} dbname={} password={} port={}",
        pg_settings.host,
        pg_settings.username,
        pg_settings.db,
        pg_settings.password,
        pg_settings.port
    );

    let (client, connection) = tokio_postgres::connect(&connection_line, NoTls).await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Error in PostgreSQL connection: {}", e);
        }
    });

    Ok(Arc::new(Mutex::new(client)))
}

pub async fn users_db_request(
    client: Arc<Mutex<Client>>,
    cluster: String,
) -> Result<Vec<UserRow>, Box<dyn Error>> {
    let client = client.lock().await;

    let rows = client
        .query(
            "SELECT id, trial, password, cluster, created  FROM users where cluster = $1",
            &[&cluster],
        )
        .await?;

    let users: Vec<UserRow> = rows
        .into_iter()
        .map(|row| {
            let user_id: Uuid = row.get(0);
            let trial: bool = row.get(1);
            let password: String = row.get(2);
            let cluster: String = row.get(3);
            let created: NaiveDateTime = row.get(4);

            UserRow {
                user_id,
                trial,
                password,
                cluster,
                created,
            }
        })
        .collect();

    Ok(users)
}
