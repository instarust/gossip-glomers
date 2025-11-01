use std::io;
use std::sync::{Arc, Mutex};

use tokio::task;

use crate::CALLBACKS;
use crate::handlers::HandlersMap;
use crate::server::Server;
use crate::types::Message;

/// # Errors
pub async fn listen(tx: tokio::sync::mpsc::Sender<Message>) -> io::Result<()> {
    log::info!("starting listener loop");
    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        log::info!("message: {input}");
        match serde_json::from_str::<Message>(&input) {
            Ok(msg) => {
                if let Err(e) = tx.send(msg).await {
                    log::error!("{e}");
                }
            }
            Err(e) => {
                log::error!("{e}");
            }
        }
    }
}

/// # Errors
/// - forwards `serde_json` errors
/// - returns an error if the message type is not found in the handlers map
/// # Panics
/// - Panics if `msg["body"]["type"]` is not a string
/// - panics if the mutex on `node_arc` is poisoned
pub async fn handle_msg(
    server: Arc<Mutex<dyn Server + Send + Sync + 'static>>,
    handlers_map: &HandlersMap<dyn Server + Send + Sync + 'static>,
    msg: Message,
) -> Result<(), String> {
    {
        if let Some(callback) = msg.body["id"]
            .as_u64()
            .and_then(|id| CALLBACKS.lock().unwrap().remove(&id))
        {
            callback();
        }
    }

    let msg_type = msg.body["type"].as_str().unwrap();
    _ = handlers_map
        .get(msg_type)
        .ok_or(format!("handler {msg_type} not found"))?(server, msg)
    .await;
    Ok(())
}

/// # Errors
/// - forwards `io` errors
pub async fn serve(
    node: Arc<Mutex<dyn Server + Send + Sync + 'static>>,
    handlers: HandlersMap<dyn Server + Send + Sync + 'static>,
) -> io::Result<()> {
    // 10 is an arbitrary value, the size doesn't actually matter (wink, wink)
    let (tx, mut rx) = tokio::sync::mpsc::channel(10);

    tokio::spawn(async move {
        log::info!("starting message thread");
        while let Some(msg) = rx.recv().await {
            let node_mut = node.clone();
            let h = handlers.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_msg(node_mut, &h, msg).await {
                    log::error!("failed to handle message: {e}");
                }
            });
        }
    });

    listen(tx).await
}

/// # Errors
/// - forwards `serde_json` errors
/// - forwards `io` errors
/// # Panics
/// This function will panic if the mutex on `node_arc` is poisoned.
pub fn send_synchronous(
    server_mut: &Arc<Mutex<dyn Server + Send + Sync + 'static>>,
    mut msg: Message,
    message_timout: tokio::time::Duration,
) -> io::Result<()> {
    let mut server = server_mut.lock().unwrap();
    let mut msg_count = server.get_msg_count();
    msg_count += 1;
    server.set_msg_count(msg_count);

    msg.body["id"] = msg_count.into();

    log::info!(
        "{server_id}: using message number {msg_count} for destination {dest}",
        dest = msg.dest,
        server_id = server.get_id()
    );

    let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();

    // register the callback
    {
        CALLBACKS.lock().unwrap().insert(
            msg_count,
            Box::new(move || {
                _ = tx.send(());
            }),
        );
    }

    // initial send
    server.send(&msg)?;

    let server_mut_copy = server_mut.clone();
    task::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut rx => {
                    break;
                },

                () = tokio::time::sleep(message_timout) => {
                    let srv = server_mut_copy.lock().unwrap();
                    log::info!("node {}: receiving a response from {} timed out, sending again", srv.get_id(), msg.dest);
                    if let Err(e) = srv.send(&msg) {
                        log::error!("failed to send echo_ok: {e}");
                    }
                },
            }
        }

        // ammar: add payload identification for different message types
        log::info!("node {dest} received message", dest = msg.dest,);
    });
    Ok(())
}
