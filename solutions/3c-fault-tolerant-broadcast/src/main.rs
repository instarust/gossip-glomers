use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
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
    values: HashSet<u64>,
    topology: RefCell<HashMap<String, bool>>,
    // msg_count: u64, // dead
    handlers: HashMap<String, FnHandler>,
}

fn build_handlers_map() -> HashMap<String, FnHandler> {
    let mut handlers: HashMap<String, FnHandler> = HashMap::new();
    handlers.insert(
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
                node.topology.borrow_mut().insert(k.clone(), true);
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
    handlers
}

impl Node {
    fn new(stdout: Stdout) -> Self {
        Node {
            id: String::new(),
            stdout: RefCell::new(stdout),
            values: HashSet::new(),
            topology: RefCell::new(HashMap::new()),
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

    // this thread can be made to own the node, again. allowing for avoiding locking the entire
    thread::spawn(move || {
        info!("starting message thread");
        for msg in rx {
            let node_copy = node.clone();
            thread::spawn(move || {
                if let Err(e) = node_copy.lock().unwrap().handle_msg(msg.as_str()) {
                    error!("failed to handle message: {e}");
                }
            });
        }
    });

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
