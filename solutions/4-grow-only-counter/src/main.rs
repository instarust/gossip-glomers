use std::{
    io,
    sync::{Arc, Mutex},
};

use serde_json::json;

use node::{Message, SequentialKV, Server, build_default_handlers};

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();
    let seq_kv = Arc::new(Mutex::new(SequentialKV::default()));
    let mut handlers = build_default_handlers();

    handlers.insert(
        "add",
        Arc::new(|srv_mutex, msg| {
            Box::pin(async move {
                let mut skv_any = srv_mutex.lock().unwrap();
                let Some(skv) = skv_any.as_any_mut().downcast_mut::<SequentialKV>() else {
                    return Ok(());
                };

                let msg_hash = match msg.src.as_bytes()[0] as char {
                    'n' => {
                        if let Some(incoming_hash) = msg.body["hash"].as_u64() {
                            incoming_hash
                        } else {
                            log::error!("missing hash in message body");
                            return Ok(());
                        }
                    }
                    'c' => msg.hash(),
                    _ => {
                        log::error!("message has a sender that doesn't start with a c or n");
                        return Ok(());
                    }
                };
                let reply = &skv.build_reply("add_ok", &msg, json!({})).ok_or(())?;

                let _ = skv.send(reply).map_err(|e| {
                    log::error!("failed to send add_ok: {e}");
                });

                if skv.values.contains(&msg_hash) {
                    return Ok(());
                }

                if let Some(number) = msg.body["delta"].as_u64() {
                    skv.values.insert(msg_hash);
                    skv.counter += number;

                    let node_id = skv.get_id();
                    let all_nodes = skv.get_topology();
                    drop(skv_any);

                    // send it to everyone else
                    for n in all_nodes {
                        if *n == node_id {
                            continue;
                        }
                        let mut new_msg = Message {
                            dest: n,
                            src: node_id.clone(),
                            body: msg.body.clone(),
                        };
                        new_msg.body["hash"] = msg_hash.into();
                        let _ = node::send_synchronous(
                            &srv_mutex,
                            new_msg,
                            tokio::time::Duration::from_millis(500),
                        );
                    }
                }
                Ok(())
            })
        }),
    );
    handlers.insert(
        "read",
        Arc::new(|seq_kv_mutex, msg| {
            Box::pin(async move {
                let mut skv_any = seq_kv_mutex.lock().unwrap();
                let Some(skv) = skv_any.as_any_mut().downcast_mut::<SequentialKV>() else {
                    return Ok(());
                };

                let reply = &skv
                    .build_reply("read_ok", &msg, json!({"value": skv.counter}))
                    .ok_or(())?;

                skv.send(reply).map_err(|e| {
                    log::error!("failed to send read_ok: {e}");
                })
            })
        }),
    );
    node::serve(seq_kv, handlers).await
}
