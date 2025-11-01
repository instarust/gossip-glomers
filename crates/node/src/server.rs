use std::any::Any;
use std::collections::HashSet;
use std::io::{self, Write};

use crate::types::Message;

pub trait Server {
    fn get_id(&self) -> String;

    fn get_topology(&self) -> Vec<String>;

    fn get_msg_count(&self) -> u64;

    fn set_id(&mut self, id: String);

    fn set_topology(&mut self, topo: HashSet<String>);

    fn set_msg_count(&mut self, count: u64);

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// # Errors
    /// - forwards `serde_json` errors
    /// - forwards `io` errors
    fn send(&self, msg: &Message) -> io::Result<()> {
        let json_str = serde_json::to_string(msg)?;
        let mut writer = std::io::stdout();
        writer.write_all(json_str.as_bytes())?;
        writer.write_all(b"\n")?;
        Ok(())
    }

    fn build_reply(
        &self,
        r#type: &str,
        msg: &Message,
        mut body: serde_json::Value,
    ) -> Option<Message> {
        let Some(msg_id) = msg.body["msg_id"].as_u64() else {
            log::error!("couldn't construct reply to `{msg:?}`: missing `.body.msg_id` field");
            return None;
        };

        body["type"] = r#type.into();
        body["in_reply_to"] = msg_id.into();

        Some(Message {
            src: self.get_id(),
            dest: msg.src.clone(),
            body,
        })
    }
}

impl dyn Server {
    pub fn downcast_ref<T: Server + 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }

    pub fn downcast_mut<T: Server + 'static>(&mut self) -> Option<&mut T> {
        self.as_any_mut().downcast_mut::<T>()
    }
}

// it really does, right ?
macro_rules! impl_server {
    ($t:ty $(, { $($body:tt)* })?) => {
        impl Server for $t {
            fn get_id(&self) -> String {
                self.id.clone()
            }

            fn get_topology(&self) -> Vec<String> {
                self.topology.iter().cloned().collect()
            }

            fn get_msg_count(&self) -> u64 {
                self.msg_count
            }

            fn set_id(&mut self, id: String) {
                self.id = id;
            }

            fn set_topology(&mut self, topo: HashSet<String>) {
                self.topology = topo;
            }

            fn set_msg_count(&mut self, count: u64) {
                self.msg_count = count;
            }

            fn as_any(&self) -> &dyn Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn Any {
                self
            }

            $($($body)*)?
        }
    };
}

impl_server!(crate::types::Node);

impl_server!(crate::types::SequentialKV);
