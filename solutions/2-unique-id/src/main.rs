use std::{
    io,
    sync::{Arc, Mutex},
};

use serde_json::json;
use uuid::Uuid;

use node::{Node, Server, build_default_handlers};

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let mut handlers = build_default_handlers();
    handlers.insert(
        "generate",
        Arc::new(|srv_mutex, msg| {
            Box::pin(async move {
                let mut srv_any = srv_mutex.lock().unwrap();
                let Some(node) = srv_any.as_any_mut().downcast_mut::<Node>() else {
                    return Ok(());
                };

                let Some("generate") = msg.body["type"].as_str() else {
                    log::error!("ignoring invalid generate message :(");
                    return Err(());
                };

                let reply = &node
                    .build_reply(
                        "generate_ok",
                        &msg,
                        json!({"id": Uuid::now_v7().to_string()}),
                    )
                    .ok_or(())?;
                node.send(reply).map_err(|e| {
                    log::error!("failed to send generate_ok: {e}");
                })
            })
        }),
    );

    let node_mutex = Arc::new(Mutex::new(Node::default()));
    node::serve(node_mutex, handlers).await
}
