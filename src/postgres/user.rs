use chrono::NaiveDateTime;
use serde::Deserialize;
use serde::Serialize;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client;
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct UserRow {
    pub user_id: Uuid,
    pub trial: bool,
    pub password: String,
    pub cluster: String,
    pub created: NaiveDateTime,
}

pub async fn users_db_request(
    client: Arc<Mutex<Client>>,
    cluster: Option<String>,
) -> Result<Vec<UserRow>, Box<dyn Error>> {
    let client = client.lock().await;

    let query = if let Some(_) = cluster {
        "
        SELECT id, trial, password, cluster, created 
        FROM users 
        WHERE cluster = $1
        "
    } else {
        "
        SELECT id, trial, password, cluster, created 
        FROM users
        "
    };

    let rows = if let Some(env) = cluster {
        client.query(query, &[&env]).await?
    } else {
        client.query(query, &[]).await?
    };

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
