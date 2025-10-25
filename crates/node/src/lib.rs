use std::{
    collections::{HashMap, HashSet},
    fmt,
    io::{self, Write},
    pin::Pin,
    sync::{Arc, Mutex},
};

use tokio::task;

use serde_json::json;

pub type FnHandler = Arc<
    dyn Fn(Arc<Mutex<Node>>, serde_json::Value) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync,
>;
#[derive(Default)]
pub struct Node {
    pub id: String,
    pub values: HashSet<u64>,
    pub callbacks: HashMap<u64, Box<dyn FnOnce() + Send + Sync>>,
    pub topology: HashSet<String>,
    pub msg_count: u64,
}
impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("id", &self.id)
            .field("values", &self.values)
            .field("topology", &self.topology)
            .field("msg_count", &self.msg_count)
            .field(
                "callbacks",
                &format!(
                    "{} callbacks, keys: `{}`",
                    self.callbacks.len(),
                    self.callbacks
                        .keys()
                        .map(|k| { k.to_string() })
                        .collect::<Vec<String>>()
                        .join("`, `")
                ),
            )
            .finish()
    }
}

/// Returns a map of default message handlers for the node.
/// # Panics
/// The handlers created by this function may panic if the mutex on the node is poisoned.
#[must_use]
pub fn build_default_handlers() -> HashMap<String, FnHandler> {
    let mut handlers: HashMap<String, FnHandler> = HashMap::new();

    handlers.insert(
        String::from("init"),
        Arc::new(|node_mutex, msg| {
            Box::pin(async move {
                let mut node = node_mutex.lock().unwrap();
                if !node.id.is_empty() {
                    return;
                }

                if let Some(node_id) = msg["body"]["node_id"].as_str() {
                    node.id = node_id.to_string();
                } else {
                    log::error!("ignoring invalid init message :(");
                    return;
                }

                let Some(reply) = node.build_reply("init_ok", &msg, json!({})) else {
                    return;
                };

                if let Err(e) = Node::send(&reply) {
                    log::error!("failed to send init_ok: {e}");
                }
            })
        }),
    );
    handlers.insert(
        String::from("topology"),
        Arc::new(|node_mutex, msg| {
            Box::pin(async move {
                let mut node = node_mutex.lock().unwrap();
                let Some(topo) = msg["body"]["topology"].as_object() else {
                    log::error!("ignoring invalid topology message :(");
                    return;
                };
                for k in topo.keys() {
                    node.topology.insert(k.clone());
                }

                let Some(reply) = node.build_reply("topology_ok", &msg, json!({})) else {
                    return;
                };

                if let Err(e) = Node::send(&reply) {
                    log::error!("failed to send topology_ok: {e}");
                }
            })
        }),
    );

    handlers
}

impl Node {
    /// # Errors
    /// - forwards `io` errors
    pub async fn serve(
        node: Arc<Mutex<Node>>,
        handlers: HashMap<String, FnHandler>,
    ) -> io::Result<()> {
        // 10 is an arbitrary value, the size doesn't actually matter (wink, wink)
        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

        tokio::spawn(async move {
            log::info!("starting message thread");
            while let Some(msg) = rx.recv().await {
                let node_mut = node.clone();
                let h = handlers.clone();
                tokio::spawn(async move {
                    if let Err(e) = Node::handle_msg(node_mut, &h, msg).await {
                        log::error!("failed to handle message: {e}");
                    }
                });
            }
        });

        log::info!("starting listener loop");
        loop {
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            log::info!("message: {input}");
            if let Err(e) = tx.send(input).await {
                log::error!("{e}");
            }
        }
    }

    /// # Errors
    /// - forwards `serde_json` errors
    /// - forwards `io` errors
    pub fn send(msg: &serde_json::Value) -> io::Result<()> {
        let json_str = serde_json::to_string(msg)?;
        let mut writer = std::io::stdout();
        writer.write_all(json_str.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        Ok(())
    }

    #[must_use]
    pub fn build_reply(
        &self,
        r#type: &str,
        msg: &serde_json::Value,
        mut body: serde_json::Value,
    ) -> Option<serde_json::Value> {
        let Some(dest) = msg["src"].as_str() else {
            log::error!("init message missing `src` field, cannot reply");
            return None;
        };
        let Some(msg_id) = msg["body"]["msg_id"].as_u64() else {
            log::error!("couldn't construct reply to `{msg}`: missing `.body.msg_id` field");
            return None;
        };

        body["type"] = r#type.into();
        body["in_reply_to"] = msg_id.into();

        Some(json!({"src": self.id, "dest": dest, "body": body}))
    }

    /// # Errors
    /// - forwards `serde_json` errors
    /// - forwards `io` errors
    /// # Panics
    /// This function will panic if the mutex on `node_arc` is poisoned.
    pub fn send_synchronous(
        node_mut: &Arc<Mutex<Node>>,
        mut msg: serde_json::Value,
        message_timout: tokio::time::Duration,
    ) -> io::Result<()> {
        let mut node = node_mut.lock().unwrap();
        node.msg_count += 1;
        let msg_count = node.msg_count;
        msg["body"]["msg_id"] = msg_count.into();
        let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();

        // register the callback
        node.callbacks.insert(
            msg_count,
            Box::new(move || {
                let _ = tx.send(());
            }),
        );

        // initial send
        Node::send(&msg)?;

        let node_mut_copy = node_mut.clone();
        task::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut rx => {
                        break;
                    },

                    () = tokio::time::sleep(message_timout) => {
                        let node = node_mut_copy.lock().unwrap();
                        log::info!("node {}: receiving a response from {} timed out, sending again", node.id, msg["dest"].as_str().unwrap());
                        let _ = Node::send(&msg);
                    },
                }
            }
            log::info!("broadcast exiting");
        });
        Ok(())
    }

    /// # Errors
    /// - forwards `serde_json` errors
    /// - returns an error if the message type is not found in the handlers map
    /// # Panics
    /// - Panics if `msg["body"]["type"]` is not a string
    /// - panics if the mutex on `node_arc` is poisoned
    pub async fn handle_msg(
        node_arc: Arc<Mutex<Node>>,
        handlers_map: &HashMap<String, FnHandler>,
        raw_msg: String,
    ) -> Result<(), String> {
        let msg: serde_json::Value = serde_json::from_str(&raw_msg).map_err(|e| e.to_string())?;

        if let Some(callback) = msg["body"]["msg_id"]
            .as_u64()
            .and_then(|id| node_arc.lock().ok()?.callbacks.remove(&id))
        {
            callback();
        }

        let msg_type = msg["body"]["type"].as_str().unwrap();
        let handler = handlers_map
            .get(msg_type)
            .ok_or(format!("handler {msg_type} not found"))?;
        handler(node_arc, msg).await;

        Ok(())
    }
}
