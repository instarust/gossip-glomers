use std::{collections::HashMap, hash::Hash, io};
use serde::{Deserialize, Serialize};
use serde_json;
use std::sync::mpsc;
use std::thread;
use log::{debug, error, info};
use serde_json::{Value,json};


#[derive(Deserialize, Serialize, Debug)]
struct Message { // i should have a type for each message, not all
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "message")]
    value: u64,
    messages: Vec<u8>,
    topology: HashMap<String, Vec<String>>
}

// will it print fields that are default or empty ?
impl Message {
    fn new(kind: String, value: u64) -> Self {
        let messages = Vec::new();
        let topology = HashMap::new();
        Message { kind, value, messages, topology }
    }
}
struct Node {
    values: Vec<u64>,
}

impl Node {
    fn new() -> Self {
        Node{
            values: Vec::new()
        }
    }

    fn send(msg: &Message) -> io::Result<()> {
        let json_str =  serde_json::to_string(msg)?;
        print!("{}", json_str);
        Ok(())
    }

    // make this return a potential error, and handle this error
    fn handle_msg(&mut self, raw_msg: String) -> Result<(),String>{
        let msg: Message = match serde_json::from_str(&raw_msg) {
            Ok(m) => m,
            Err(e) => {
                return Err(e.to_string());
            }
        };
        
        if msg.kind == "broadcast" {
            self.values.push(msg.value);
        }
        if msg.kind == "topology" {

        };
        if msg.kind == "read" {
            // Build the JSON value
            let message_json = json!({
                "type": "read_ok",
            });
        
            // Serialize to string if needed
            let message_str = serde_json::to_string(&message_json)
                .map_err(|e| e.to_string())?;
        

            match serde_json::from_str::<Message>(&message_str) {
                Ok(m) => {
                    // don't use the result
                    let _ = Node::send(&m).map_err(|e| e.to_string());
                }
                Err(e) => {
                    return Err(e.to_string());
                }
            }
        }
        Ok(())
    }
}


#[tokio::main]
async fn main() -> io::Result<()> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut node = Node::new();
        for msg in rx {
            match node.handle_msg(msg) {
                Ok(())=>{}
                Err(e) =>{error!("failed to handle message: {}", e);}
            }
        }
    });

    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        match tx.send(input) {
            Ok(()) => {}
            Err(e) => {
                error!("{}", e);
            }
        };
    }
}



