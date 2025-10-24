use std::io;

use serde_json::json;

use node::Node;

fn main() -> io::Result<()> {
    env_logger::init();

    let mut node = Node::default();
    node.handlers.insert(
        "read".to_string(),
        Box::new(|node, msg| {
            let mut all_values: Vec<u64> = Vec::new();
            for val in &node.values {
                all_values.push(*val);
            }
            let mut reply = json!({
                "type": "read_ok",
                "messages": all_values,
            });
            let _ = node
                .send(&node.build_reply(msg["src"].as_str().unwrap(), &msg, &mut reply))
                .map_err(|e| e.to_string());
        }),
    );
    node.handlers.insert(
        "broadcast".to_string(),
        Box::new(|node, msg| {
            // check if i had this value before, if i did. do nothing at all
            // if i didn't add it to the hashmap and send it to the others
            let number = msg["body"]["message"].as_u64().unwrap();
            if node.values.contains(&number) {
                return;
            }

            node.values.insert(number);
            // send it to everyone else
            for n in node.topology.borrow().keys() {
                if *n == msg["src"] || *n == node.id {
                    continue;
                }
                let new_msg = json!({
                    "src": node.id,
                    "dest": n,
                    "body": msg["body"],
                });
                let _ = node.send(&new_msg).map_err(|e| e.to_string());
            }
            let mut reply = json!({
                "type": "broadcast_ok",
            });
            let _ = node
                .send(&node.build_reply(msg["src"].as_str().unwrap(), &msg, &mut reply))
                .map_err(|e| e.to_string());
        }),
    );

    node.serve()
}
