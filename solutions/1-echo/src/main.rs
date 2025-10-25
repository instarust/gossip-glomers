use std::io;

use serde_json::json;

use node::Node;

fn main() -> io::Result<()> {
    env_logger::init();

    let mut node = Node::default();
    node.handlers.insert(
        String::from("echo"),
        Box::new(|node, msg| {
            let Some(echo) = msg["body"]["echo"].as_str() else {
                log::error!("ignoring invalid echo message :(");
                return;
            };

            let Some(reply) = node.build_reply("echo_ok", msg, json!({"echo": echo})) else {
                return;
            };

            if let Err(e) = node.send(&reply) {
                log::error!("failed to send echo_ok: {e}");
            }
        }),
    );

    node.serve()
}
