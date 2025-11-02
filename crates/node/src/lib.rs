use std::collections::HashMap;
use std::sync::Mutex;

use once_cell::sync::Lazy;

// Module declarations
pub mod handlers;
pub mod messaging;
pub mod server;
pub mod types;

pub use handlers::{FnHandler, HandlersMap, build_default_handlers};
pub use messaging::{handle_msg, listen, send_synchronous, serve};
pub use server::Server;
pub use types::{Message, Node, SequentialKV};

// Global callback store
// ammar: not sure about the global variable thing, maybe bring it back into the types ?
type CallbackStore = Lazy<Mutex<HashMap<u64, Box<dyn FnOnce() + Send + Sync>>>>;

pub static CALLBACKS: CallbackStore = std::sync::LazyLock::new(|| HashMap::new());
