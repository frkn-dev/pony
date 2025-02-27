use chrono::NaiveDateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client;
use uuid::Uuid;

use crate::zmq::message::Action;
use crate::zmq::message::Message as UserRequest;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct UserRow {
    pub user_id: Uuid,
    pub username: Option<String>,
    pub trial: bool,
    pub password: String, //ToDo Option<>
    pub cluster: String,  //ToDo Option<>
    pub created: NaiveDateTime,
    pub paid_at: Option<NaiveDateTime>,
    pub paid_days: Option<i32>,
}

impl UserRow {
    pub fn new(username: &str) -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Some(username.to_string()),
            trial: true,
            password: "random123".to_string(),
            cluster: "dev".to_string(),
            created: Utc::now().naive_utc(),
            paid_at: None,
            paid_days: None,
        }
    }

    pub fn as_create_user_request(&self) -> UserRequest {
        UserRequest {
            user_id: self.user_id,
            action: Action::Create,
            env: self.cluster.clone(),
            trial: Some(self.trial),
            limit: Some(1000), //Todo fix
            password: Some(self.password.clone()),
        }
    }
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
                username: None,
                trial,
                password,
                cluster,
                created,
                paid_at: None,
                paid_days: None,
            }
        })
        .collect();

    Ok(users)
}

pub async fn user_exist(client: Arc<Mutex<Client>>, username: String) -> Option<Uuid> {
    let client = client.lock().await;

    let query = "
        SELECT id 
        FROM users 
        WHERE username = $1
    ";

    match client.query(query, &[&username]).await {
        Ok(rows) => rows.first().map(|row| row.get(0)),
        Err(err) => {
            log::error!("Database query failed: {}", err);
            None
        }
    }
}

pub async fn insert_user(
    client: Arc<Mutex<Client>>,
    user: UserRow,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let client = client.lock().await;

    let query = "
        INSERT INTO users (id, username, trial, password, cluster, created, paid_at, paid_days)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
    ";

    client
        .execute(
            query,
            &[
                &user.user_id,
                &user.username,
                &user.trial,
                &user.password,
                &user.cluster,
                &user.created,
                &user.paid_at,
                &user.paid_days,
            ],
        )
        .await?;

    Ok(())
}
