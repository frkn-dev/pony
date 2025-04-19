use std::sync::Arc;

use log::{error, info};
use tokio::sync::Mutex;
use zmq::Socket as ZmqSocket;

use crate::AgentSettings;
use crate::HandlerClient;
use crate::Message;
use crate::NodeStorage;
use crate::State;

pub struct Subscriber {
    socket: Arc<Mutex<ZmqSocket>>,
}

impl Subscriber {
    pub fn new(endpoint: &str, topic: &str) -> Self {
        let context = zmq::Context::new();
        let socket = context
            .socket(zmq::SUB)
            .expect("Failed to create SUB socket");

        socket
            .connect(endpoint)
            .expect("Failed to connect SUB socket");
        socket
            .set_subscribe(topic.as_bytes())
            .expect("Failed to subscribe to topic");

        // wrap in Arc<Mutex<>> to make it Send + Sync
        Self {
            socket: Arc::new(Mutex::new(socket)),
        }
    }

    pub fn clone(&self) -> Self {
        Self {
            socket: Arc::clone(&self.socket),
        }
    }

    pub async fn recv(&self) -> Option<String> {
        match self.socket.lock().await.recv_string(0) {
            Ok(Ok(data)) => Some(data),
            _ => None,
        }
    }

    pub async fn run<T>(
        &self,
        client: Arc<Mutex<HandlerClient>>,
        settings: AgentSettings,
        state: Arc<Mutex<State<T>>>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: NodeStorage + Send + Sync + Clone + 'static,
    {
        info!(
            "ZMQ Subscriber connected to {}:{}",
            settings.zmq.sub_endpoint, settings.node.env
        );

        loop {
            tokio::task::yield_now().await;
            if let Some(data) = self.recv().await {
                let client = client.clone();
                let state = state.clone();
                let settings = settings.clone();

                tokio::spawn(async move {
                    let mut parts = data.splitn(2, ' ');
                    let topic = parts.next().unwrap_or("");
                    let payload = parts.next().unwrap_or("");

                    if topic != settings.node.env {
                        return;
                    }

                    if let Ok(message) = serde_json::from_str::<Message>(payload) {
                        if let Err(err) = message
                            .handle_sub_message(client.clone(), state.clone())
                            .await
                        {
                            error!("ZMQ SUB: Failed to handle message: {}", err);
                        }
                    }
                });
            }
        }
    }
}
