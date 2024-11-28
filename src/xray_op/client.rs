use crate::config2::Settings;
use crate::xray_api::xray::app::proxyman::command::handler_service_client::HandlerServiceClient;
use std::sync::Arc;
use tonic::transport::Channel;

use tokio::sync::Mutex;

pub async fn create_client(settings: Settings) -> Arc<Mutex<HandlerServiceClient<Channel>>> {
    let channel = Channel::from_shared(settings.xray.xray_api_endpoint)
        .unwrap()
        .connect()
        .await
        .unwrap();

    Arc::new(Mutex::new(HandlerServiceClient::new(channel)))
}
