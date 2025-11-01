use std::io;

use std::sync::{Arc, Mutex};

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
                let srv = node_mutex.lock().unwrap();
                let Some(echo) = msg.body["echo"].as_str() else {
                    log::error!("ignoring invalid echo message :(");
                    return Ok(());
                };

                let Some(reply) = &srv.build_reply("echo_ok", &msg, json!({"echo": echo})) else {
                    return Ok(());
                };

                srv.send(reply).map_err(|e| {
                    log::error!("failed to send echo_ok: {e}");
                })
            })
        }),
    );

    let node_mutex = Arc::new(Mutex::new(Node::default()));
    node::serve(node_mutex, handlers).await
}
