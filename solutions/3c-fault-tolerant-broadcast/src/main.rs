use std::{
    io,
    sync::{Arc, Mutex},
};

use serde_json::json;

use node::{Message, Node, build_default_handlers};

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let mut handlers = build_default_handlers();
    handlers.insert(
        "read",
        Arc::new(|node_mutex, msg| {
            Box::pin(async move {
                let node = node_mutex.lock().unwrap();
                let mut all_values = Vec::<_>::new();
                for val in &node.values {
                    all_values.push(*val);
                }

                let Some(reply) =
                    node.build_reply("read_ok", &msg, json!({"messages": all_values}))
                else {
                    return;
                };

                if let Err(e) = Node::send(&reply) {
                    log::error!("failed to send read_ok: {e}");
                }
            })
        }),
    );
    handlers.insert(
        "topology",
        Arc::new(|node_mutex, msg| {
            Box::pin(async move {
                let mut node = node_mutex.lock().unwrap();
                let Some(topo) = msg.body["topology"].as_object() else {
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
    handlers.insert(
        "broadcast",
        Arc::new(|node_mutex, msg| {
            Box::pin(async move {
                let mut node = node_mutex.lock().unwrap();
                let number = msg.body["message"].as_u64().unwrap();
                if node.values.contains(&number) {
                    return;
                }
                node.values.insert(number);
                let _ = Node::send(&node.build_reply("broadcast_ok", &msg, json!({})).unwrap())
                    .map_err(|e| e.to_string());
                let node_id = node.id.clone();
                let all_nodes = node.topology.iter().cloned().collect::<Vec<_>>();
                drop(node);

                // send it to everyone else
                for n in all_nodes {
                    if *n == node_id {
                        continue;
                    }
                    let new_msg = Message {
                        dest: n,
                        src: node_id.clone(),
                        body: msg.body.clone(),
                    };
                    let node_mut = node_mutex.clone();
                    let _ = Node::send_synchronous(
                        &node_mut,
                        new_msg,
                        tokio::time::Duration::from_millis(500),
                    );
                }
            })
        }),
    );

    let node_mutex = Arc::new(Mutex::new(Node::default()));
    Node::serve(node_mutex, handlers).await
}
