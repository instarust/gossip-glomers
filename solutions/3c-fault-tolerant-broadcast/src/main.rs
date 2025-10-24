use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Write, Stdout};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use log::{info, error};
use serde_json::{json, Value};
use std::sync::{Arc,Mutex};

struct Empty;

type NodeFn = Box<dyn Fn(&mut Node, Value) + Send + Sync>;

struct Node {
    id: String,
    stdout: RefCell<Stdout>,
    values: HashMap<u64, Empty>,
    topology: RefCell<HashMap<String, bool>>,
    msg_count: u64,
    handlers: HashMap<String, NodeFn>,
}

fn build_handlers_map() -> HashMap<String, NodeFn> {
    let mut handlers: HashMap<String, Box<dyn Fn(&mut Node, Value) + Send + Sync>> =  HashMap::new();
    handlers.insert("broadcast".to_string(), Box::new(|node, msg| {   
        // check if i had this value before, if i did. do nothing at all
        // if i didn't add it to the hashmap and send it to the others
        let number = msg["body"]["message"].as_u64().unwrap();
        if let Some(_) = node.values.get(&number) {
            return
        }

        node.values.insert(number, Empty);
        // send it to everyone else
        for n in node.topology.borrow().keys() {
            if *n == msg["src"] || *n == node.id {
                continue
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
        let _ = node.send(&node.build_reply(msg["src"].as_str().unwrap(),&msg, &mut reply)).map_err(|e| e.to_string());
    }));
    handlers.insert("init".to_string(), Box::new(|node, msg| {
        if !node.id.is_empty() {
            return
        }
        node.id = msg["body"]["node_id"].as_str().unwrap().to_string();    
        let mut reply = json!({
            "type":"init_ok",
        });
        let _ = node.send(&node.build_reply(msg["src"].as_str().unwrap(), &msg, &mut reply)).map_err(|e| e.to_string());
    }));
    handlers.insert("topology".to_string(), Box::new(|node, msg| {
        let topo = msg["body"]["topology"].as_object().unwrap();
        for k in topo.keys() {
            node.topology.borrow_mut().insert(k.to_string(), true);
        }
        let mut reply = json!({
            "type": "topology_ok",
        });
        let _ = node.send(&node.build_reply(msg["src"].as_str().unwrap(),&msg, &mut reply)).map_err(|e| e.to_string());
    }));
    handlers.insert("read".to_string(), Box::new(|node, msg| {      
        let mut all_values: Vec<u64> = Vec::new();
        for val in node.values.keys() {
            all_values.push(*val);
        }
        let mut reply = json!({
        "type": "read_ok",
        "messages": all_values,
    });
    let _ = node.send(&node.build_reply(msg["src"].as_str().unwrap(), &msg,&mut reply)).map_err(|e| e.to_string());
    }));
    handlers
}

impl Node {
    fn new(stdout: Stdout) -> Self {
        Node{
            id: String::new(),
            stdout: RefCell::new(stdout),
            values: HashMap::new(),
            msg_count: 0,
            topology: RefCell::new(HashMap::new()),
            handlers: build_handlers_map(),
        }
    }

    fn ping(&mut self, dest: String) {
        let ping_msg = json!({
            "src": self.id,
            "dest": dest,
            "msg_id": self.msg_count,
            "body": {
                "type": "ping"
            }
        });
        let _ = self.send(&ping_msg).map_err(|e| e.to_string());
        self.msg_count += 1;
        let mut callbacks: HashMap<u64, Box<dyn Fn()>> = HashMap::new();
        let (tx, rx) = mpsc::channel();

        callbacks.insert(
            self.msg_count,
            Box::new(move || {
                let r = Empty;
                if let Err(e) = tx.send(r) {
                    error!("{}", e)
                }
            })
        );
        
        /*
        - send a ping mesage to some node (ping will include a message number)
        - create a channel
        - create a callback -> write to that channel
        - create a function that checks incoming messages and if it does contain the message id, check the callback map and run the call back
        - wait for a while, if the write comes back declare ping successfull
        */

        /* how to act on failure ?
        given that: the node doesn't encurr a full crash, just a network partition, it still has its values. any node can receive messages
        */
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(_) => {
                // declare that this node that i pinged is healthy
                self.topology.borrow_mut().insert(dest, true);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // declare that the node wasn't healthy
                self.topology.borrow_mut().insert(dest, false);
            }
            _ => {}
        }
    }

    fn run(node: Arc<Mutex<Self>>) {
        thread::spawn(move || {
            loop {
                let mut n = node.lock().unwrap();
                // pick a random node
                n.ping("".to_string())
            }
        });
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

    fn handle_msg(&mut self, raw_msg: String) -> Result<(), String> {
        let msg: Value = serde_json::from_str(&raw_msg)
            .map_err(|e| e.to_string())?;

        let h;
        let msg_type = msg["body"]["type"].as_str().unwrap();
        if let Some(handler) = self.handlers.remove(msg_type) {
            h = handler;
        } else {
            return Err(format!("handler {} not found", msg["body"]["type"]));
        }
        h(self, msg.clone());
        self.handlers.insert(msg_type.to_string(), h);
        Ok(())
    }
}



fn main() -> io::Result<()> {
    env_logger::init();
    let (tx, rx) = mpsc::channel();
    let stdin = io::stdin();
    let stdout = io::stdout();

    let node = Arc::new(Mutex::new(Node::new(stdout)));

    // this thread can be made to own the node, again. allowing for avoiding locking the entire 
    thread::spawn(move || {
        info!("starting message thread");
        for msg in rx {
            let node_copy = node.clone();
                thread::spawn(move || {
                    if let Err(e) = node_copy.lock().unwrap().handle_msg(msg) {
                        error!("failed to handle message: {}", e);
                    }
                });
            }
    });

    // info!("starting node main threap");
    // Node::run(node);

    // need to start another thread that runs the node swim gossip
    info!("starting listener loop");
    loop {
        let mut input = String::new();
        stdin.read_line(&mut input)?;
        info!("message: {}", input);
        if let Err(e) = tx.send(input) {
            error!("{}", e);
        }
    }
}



