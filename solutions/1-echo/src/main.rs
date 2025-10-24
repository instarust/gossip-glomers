use std::io;

use serde_json::json;

use node::Node;

fn main() -> io::Result<()> {
    env_logger::init();

    let mut node = Node::default();
    node.handlers.insert(
        "echo".to_string(),
        Box::new(|node, msg| {
            let echo = msg["body"]["echo"].as_str().unwrap();
            let mut reply = json!({
                "type": "echo_ok",
                "echo": echo,
            });
            let _ = node
                .send(&node.build_reply(msg["src"].as_str().unwrap(), &msg, &mut reply))
                .map_err(|e| e.to_string());
        }),
    );

    node.serve()
}
