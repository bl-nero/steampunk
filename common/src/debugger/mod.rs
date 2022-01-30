pub mod adapter;
mod protocol;

use crate::debugger::adapter::DebugAdapter;
use crate::debugger::adapter::DebugAdapterError;
use crate::debugger::protocol::Request;
use crate::debugger::protocol::Response;
use debugserver_types::AttachResponse;
use debugserver_types::InitializeResponse;
use std::sync::mpsc::TryRecvError;

/// A debugger for 6502-based machines. Uses Debug Adapter Protocol internally
/// to communicate with a debugger UI.
pub struct Debugger {
    adapter: DebugAdapter,
}

impl Debugger {
    pub fn new(adapter: DebugAdapter) -> Self {
        Self { adapter }
    }
    pub fn process_meessages(&self) {
        match self.adapter.try_receive_request() {
            Ok(Request::Initialize(req)) => self
                .adapter
                .send_response(Response::Initialize(InitializeResponse {
                    seq: 1,
                    request_seq: req.seq,
                    type_: "response".into(),
                    command: "initialize".into(),
                    success: true,
                    message: None,
                    body: None,
                }))
                .unwrap(),
            Ok(Request::Attach(req)) => self
                .adapter
                .send_response(Response::Attach(AttachResponse {
                    seq: 2,
                    request_seq: req.seq,
                    type_: "response".into(),
                    command: "attach".into(),
                    success: true,
                    message: None,
                    body: None,
                }))
                .unwrap(),
            Err(DebugAdapterError::ReceiveRequestError(TryRecvError::Empty)) => {} // Ignore
            other => println!("{:?}", other),
        }
    }
}
