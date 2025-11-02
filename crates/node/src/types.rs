use std::collections::{hash_map::DefaultHasher, HashSet};
use std::hash::{Hash, Hasher};

use rand::Rng;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Message {
    pub src: String,
    pub dest: String,
    pub body: serde_json::Value,
}

impl Message {
    #[must_use]
    pub fn hash(&self) -> u64 {
        let mut message_str = String::new();
        message_str.push_str(&self.src);
        message_str.push_str(&self.dest);
        message_str.push_str(&self.body.to_string());
        let mut hasher = DefaultHasher::new();
        message_str.hash(&mut hasher);
        hasher.finish()
    }
}

#[derive(Debug)]
pub struct Node {
    pub id: String,
    pub values: HashSet<u64>,
    pub topology: HashSet<String>,
    pub msg_count: u64,
}

#[derive(Debug)]
pub struct SequentialKV {
    pub counter: u64,
    pub values: HashSet<u64>,
    pub id: String,
    pub topology: HashSet<String>,
    pub msg_count: u64,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            id: String::default(),
            values: HashSet::default(),
            topology: HashSet::default(),
            msg_count: rand::rng().random_range(0..10000),
        }
    }
}

impl Default for SequentialKV {
    fn default() -> Self {
        Self {
            id: String::default(),
            values: HashSet::default(),
            topology: HashSet::default(),
            counter: 0,
            msg_count: rand::rng().random_range(0..10000),
        }
    }
}
