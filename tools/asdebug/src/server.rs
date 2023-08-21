use std::{
    io::BufReader,
    sync::{mpsc::Receiver, Arc, RwLock},
    thread::spawn,
};

use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use shared::scripting::angelscript::debug::{
    debug_ipc_read, debug_ipc_write, Message, Notification, Request, Response,
};

use crate::context::{Context, DebuggeeState, ServerConnectionState};

pub fn start_server(rx: Receiver<Response>, context: Arc<RwLock<Context>>) {
    let listener = match LocalSocketListener::bind("asdebug-localsocket") {
        Ok(l) => l,
        Err(e) => {
            context.write().unwrap().connection_state = ServerConnectionState::Error(e.to_string());
            return;
        }
    };

    let _ = spawn(move || {
        server_thread(listener, rx, context);
    });
}

fn server_thread(
    listener: LocalSocketListener,
    rx: Receiver<Response>,
    context: Arc<RwLock<Context>>,
) {
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

    loop {
        let msg = debug_ipc_read(&mut reader);
        match msg {
            Ok(m) => handle_message(m, &mut reader, &rx, context.clone()),
            Err(e) => {
                context.write().unwrap().connection_state =
                    ServerConnectionState::Error(e.to_string())
            }
        }

        context.read().unwrap().request_repaint();
    }
}

fn handle_message(
    msg: Message,
    conn: &mut BufReader<LocalSocketStream>,
    rx: &Receiver<Response>,
    context: Arc<RwLock<Context>>,
) {
    match msg {
        Message::Notification(Notification::ModuleChanged { module, function }) => {
            let mut c = context.write().unwrap();
            c.module = Some(module);
            c.function_id = function;
        }
        Message::Notification(Notification::ObjectsChanged(obj)) => {
            let mut c = context.write().unwrap();
            c.objects = obj;
        }
        Message::Notification(Notification::StackChanged(stack)) => {
            let mut c = context.write().unwrap();
            c.stack = stack;
        }
        Message::Notification(Notification::RegisterChanged {
            pc,
            sp,
            fp,
            r1,
            r2,
            object_register,
        }) => {
            let mut c = context.write().unwrap();
            c.pc = pc;
            c.sp = sp;
            c.fp = fp;
            c.r1 = r1;
            c.r2 = r2;
            c.object_register = object_register;
        }
        Message::Notification(Notification::GlobalFunctionsChanged(functions)) => {
            let mut c = context.write().unwrap();
            c.functions = functions;
        }
        Message::Request { id, content } => handle_request(id, content, conn, rx, context.clone()),
        Message::Response { id: _, content: _ } => {}
    }
}

fn handle_request(
    id: usize,
    request: Request,
    conn: &mut BufReader<LocalSocketStream>,
    rx: &Receiver<Response>,
    context: Arc<RwLock<Context>>,
) {
    match request {
        Request::WaitForAction => {
            context.write().unwrap().state = DebuggeeState::WaitForAction;
            context.write().unwrap().request_repaint();

            let response = rx.recv().unwrap();
            let _ = debug_ipc_write(
                conn.get_mut(),
                Message::Response {
                    id,
                    content: response,
                },
            );

            context.write().unwrap().state = DebuggeeState::Running;
        }
    }
}
