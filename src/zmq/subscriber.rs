use std::sync::Arc;

use log::debug;
use log::warn;
use log::{error, info};
use tokio::sync::Mutex;
use uuid::Uuid;
use zmq::Socket as ZmqSocket;

use crate::AgentSettings;
use crate::HandlerClient;
use crate::Message;
use crate::NodeStorage;
use crate::State;
use crate::Topic;

pub struct Subscriber {
    socket: Arc<Mutex<ZmqSocket>>,
}

impl Subscriber {
    pub fn new(endpoint: &str, uuid: &Uuid, env: &str) -> Self {
        let context = zmq::Context::new();
        let socket = context
            .socket(zmq::SUB)
            .expect("Failed to create SUB socket");

        socket
            .connect(endpoint)
            .expect("Failed to connect SUB socket");

        let topics = vec![format!("{}", *uuid), format!("{}", env)];
        info!("Subscribed to topics: {:?}", Topic::all(*uuid, env));

        for topic in topics {
            socket
                .set_subscribe(topic.as_bytes())
                .expect("Failed to subscribe to topic");
        }

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
            "ZMQ Subscriber connected to {} for topics: {:?}",
            settings.zmq.sub_endpoint,
            Topic::all(settings.node.uuid, &settings.node.env)
        );

        loop {
            if let Some(data) = self.recv().await {
                debug!("SUB: Got raw message: {:?}", data);
                let client = client.clone();
                let state = state.clone();
                let settings = settings.clone();

                tokio::spawn(async move {
                    let mut parts = data.splitn(2, ' ');
                    let topic_str = parts.next().unwrap_or("");
                    let payload = parts.next().unwrap_or("");

                    let topic = Topic::from_raw(topic_str);

                    match topic {
                        Topic::Init(uuid) => {
                            if uuid != settings.node.uuid.to_string() {
                                debug!("ZMQ: Received init for another node: {}", uuid);
                                return;
                            }
                        }
                        Topic::Updates(env) => {
                            if env != settings.node.env {
                                debug!("ZMQ: Received update for another env: {}", env);
                                return;
                            }
                        }
                        Topic::Unknown(raw) => {
                            warn!("ZMQ: Received unknown topic: {}", raw);
                            return;
                        }
                    }

                    if let Ok(message) = serde_json::from_str::<Message>(payload) {
                        debug!("Message recieved {:?}", message);
                        if let Err(err) = message
                            .handle_sub_message(client.clone(), state.clone())
                            .await
                        {
                            error!("ZMQ SUB: Failed to handle message: {}", err);
                        }
                    } else {
                        error!("ZMQ SUB: Failed to parse message: {}", payload);
                    }
                });
            }
        }
    }
}
