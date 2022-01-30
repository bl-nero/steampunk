pub mod adapter;
mod protocol;

use crate::debugger::adapter::DebugAdapter;
use crate::debugger::adapter::DebugAdapterError;
use crate::debugger::protocol::IncomingMessage;
use crate::debugger::protocol::OutgoingMessage;
use debugserver_types::AttachResponse;
use debugserver_types::InitializeResponse;
use debugserver_types::StoppedEvent;
use debugserver_types::StoppedEventBody;
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
        match self.adapter.try_receive_message() {
            Ok(IncomingMessage::Initialize(req)) => self
                .adapter
                .send_message(OutgoingMessage::Initialize(InitializeResponse {
                    seq: 1,
                    request_seq: req.seq,
                    type_: "response".into(),
                    command: "initialize".into(),
                    success: true,
                    message: None,
                    body: None,
                }))
                .unwrap(),
            Ok(IncomingMessage::Attach(req)) => {
                self.adapter
                    .send_message(OutgoingMessage::Attach(AttachResponse {
                        seq: 2,
                        request_seq: req.seq,
                        type_: "response".into(),
                        command: "attach".into(),
                        success: true,
                        message: None,
                        body: None,
                    }))
                    .unwrap();
                self.adapter
                    .send_message(OutgoingMessage::Stopped(StoppedEvent {
                        seq: 3,
                        type_: "event".into(),
                        event: "stopped".into(),
                        body: StoppedEventBody {
                            reason: "entry".into(),
                            description: None,
                            thread_id: None,
                            preserve_focus_hint: None,
                            text: None,
                            all_threads_stopped: None,
                        },
                    }))
                    .unwrap();
            }
            Err(DebugAdapterError::TryRecvError(TryRecvError::Empty)) => {} // Ignore
            other => println!("{:?}", other),
        }
    }
}
