use std::io;

use std::sync::{Arc, Mutex};

use serde_json::json;

use node::Node;

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let node_mutex = Arc::new(Mutex::new(Node::default()));
    let mut handlers = node::build_default_handlers();
    handlers.insert(
        String::from("echo"),
        Arc::new(|node_mutex, msg| {
            Box::pin(async move {
                let node = node_mutex.lock().unwrap();
                let Some(echo) = msg["body"]["echo"].as_str() else {
                    log::error!("ignoring invalid echo message :(");
                    return;
                };

                let Some(reply) = &node.build_reply("echo_ok", &msg, json!({"echo": echo})) else {
                    return;
                };

                if let Err(e) = Node::send(&reply) {
                    log::error!("failed to send echo_ok: {e}");
                }
            })
        }),
    );

    Node::serve(node_mutex, handlers).await
}
