use chrono::NaiveDateTime;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client;
use uuid::Uuid;

use super::postgres::UserRow;

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
