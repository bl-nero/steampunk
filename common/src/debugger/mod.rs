mod protocol;

use crate::debugger::protocol::parse_request;
use crate::debugger::protocol::raw_messages;
use crate::debugger::protocol::send_raw_message;
use crate::debugger::protocol::serialize_response;
use crate::debugger::protocol::Request;
use crate::debugger::protocol::Response;
use debugserver_types::InitializeResponse;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::mpsc::SendError;
use std::sync::mpsc::TryRecvError;
use std::thread;
use thiserror::Error;

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
            Err(DebugAdapterError::ReceiveRequestError(TryRecvError::Empty)) => {} // Ignore
            other => println!("{:?}", other),
        }
    }
}

#[derive(Error, Debug)]
pub enum DebugAdapterError {
    #[error("Unable to retrieve request from debugger adapter: {0}")]
    ReceiveRequestError(#[from] TryRecvError),

    #[error("Unable to send response to debugger adapter: {0}")]
    UnsupportedMessageType(#[from] SendError<WriterThreadEvent>),
}

/// Uses Debug Adapter Protocol over a TCP socket to communicate to a debugger
/// UI. The adapter spawns two threads internally — one to read, and one to
/// write to the TCP port — and communicates with them over `mpsc` channels. The
/// adapter doesn't expose a blocking interface, as it's supposed to be consumed
/// in the emulator's update loop anyway.
///
/// One important limitation is that only a single TCP connection is allowed at
/// any given time, but connecting with two debuggers at once would be a bad
/// idea anyway.
pub struct DebugAdapter {
    writer_event_sender: mpsc::Sender<WriterThreadEvent>,
    request_receiver: mpsc::Receiver<Request>,
}

impl DebugAdapter {
    /// Creates a new `DebugAdapter` and starts listening on given port.
    pub fn new(port: u16) -> Self {
        let writer_event_sender = spawn_writer_thread();
        let request_receiver = spawn_reader_thread(port, writer_event_sender.clone());
        Self {
            writer_event_sender,
            request_receiver,
        }
    }

    /// Attempts to receive a request from the debugger UI. Returns immediately
    /// with [`DebugAdapterError::TryRecvError(TryRecvError::Empty)`] if there
    /// are no pending requests.
    pub fn try_receive_request(&self) -> Result<Request, DebugAdapterError> {
        self.request_receiver.try_recv().map_err(|e| e.into())
    }

    pub fn send_response(&self, response: Response) -> Result<(), DebugAdapterError> {
        self.writer_event_sender
            .send(WriterThreadEvent::DebuggerResponse(response))
            .map_err(|e| e.into())
    }
}

/// Spawns a reader thread that listens, repeatedly accepts and handles TCP
/// connections.
fn spawn_reader_thread(
    port: u16,
    writer_event_sender: mpsc::Sender<WriterThreadEvent>,
) -> mpsc::Receiver<Request> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("debugger reader thread".into())
        .spawn(move || {
            let address = SocketAddr::from(([127, 0, 0, 1], port));
            let listener = TcpListener::bind(address).expect("Unable to listen for a debugger");
            eprintln!("Listening for a debugger at {}...", address);
            loop {
                let (connection, address) = listener.accept().unwrap();
                eprintln!("Debugger connection accepted from {}", address);
                writer_event_sender
                    .send(WriterThreadEvent::Connected(
                        connection.try_clone().unwrap(),
                    ))
                    .unwrap();
                handle_input(connection, &tx);
                writer_event_sender
                    .send(WriterThreadEvent::Disconnected)
                    .unwrap();
            }
        })
        .expect("Unable to start the debugger reader thread");
    return rx;
}

pub enum WriterThreadEvent<W: Write = TcpStream> {
    DebuggerResponse(Response),
    Connected(W),
    Disconnected,
}

fn handle_writer_events<W: Write>(events: impl IntoIterator<Item = WriterThreadEvent<W>>) {
    let mut stream = None;
    for event in events {
        match event {
            WriterThreadEvent::Connected(new_stream) => stream = Some(new_stream),
            WriterThreadEvent::DebuggerResponse(response) => {
                if let Some(ref mut stream) = stream {
                    let raw_message = serialize_response(&response).unwrap();
                    send_raw_message(raw_message, stream).unwrap();
                }
            }
            WriterThreadEvent::Disconnected => stream = None,
        }
    }
}

fn spawn_writer_thread() -> mpsc::Sender<WriterThreadEvent> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("debugger writer thread".into())
        .spawn(|| handle_writer_events(rx))
        .expect("Unable to spawn the debugger writer thread");
    return tx;
}

fn handle_input(input: impl Read, sender: &mpsc::Sender<Request>) {
    let mut reader = BufReader::new(input);
    raw_messages(&mut reader)
        .map(Result::unwrap)
        .map(parse_request)
        .map(Result::unwrap)
        .for_each(|request| sender.send(request).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debugger::protocol::ProtocolResult;
    use debugserver_types::DisconnectArguments;
    use debugserver_types::DisconnectRequest;
    use debugserver_types::InitializeRequest;
    use debugserver_types::InitializeRequestArguments;
    use debugserver_types::NextResponse;
    use std::fs::File;
    use std::path::Path;

    #[test]
    fn receives_messages() {
        let (tx, rx) = mpsc::channel();
        let stream = File::open(
            Path::new("src")
                .join("debugger")
                .join("test_data")
                .join("session_dump.txt"),
        )
        .unwrap();
        handle_input(stream, &tx);

        // Receive 2 messages.
        assert_matches!(
            rx.try_recv(),
            Ok(Request::Initialize(InitializeRequest {
                arguments: InitializeRequestArguments {
                    client_id: Some(ref client_id),
                    ref adapter_id,
                    ..
                },
                ..
            })) if client_id == "vscode" && adapter_id == "steampunk-6502"
        );
        assert_matches!(
            rx.try_recv(),
            Ok(Request::Disconnect(DisconnectRequest {
                arguments: Some(DisconnectArguments {
                    restart: Some(false),
                    ..
                }),
                ..
            }))
        );

        // Stop at the 3rd one: end of stream.
        assert_eq!(rx.try_recv().is_err(), true);
    }

    fn response_with_seq(seq: i64) -> Response {
        Response::Next(NextResponse {
            type_: "response".into(),
            request_seq: 1,
            success: true,
            command: "next".into(),
            seq,
            body: None,
            message: None,
        })
    }

    fn into_json_value(raw_message_result: ProtocolResult<Vec<u8>>) -> serde_json::Value {
        serde_json::from_slice(&raw_message_result.unwrap()).unwrap()
    }

    fn message_seq_numbers_from_stream(stream: Vec<u8>) -> Vec<i64> {
        let mut stream_reader = stream.as_slice();
        raw_messages(&mut stream_reader)
            .map(into_json_value)
            .map(|resp| resp["seq"].as_i64().unwrap())
            .collect()
    }

    #[test]
    fn write_thread_handles_events() {
        use WriterThreadEvent::*;
        let mut stream = vec![];
        let events = vec![
            Connected(&mut stream),
            DebuggerResponse(response_with_seq(4)),
            DebuggerResponse(response_with_seq(5)),
        ];

        handle_writer_events(events);

        // Instead of inspecting the stream, which would be fragile and depend
        // on Serde implementation details, we'll parse the output and compare
        // it with the original message.
        assert_eq!(message_seq_numbers_from_stream(stream), vec![4, 5]);
    }

    #[test]
    fn write_thread_ignores_events_between_connections() {
        use WriterThreadEvent::*;
        let mut stream1 = vec![];
        let mut stream2 = vec![];
        let events = vec![
            DebuggerResponse(response_with_seq(1)),
            DebuggerResponse(response_with_seq(2)),
            Connected(&mut stream1),
            DebuggerResponse(response_with_seq(3)),
            DebuggerResponse(response_with_seq(4)),
            Disconnected,
            DebuggerResponse(response_with_seq(5)),
            DebuggerResponse(response_with_seq(6)),
            Connected(&mut stream2),
            DebuggerResponse(response_with_seq(7)),
            DebuggerResponse(response_with_seq(8)),
        ];

        handle_writer_events(events);

        assert_eq!(message_seq_numbers_from_stream(stream1), vec![3, 4]);
        assert_eq!(message_seq_numbers_from_stream(stream2), vec![7, 8]);
    }
}
