use std::{
    io,
    sync::{Arc, Mutex},
};

use serde_json::json;

use node::{Message, Node, Server, build_default_handlers};

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let mut handlers = build_default_handlers();
    handlers.insert(
        "topology",
        Arc::new(|srv_mutex, msg| {
            Box::pin(async move {
                let mut srv_any = srv_mutex.lock().unwrap();
                let node = srv_any.as_any_mut().downcast_mut::<Node>().ok_or_else(|| {
                    log::error!("server wasn't a node while calling a node handler");
                })?;
                let topo = msg.body["topology"].as_object().ok_or_else(|| {
                    log::error!("ignoring invalid topology message :(");
                })?;

                node.topology.extend(topo.keys().cloned());

                let reply = node.build_reply("topology_ok", &msg, json!({})).ok_or(())?;
                node.send(&reply).map_err(|e| {
                    log::error!("failed to send topology_ok: {e}");
                })
            })
        }),
    );
    handlers.insert(
        "read",
        Arc::new(|srv_mutex, msg| {
            Box::pin(async move {
                let mut srv = srv_mutex.lock().unwrap();
                let node= srv.as_any_mut().downcast_mut::<Node>().ok_or_else(|| {
                    log::error!("server wasn't a node when calling a node handler");
                })?;

                let values: Vec<&u64> = node.values.iter().collect();
                let reply = node
                    .build_reply("read_ok", &msg, json!({"messages": values}))
                    .ok_or(())?;

                srv.send(&reply).map_err(|e| {
                    log::error!("failed to send read_ok: {e}");
                })
            })
        }),
    );
    handlers.insert(
        "broadcast",
        Arc::new(|srv_mutex, msg| {
            Box::pin(async move {
                let mut srv_any = srv_mutex.lock().unwrap();
                let node= srv_any.as_any_mut().downcast_mut::<Node>().ok_or_else(|| {
                    log::error!("server wasn't a node when calling a node handler");
                })?;

                let number = msg.body["message"].as_u64().ok_or(())?;
                if node.values.contains(&number) {
                    return Ok(());
                }

                node.values.insert(number);

                let reply = node
                    .build_reply("broadcast_ok", &msg, json!({}))
                    .ok_or(())?;

                _ = node.send(&reply);

                let node_id = node.get_id();
                let all_nodes = node.get_topology();
                drop(srv_any);

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
                    let node_mut = srv_mutex.clone();
                    let _ = node::send_synchronous(
                        &node_mut,
                        new_msg,
                        tokio::time::Duration::from_millis(500),
                    );
                }
                Ok(())
            })
        }),
    );

    let node_mutex = Arc::new(Mutex::new(Node::default()));
    node::serve(node_mutex, handlers).await
}
