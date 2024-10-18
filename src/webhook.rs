use actix_web::web::Data;
use actix_web::web::Path;
use actix_web::{post, web, HttpResponse, Responder};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use chrono::{DateTime, Utc};
use log::{debug, error};
use serde::Deserialize;
use std::str::FromStr;
use std::sync::Arc;

use crate::config2::Settings;
use crate::metrics::{AsMetric, Metric};
use crate::utils::send_to_carbon;

#[derive(Deserialize)]
struct Product {
    id: String,
    title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebhookPayload {
    product: Product,
    amount: f64,
    currency: String,
    timestamp: String,
    status: String,
    error_message: String,
}

impl AsMetric for WebhookPayload {
    type Output = f64;

    fn as_metric(&self, name: &str, settings: Settings) -> Vec<Metric<Self::Output>> {
        let path = format!("{}.{}.{}", settings.app.env, self.product.id, name);
        let datetime =
            DateTime::<Utc>::from_str(&self.timestamp).expect("Failed to parse timestamp");

        vec![Metric {
            path,
            value: self.amount,
            timestamp: datetime.timestamp() as u64,
        }]
    }
}

#[derive(Deserialize)]
struct Provider {
    provider_name: String,
}

#[post("/webhook/{provider_name}")]
async fn webhook_handler(
    auth: BearerAuth,
    req: Path<Provider>,
    payload: web::Json<WebhookPayload>,
    settings: Data<Arc<Settings>>,
) -> impl Responder {
    let verified = validate(auth.token(), &settings.app.api_token);
    if verified {
        let settings_ref = Arc::as_ref(settings.get_ref()).clone();
        if payload.status.trim() == "failed" {
            let metrics = payload.as_metric(
                &format!("{}.error", req.provider_name),
                settings_ref.clone(),
            );
            for metric in metrics {
                match send_to_carbon(&metric, &settings_ref.carbon.address.clone()).await {
                    Ok(_) => debug!(
                        "Error transaction {} (ID: {}): {}",
                        payload.product.title, payload.product.id, payload.error_message,
                    ),
                    Err(err) => error!("Can't send to carbon {}", err),
                }
            }
        } else {
            let metrics = payload.as_metric(
                &format!("{}.success", req.provider_name),
                settings_ref.clone(),
            );
            for metric in metrics {
                match send_to_carbon(&metric, &settings_ref.carbon.address.clone()).await {
                    Ok(_) => debug!(
                        "Success transaction {} (ID: {}) {} {}",
                        payload.product.title, payload.product.id, payload.amount, payload.currency
                    ),
                    Err(err) => error!("Can't send to carbon {}", err),
                }
            }
        }
        HttpResponse::Ok().body("Ok")
    } else {
        HttpResponse::Forbidden().body("Forbidden")
    }
}

fn validate(token: &str, recieved: &str) -> bool {
    token == recieved
}
