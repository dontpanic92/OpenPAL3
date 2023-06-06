use std::io::{BufReader, Read, Write};

use anyhow::{anyhow, Context};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use interprocess::local_socket::LocalSocketStream;
use serde::{Deserialize, Serialize};

use super::ScriptModule;

pub(super) struct DebugIpcClient {
    stream: anyhow::Result<BufReader<LocalSocketStream>>,
    id: usize,
}

impl DebugIpcClient {
    pub fn new() -> Self {
        let stream = LocalSocketStream::connect("asdebug-localsocket")
            .context("Connect failed")
            .map(BufReader::new);

        Self { stream, id: 0 }
    }

    pub fn notify(&mut self, notification: Notification) -> anyhow::Result<()> {
        if self.stream.is_err() {
            return Err(anyhow!("Connection not established"));
        }

        let msg = Message::Notification(notification);
        self.send(msg)
    }

    pub fn call(&mut self, request: Request) -> anyhow::Result<Response> {
        if self.stream.is_err() {
            return Err(anyhow!("Connection not established"));
        }

        let msg = Message::Request {
            id: self.next_id(),
            content: request,
        };

        self.send(msg)?;

        let response: Message = self.read()?;
        if let Message::Response { id: _, content } = response {
            Ok(content)
        } else {
            Err(anyhow!("Expect Response"))
        }
    }

    fn send(&mut self, msg: Message) -> anyhow::Result<()> {
        debug_ipc_write(self.stream.as_mut().unwrap().get_mut(), msg)
    }

    fn read(&mut self) -> anyhow::Result<Message> {
        debug_ipc_read(self.stream.as_mut().unwrap())
    }

    fn next_id(&mut self) -> usize {
        if self.id == usize::MAX {
            self.id = 0;
        } else {
            self.id += 1;
        }

        self.id
    }
}

pub fn debug_ipc_read(reader: &mut dyn Read) -> anyhow::Result<Message> {
    let len = reader.read_u32::<LittleEndian>()?;

    let mut buffer = vec![0u8; len as usize];
    reader.read_exact(&mut buffer)?;

    let s = String::from_utf8(buffer)?;
    let msg = serde_json::from_str(&s)?;

    Ok(msg)
}

pub fn debug_ipc_write(writer: &mut dyn Write, msg: Message) -> anyhow::Result<()> {
    let buf = serde_json::to_string(&msg)?;
    writer.write_u32::<LittleEndian>(buf.as_bytes().len() as u32)?;
    writer.write(buf.as_bytes())?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
pub enum Message {
    Request { id: usize, content: Request },
    Response { id: usize, content: Response },
    Notification(Notification),
}

#[derive(Serialize, Deserialize)]
pub enum Request {
    WaitForAction,
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    SingleStep,
}

#[derive(Serialize, Deserialize)]
pub enum Notification {
    ModuleChanged {
        module: ScriptModule,
        function: u32,
    },
    StackChanged(Vec<u8>),
    RegisterChanged {
        pc: usize,
        sp: usize,
        fp: usize,
        r1: u32,
        r2: u32,
        object_register: usize,
    },
    ObjectsChanged(Vec<Option<String>>),
    GlobalFunctionsChanged(Vec<String>),
}
