use std::{
    io,
    sync::{Arc, Mutex},
};

use serde_json::json;

use node::{Node, build_default_handlers};

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let mut handlers = build_default_handlers();
    handlers.insert(
        "echo",
        Arc::new(|node_mutex, msg| {
            Box::pin(async move {
                let node = node_mutex.lock().unwrap();
                let echo = msg.body["echo"].as_str().ok_or_else(|| {
                    log::error!("ignoring invalid echo message :(");
                })?;
                let reply = node
                    .build_reply("echo_ok", &msg, json!({"echo": echo}))
                    .ok_or(())?;
                Node::send(&reply).map_err(|e| {
                    log::error!("failed to send echo_ok: {e}");
                })
            })
        }),
    );

    let node_mutex = Arc::new(Mutex::new(Node::default()));
    Node::serve(node_mutex, handlers).await
}
