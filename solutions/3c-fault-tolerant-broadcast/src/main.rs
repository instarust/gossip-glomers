use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
};

use serde_json::json;

use node::{Node, build_default_handlers};

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let node_mutex = Arc::new(Mutex::new(Node::default()));
    let mut handlers: HashMap<String, node::FnHandler> = build_default_handlers();
    handlers.insert(
        String::from("read"),
        Arc::new(|node_mutex, msg| {
            Box::pin(async move {
                let node = node_mutex.lock().unwrap();
                let mut all_values: Vec<u64> = Vec::new();
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
        String::from("broadcast"),
        Arc::new(|node_mutex, msg| {
            Box::pin(async move {
                let mut node = node_mutex.lock().unwrap();
                let number = msg["body"]["message"].as_u64().unwrap();
                if node.values.contains(&number) {
                    return;
                }
                node.values.insert(number);
                let _ = Node::send(&node.build_reply("broadcast_ok", &msg, json!({})).unwrap())
                    .map_err(|e| e.to_string());
                let node_id = node.id.clone();
                let all_nodes: Vec<_> = { node.topology.iter().cloned().collect() };
                drop(node);

                // send it to everyone else
                for n in all_nodes {
                    if *n == msg["src"] || *n == node_id {
                        continue;
                    }
                    let new_msg = json!({
                        "src": node_id,
                        "dest": n,
                        "body": msg["body"],
                    });
                    let node_mut = node_mutex.clone();
                    let _ = Node::send_synchronous(
                        &node_mut,
                        new_msg.clone(),
                        tokio::time::Duration::from_millis(500),
                    );
                }
            })
        }),
    );

    Node::serve(node_mutex, handlers).await
}
