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
                let reply = node
                    .build_reply("read_ok", &msg, json!({"messages": node.values.clone()}))
                    .ok_or(())?;
                Node::send(&reply).map_err(|e| {
                    log::error!("failed to send read_ok: {e}");
                })
            })
        }),
    );
    handlers.insert(
        "topology",
        Arc::new(|node_mutex, msg| {
            Box::pin(async move {
                let topo = msg.body["topology"].as_object().ok_or_else(|| {
                    log::error!("ignoring invalid topology message :(");
                })?;

                let mut node = node_mutex.lock().unwrap();

                node.topology.extend(topo.keys().cloned());

                let reply = node.build_reply("topology_ok", &msg, json!({})).ok_or(())?;
                Node::send(&reply).map_err(|e| {
                    log::error!("failed to send topology_ok: {e}");
                })
            })
        }),
    );
    handlers.insert(
        "broadcast",
        Arc::new(|node_mutex, msg| {
            Box::pin(async move {
                let mut node = node_mutex.lock().unwrap();

                let number = msg.body["message"].as_u64().ok_or(())?;
                if node.values.contains(&number) {
                    return Ok(());
                }
                node.values.insert(number);

                let reply = node
                    .build_reply("broadcast_ok", &msg, json!({}))
                    .ok_or(())?;
                _ = Node::send(&reply);

                let node_id = node.id.clone();
                let all_nodes = node.topology.clone();
                drop(node); // release the lock before responding

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
                    _ = Node::send_synchronous(
                        &node_mutex.clone(),
                        new_msg,
                        tokio::time::Duration::from_millis(500),
                    );
                }

                Ok(())
            })
        }),
    );

    let node_mutex = Arc::new(Mutex::new(Node::default()));
    Node::serve(node_mutex, handlers).await
}
