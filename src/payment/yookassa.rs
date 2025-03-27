use anyhow::{bail, Result};
use log::debug;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::BotSettings;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct PaymentRequest {
    amount: Amount,
    capture: bool,
    confirmation: ConfirmationReq,
    description: String,
    metadata: Metadata,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ConfirmationReq {
    #[serde(rename = "type")]
    confirmation_type: String,
    return_url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PaymentResponse {
    pub id: String,
    pub status: String,
    pub paid: bool,
    pub amount: Amount,
    pub confirmation: ConfirmationResp,
    pub created_at: String,
    pub description: Option<String>,
    pub metadata: Option<Metadata>,
    pub recipient: Recipient,
    pub refundable: bool,
    pub test: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Amount {
    pub value: String,
    pub currency: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConfirmationResp {
    #[serde(rename = "type")]
    pub confirmation_type: String,
    pub confirmation_url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Metadata {
    pub user_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Recipient {
    pub account_id: String,
    pub gateway_id: String,
}

pub async fn create_payment(
    settings: Arc<Mutex<BotSettings>>,
    user_id: uuid::Uuid,
) -> Result<String> {
    let settings = settings.lock().await;
    let shop_id = &settings.pay.yookassa.shop_id;
    let secret_key = &settings.pay.yookassa.secret_key;
    let amount = &settings.pay.yookassa.price;
    let addr = &settings.bot.address;

    let client = Client::new();
    let url = "https://api.yookassa.ru/v3/payments";

    let key = uuid::Uuid::new_v4();

    let payment_request = PaymentRequest {
        amount: Amount {
            value: amount.to_string(),
            currency: "RUB".to_string(),
        },
        capture: true,
        confirmation: ConfirmationReq {
            confirmation_type: "redirect".to_string(),
            return_url: addr.to_string(),
        },
        description: "Оплата подписки".to_string(),
        metadata: {
            Metadata {
                user_id: user_id.to_string(),
            }
        },
    };

    println!("{:?}", payment_request.clone());

    let response = client
        .post(url)
        .header(header::CONTENT_TYPE, "application/json")
        .header("Idempotence-Key", key.to_string())
        .basic_auth(shop_id, Some(secret_key))
        .json(&payment_request)
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        debug!("Ошибка от Юкассы ({}): {}", status, body);
        bail!("Ошибка создания платежа в Юкассе");
    }

    let payment_response: PaymentResponse = serde_json::from_str(&body)?;

    Ok(payment_response.confirmation.confirmation_url)
}
