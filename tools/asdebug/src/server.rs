use std::{
    io::{BufReader, Read},
    sync::{Arc, Mutex, RwLock},
    thread::spawn,
};

use interprocess::local_socket::LocalSocketListener;

use crate::context::{Context, ServerConnectionState};

pub fn start_server(context: Arc<RwLock<Context>>) {
    let listener = match LocalSocketListener::bind("asdebug-localsocket") {
        Ok(l) => l,
        Err(e) => {
            context.write().unwrap().connection_state = ServerConnectionState::Error(e.to_string());
            return;
        }
    };

    let _ = spawn(move || {
        server_thread(listener, context);
    });
}

fn server_thread(listener: LocalSocketListener, context: Arc<RwLock<Context>>) {
    let conn = match listener.accept() {
        Ok(c) => {
            context.write().unwrap().connection_state = ServerConnectionState::Connected;
            c
        }

        Err(_) => {
            context.write().unwrap().connection_state =
                ServerConnectionState::Error("Error accepting connection".to_string());
            return;
        }
    };

    context.read().unwrap().request_repaint();

    let mut reader = BufReader::new(conn);
    match server_read(&mut reader) {
        Ok(()) => {}
        Err(e) => {
            context.write().unwrap().connection_state = ServerConnectionState::Error(e.to_string())
        }
    }
}

fn server_read(reader: &mut dyn Read) -> anyhow::Result<()> {
    let mut buffer = Vec::with_capacity(128);
    reader.read(&mut buffer)?;

    Ok(())
}
