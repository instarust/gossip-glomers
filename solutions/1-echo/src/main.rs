use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Stdout, Write};
use std::sync::mpsc;
use std::thread;

use log::{error, info};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

type FnHandler = Box<dyn Fn(&mut Node, Value) + Send + Sync>;
struct Node {
    id: String,
    stdout: RefCell<Stdout>,
    // values: Vec<u64>, // dead
    topology: HashMap<String, u64>,
    handlers: HashMap<String, FnHandler>,
}

fn build_handlers_map() -> HashMap<String, FnHandler> {
    let mut handlers: HashMap<String, FnHandler> = HashMap::new();
    handlers.insert(
        "init".to_string(),
        Box::new(|node, msg| {
            if !node.id.is_empty() {
                return;
            }
            node.id = msg["body"]["node_id"].as_str().unwrap().to_string();
            let mut reply = json!({
                "type":"init_ok",
            });
            let _ = node
                .send(&node.build_reply(msg["src"].as_str().unwrap(), &msg, &mut reply))
                .map_err(|e| e.to_string());
        }),
    );
    handlers.insert(
        "topology".to_string(),
        Box::new(|node, msg| {
            let topo = msg["body"]["topology"].as_object().unwrap();
            for k in topo.keys() {
                node.topology.insert(k.clone(), 0);
            }
            let mut reply = json!({
                "type": "topology_ok",
            });
            let _ = node
                .send(&node.build_reply(msg["src"].as_str().unwrap(), &msg, &mut reply))
                .map_err(|e| e.to_string());
        }),
    );

    handlers.insert(
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
    handlers
}

impl Node {
    fn new(stdout: Stdout) -> Self {
        Node {
            id: String::new(),
            stdout: RefCell::new(stdout),
            topology: HashMap::new(),
            handlers: build_handlers_map(),
        }
    }

    fn send(&self, msg: &Value) -> io::Result<()> {
        let json_str = serde_json::to_string(msg)?;
        let mut writer = self.stdout.borrow_mut();
        writer.write_all(json_str.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        Ok(())
    }

    fn build_reply(&self, dest: &str, msg: &Value, body: &mut Value) -> Value {
        body["in_reply_to"] = msg["body"]["msg_id"].as_u64().unwrap().into();
        json!({
            "src": self.id,
            "dest": dest,
            "body": body,
        })
    }

    // #[allow(clippy::needless_pass_by_value)]
    fn handle_msg(&mut self, raw_msg: &str) -> Result<(), String> {
        let msg: Value = serde_json::from_str(raw_msg).map_err(|e| e.to_string())?;
        let msg_type = msg["body"]["type"].as_str().unwrap();

        let handler = self
            .handlers
            .remove(msg_type)
            .ok_or(format!("handler {} not found", msg["body"]["type"]))?;
        handler(self, msg.clone());
        self.handlers.insert(msg_type.to_string(), handler);
        Ok(())
    }
}

fn main() -> io::Result<()> {
    env_logger::init();
    let (tx, rx) = mpsc::channel::<String>();
    let stdin = io::stdin();
    let stdout = io::stdout();

    let node = Arc::new(Mutex::new(Node::new(stdout)));
    let node_copy = node.clone();

    thread::spawn(move || {
        info!("starting message thread");
        for msg in rx {
            if let Err(e) = node_copy.lock().unwrap().handle_msg(msg.as_str()) {
                error!("failed to handle message: {e}");
            }
        }
    });

    // info!("starting node main threap");
    // Node::run(node);

    // need to start another thread that runs the node swim gossip
    info!("starting listener loop");
    loop {
        let mut input = String::new();
        stdin.read_line(&mut input)?;
        info!("message: {input}");
        if let Err(e) = tx.send(input) {
            error!("{e}");
        }
    }
}
