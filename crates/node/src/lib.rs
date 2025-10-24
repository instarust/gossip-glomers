use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt,
    io::{self, Stdout, Write},
    sync::{Arc, Mutex, mpsc},
    thread,
};

use serde_json::json;

pub type FnHandler = Box<dyn Fn(&mut Node, serde_json::Value) + Send + Sync>;
#[allow(missing_debug_implementations)]
pub struct Node {
    stdout: RefCell<Stdout>,
    pub id: String,
    pub values: HashSet<u64>,
    pub topology: RefCell<HashMap<String, bool>>,
    pub handlers: HashMap<String, FnHandler>,
}
impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("id", &self.id)
            .field("stdout", &self.stdout.borrow())
            .field("values", &self.values)
            .field("topology", &self.topology.borrow())
            .field(
                "handlers",
                &format!(
                    "{} handler(s, keys: `{}`",
                    self.handlers.len(),
                    self.handlers
                        .keys()
                        .cloned()
                        .collect::<Vec<String>>()
                        .join("`, `")
                ),
            )
            .finish()
    }
}

#[must_use]
#[allow(clippy::missing_panics_doc)] // TODO
pub fn build_default_handlers() -> HashMap<String, FnHandler> {
    let mut handlers: HashMap<String, FnHandler> = HashMap::new();
    handlers.insert(
        String::from("init"),
        Box::new(|node, msg| {
            if !node.id.is_empty() {
                return;
            }
            node.id = msg["body"]["node_id"].as_str().unwrap().to_string();
            let mut reply = json!({ "type":"init_ok" });
            let _ = node
                .send(&node.build_reply(msg["src"].as_str().unwrap(), &msg, &mut reply))
                .map_err(|e| e.to_string());
        }),
    );
    handlers.insert(
        String::from("topology"),
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

    handlers
}

impl Default for Node {
    fn default() -> Self {
        Node {
            id: String::new(),
            stdout: RefCell::new(io::stdout()),
            values: HashSet::new(),
            topology: RefCell::new(HashMap::new()),
            handlers: build_default_handlers(),
        }
    }
}

impl Node {
    /// # Errors
    /// - forwards `serde_json` errors
    /// - forwards `io` errors
    pub fn send(&self, msg: &serde_json::Value) -> io::Result<()> {
        let json_str = serde_json::to_string(msg)?;
        let mut writer = self.stdout.borrow_mut();
        writer.write_all(json_str.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        Ok(())
    }

    #[allow(clippy::missing_panics_doc)] // TODO
    pub fn build_reply(
        &self,
        dest: &str,
        msg: &serde_json::Value,
        body: &mut serde_json::Value,
    ) -> serde_json::Value {
        body["in_reply_to"] = msg["body"]["msg_id"].as_u64().unwrap().into();
        json!({
            "src": self.id,
            "dest": dest,
            "body": body,
        })
    }

    /// # Errors
    /// - "missing type field" if the message does not contain `.body.type`
    /// - forwards `serde_json` errors as strings
    pub fn handle_msg(&mut self, raw_msg: &str) -> Result<(), String> {
        let msg: serde_json::Value = serde_json::from_str(raw_msg).map_err(|e| e.to_string())?;
        let msg_type = msg["body"]["type"].as_str().ok_or("missing type field")?;

        let handler = self
            .handlers
            .remove(msg_type)
            .ok_or(format!("handler {} not found", msg["body"]["type"]))?;
        handler(self, msg.clone());
        self.handlers.insert(msg_type.to_string(), handler);
        Ok(())
    }

    /// # Errors
    /// - forwards `io` errors
    #[allow(clippy::missing_panics_doc)] // TODO
    pub fn serve(self) -> io::Result<()> {
        let node = Arc::new(Mutex::new(self));
        let (tx, rx) = mpsc::channel::<String>();
        // this thread can be made to own the node, again. allowing for avoiding locking the entire
        thread::spawn(move || {
            log::info!("starting message thread");
            for msg in rx {
                let node = node.clone();
                thread::spawn(move || {
                    if let Err(e) = node.lock().unwrap().handle_msg(msg.as_str()) {
                        log::error!("failed to handle message: {e}");
                    }
                });
            }
        });

        // need to start another thread that runs the node swim gossip
        log::info!("starting listener loop");
        loop {
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            log::info!("message: {input}");
            if let Err(e) = tx.send(input) {
                log::error!("{e}");
            }
        }
    }
}
