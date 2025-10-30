use std::{
    io,
    sync::{Arc, Mutex},
};

use serde_json::json;
use uuid::Uuid;

use node::{Node, build_default_handlers};

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let mut handlers = build_default_handlers();
    handlers.insert(
        "generate",
        Arc::new(|node_mutex, msg| {
            Box::pin(async move {
                let Some("generate") = msg.body["type"].as_str() else {
                    log::error!("ignoring invalid generate message :(");
                    return Err(());
                };

                let node = node_mutex.lock().unwrap();
                let reply = &node
                    .build_reply(
                        "generate_ok",
                        &msg,
                        json!({"id": Uuid::now_v7().to_string()}),
                    )
                    .ok_or(())?;
                Node::send(reply).map_err(|e| {
                    log::error!("failed to send generate_ok: {e}");
                })
            })
        }),
    );

    let node_mutex = Arc::new(Mutex::new(Node::default()));
    Node::serve(node_mutex, handlers).await
}
