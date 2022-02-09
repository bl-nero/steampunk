use crate::debugger::protocol::parse_message;
use crate::debugger::protocol::raw_messages;
use crate::debugger::protocol::send_raw_message;
use crate::debugger::protocol::serialize_message;
use crate::debugger::protocol::IncomingMessage;
use crate::debugger::protocol::OutgoingMessage;
use crate::debugger::protocol::ParseError;
use crate::debugger::protocol::ProtocolError;
use std::error::Error;
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

/// A generic trait for debug adapter. It's an object that connects the debugger
/// to a debugger UI.
pub trait DebugAdapter {
    /// Attempts to receive a message from the debugger UI. Returns immediately
    /// with [`DebugAdapterError::TryRecvError(TryRecvError::Empty)`] if there
    /// are no pending messages.
    fn try_receive_message(&self) -> DebugAdapterResult<IncomingMessage>;
    fn send_message(&self, message: OutgoingMessage) -> DebugAdapterResult<()>;
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
pub struct TcpDebugAdapter {
    writer_event_sender: mpsc::Sender<WriterThreadCommand>,
    message_receiver: mpsc::Receiver<IncomingMessage>,
}

impl TcpDebugAdapter {
    /// Creates a new `TcpDebugAdapter` and starts listening on given port.
    pub fn new(port: u16) -> Self {
        let writer_event_sender = spawn_writer_thread();
        let message_receiver = spawn_reader_thread(port, writer_event_sender.clone());
        Self {
            writer_event_sender,
            message_receiver,
        }
    }
}

impl DebugAdapter for TcpDebugAdapter {
    fn try_receive_message(&self) -> DebugAdapterResult<IncomingMessage> {
        self.message_receiver.try_recv().map_err(|e| e.into())
    }

    fn send_message(&self, message: OutgoingMessage) -> DebugAdapterResult<()> {
        self.writer_event_sender
            .send(WriterThreadCommand::SendMessage(message))
            .map_err(|e| e.into())
    }
}

pub type DebugAdapterResult<T> = Result<T, DebugAdapterError>;

#[derive(thiserror::Error, Debug)]
pub enum DebugAdapterError {
    #[error("Unable to retrieve message from debugger adapter: {0}")]
    TryRecvError(#[from] TryRecvError),

    #[error("Unable to send message to debugger adapter: {0}")]
    UnsupportedMessageType(#[from] SendError<WriterThreadCommand>),
}

/// Spawns a reader thread that listens, repeatedly accepts and handles TCP
/// connections.
fn spawn_reader_thread(
    port: u16,
    writer_event_sender: mpsc::Sender<WriterThreadCommand>,
) -> mpsc::Receiver<IncomingMessage> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("debugger reader thread".into())
        .spawn(move || {
            let address = SocketAddr::from(([127, 0, 0, 1], port));
            let listener = TcpListener::bind(address).expect("Unable to listen for a debugger");
            eprintln!("Listening for a debugger at {}...", address);
            loop {
                // Note: For sure, there are some errors that are retriable
                // here, but whatever, this is not a "five nines" server.
                let (connection, address) =
                    listener.accept().expect("Unable to accept a connection");
                eprintln!("Debugger connection accepted from {}", address);
                if let Err(e) = handle_connection(connection, &writer_event_sender, &tx) {
                    eprintln!("Debugger connection error: {}", e);
                }
            }
        })
        .expect("Unable to start the debugger reader thread");
    return rx;
}

fn handle_connection(
    connection: TcpStream,
    writer_event_sender: &mpsc::Sender<WriterThreadCommand>,
    incoming_message_sender: &mpsc::Sender<IncomingMessage>,
) -> Result<(), Box<dyn Error>> {
    let connection_for_writer = connection.try_clone()?;
    writer_event_sender.send(WriterThreadCommand::Connect(connection_for_writer))?;
    handle_input(connection, &incoming_message_sender)?;
    writer_event_sender.send(WriterThreadCommand::Disconnect)?;
    Ok(())
}

#[derive(thiserror::Error, Debug)]
enum InputHandlingError {
    #[error("Protocol error: {0}")]
    ProtocolError(#[from] ProtocolError),

    #[error("Message parsing error: {0}")]
    ParseError(#[from] ParseError),

    #[error("Error while sending message to the main thread: {0}")]
    SendError(#[from] SendError<IncomingMessage>),
}

fn handle_input(
    input: impl Read,
    sender: &mpsc::Sender<IncomingMessage>,
) -> Result<(), InputHandlingError> {
    let mut reader = BufReader::new(input);
    for raw_message_result in raw_messages(&mut reader) {
        let message = parse_message(raw_message_result?)?;
        sender.send(message)?;
    }
    Ok(())
}

#[derive(Clone)]
pub enum WriterThreadCommand<W: Write = TcpStream> {
    SendMessage(OutgoingMessage),
    Connect(W),
    Disconnect,
}

fn spawn_writer_thread() -> mpsc::Sender<WriterThreadCommand> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("debugger writer thread".into())
        .spawn(|| handle_writer_events(rx))
        .expect("Unable to spawn the debugger writer thread");
    return tx;
}

fn handle_writer_events<W: Write>(events: impl IntoIterator<Item = WriterThreadCommand<W>>) {
    let mut stream = None;
    for event in events {
        match event {
            WriterThreadCommand::Connect(new_stream) => stream = Some(new_stream),
            WriterThreadCommand::SendMessage(message) => {
                if let Some(ref mut stream) = stream {
                    let raw_message = serialize_message(&message).unwrap();
                    send_raw_message(raw_message, stream).unwrap();
                }
            }
            WriterThreadCommand::Disconnect => stream = None,
        }
    }
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
    use std::assert_matches::assert_matches;
    use std::fs;
    use std::path::Path;

    fn response_with_seq(seq: i64) -> OutgoingMessage {
        OutgoingMessage::Next(NextResponse {
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

    fn read_session_dump() -> Vec<u8> {
        fs::read(
            Path::new("src")
                .join("debugger")
                .join("test_data")
                .join("session_dump.txt"),
        )
        .unwrap()
    }

    #[test]
    fn receives_messages() {
        let (tx, rx) = mpsc::channel();
        let stream = read_session_dump();
        handle_input(&stream[..], &tx).unwrap();

        // Receive 2 messages.
        assert_matches!(
            rx.try_recv(),
            Ok(IncomingMessage::Initialize(InitializeRequest {
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
            Ok(IncomingMessage::Disconnect(DisconnectRequest {
                arguments: Some(DisconnectArguments {
                    restart: Some(false),
                    ..
                }),
                ..
            }))
        );

        // Stop at the 3rd one: end of stream.
        rx.try_recv().unwrap_err();
    }

    #[test]
    fn stops_on_protocol_errors() {
        let (tx, rx) = mpsc::channel();
        let session_dump = read_session_dump();
        let stream = session_dump
            .chain("broken message\r\n\r\n".as_bytes())
            .chain(&session_dump[..]);

        let err = handle_input(stream, &tx).unwrap_err();
        assert_matches!(err, InputHandlingError::ProtocolError(_));

        rx.try_recv().unwrap(); // Ignore the first message.
        rx.try_recv().unwrap(); // Ignore the second message.
        rx.try_recv().unwrap_err(); // Stop at the 3rd one: end of stream.
    }

    #[test]
    fn stops_on_parse_errors() {
        let (tx, rx) = mpsc::channel();
        let session_dump = read_session_dump();
        let stream = session_dump
            .chain("Content-Length: 3\r\n\r\nfoo".as_bytes())
            .chain(&session_dump[..]);

        let err = handle_input(stream, &tx).unwrap_err();
        assert_matches!(err, InputHandlingError::ParseError(_));

        rx.try_recv().unwrap(); // Ignore the first message.
        rx.try_recv().unwrap(); // Ignore the second message.
        rx.try_recv().unwrap_err(); // Stop at the 3rd one: end of stream.
    }

    #[test]
    fn stops_on_send_errors() {
        let (tx, rx) = mpsc::channel();
        let stream = read_session_dump();

        drop(rx);
        let err = handle_input(&stream[..], &tx).unwrap_err();
        assert_matches!(err, InputHandlingError::SendError(_));
    }

    #[test]
    fn write_thread_handles_events() {
        use WriterThreadCommand::*;
        let mut stream = vec![];
        let events = vec![
            Connect(&mut stream),
            SendMessage(response_with_seq(4)),
            SendMessage(response_with_seq(5)),
        ];

        handle_writer_events(events);

        // Instead of inspecting the stream, which would be fragile and depend
        // on Serde implementation details, we'll parse the output and compare
        // it with the original message.
        assert_eq!(message_seq_numbers_from_stream(stream), vec![4, 5]);
    }

    #[test]
    fn write_thread_ignores_events_between_connections() {
        use WriterThreadCommand::*;
        let mut stream1 = vec![];
        let mut stream2 = vec![];
        let events = vec![
            SendMessage(response_with_seq(1)),
            SendMessage(response_with_seq(2)),
            Connect(&mut stream1),
            SendMessage(response_with_seq(3)),
            SendMessage(response_with_seq(4)),
            Disconnect,
            SendMessage(response_with_seq(5)),
            SendMessage(response_with_seq(6)),
            Connect(&mut stream2),
            SendMessage(response_with_seq(7)),
            SendMessage(response_with_seq(8)),
        ];

        handle_writer_events(events);

        assert_eq!(message_seq_numbers_from_stream(stream1), vec![3, 4]);
        assert_eq!(message_seq_numbers_from_stream(stream2), vec![7, 8]);
    }
}
