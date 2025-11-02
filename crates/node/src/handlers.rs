use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use serde_json::json;

use crate::server::Server;
use crate::types::Message;

pub type FnHandler<T> = Arc<
    dyn Fn(Arc<Mutex<T>>, Message) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send>>
        + Send
        + Sync,
>;

pub type HandlersMap<T> = HashMap<&'static str, FnHandler<T>>;

/// Returns a map of default message handlers for the node.
/// # Panics
/// The handlers created by this function may panic if the mutex on the node is poisoned.
#[must_use]
pub fn build_default_handlers() -> HandlersMap<dyn Server + Send + Sync> {
    let mut handlers = HandlersMap::<dyn Server + Send + Sync>::new();
    handlers.insert(
        "init",
        Arc::new(|srv_mutex, msg| {
            Box::pin(async move {
                let mut srv = srv_mutex.lock().unwrap();
                if !srv.get_id().is_empty() {
                    return Ok(());
                }

                srv.set_id(
                    msg.body["node_id"]
                        .as_str()
                        .ok_or_else(|| {
                            log::error!("ignoring invalid init message :(");
                        })?
                        .to_string(),
                );

                // handle when the topology is sent in the init
                if let Some(other_node_ids) = msg.body["node_ids"].as_array() {
                    let mut topo: HashSet<String> = HashSet::new();
                    for n in other_node_ids {
                        let Some(other_node_id) = n.as_str() else {
                            continue;
                        };
                        topo.insert(other_node_id.to_string());
                    }
                    srv.set_topology(topo);
                }

                let reply = srv.build_reply("init_ok", &msg, json!({})).ok_or(())?;
                srv.send(&reply).map_err(|e| {
                    log::error!("failed to send init_ok: {e}");
                })
            })
        }),
    );
    handlers
}
