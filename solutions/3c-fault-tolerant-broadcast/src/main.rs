use std::io;

use serde_json::json;

use node::Node;

fn main() -> io::Result<()> {
    env_logger::init();

    let mut node = Node::default();
    node.handlers.insert(
        String::from("read"),
        Box::new(|node, msg| {
            let mut all_values: Vec<u64> = Vec::new();
            for val in &node.values {
                all_values.push(*val);
            }

            let Some(reply) = node.build_reply("read_ok", msg, json!({"messages": all_values}))
            else {
                return;
            };

            if let Err(e) = node.send(&reply) {
                log::error!("failed to send read_ok: {e}");
            }
        }),
    );
    node.handlers.insert(
        String::from("broadcast"),
        Box::new(|node, msg| {
            // check if i had this value before, if i did. do nothing at all
            // if i didn't add it to the hashmap and send it to the others
            let number = msg["body"]["message"].as_u64().unwrap();
            if node.values.contains(&number) {
                return;
            }

            node.values.insert(number);
            // send it to everyone else
            for n in node.topology.borrow().iter() {
                if *n == msg["src"] || *n == node.id {
                    continue;
                }
                let new_msg = json!({
                    "src": node.id,
                    "dest": n,
                    "body": msg["body"],
                });

                if let Err(e) = node.send(&new_msg) {
                    log::error!("failed to send broadcast message to {n}: {e}");
                }
            }

            let Some(reply) = node.build_reply("broadcast_ok", msg, json!({})) else {
                return;
            };

            if let Err(e) = node.send(&reply) {
                log::error!("failed to send broadcast_ok: {e}");
            }
        }),
    );

    node.serve()
}
