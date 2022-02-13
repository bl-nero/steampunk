use crate::debugger::dap_types::MessageEnvelope;
use crate::debugger::protocol::raw_messages;
use crate::debugger::protocol::send_raw_message;
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
    fn try_receive_message(&self) -> DebugAdapterResult<MessageEnvelope>;
    fn send_message(&self, message: MessageEnvelope) -> DebugAdapterResult<()>;
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
    writer_command_sender: mpsc::Sender<WriterThreadCommand>,
    message_receiver: mpsc::Receiver<MessageEnvelope>,
}

impl TcpDebugAdapter {
    /// Creates a new `TcpDebugAdapter` and starts listening on given port.
    pub fn new(port: u16) -> Self {
        let writer_command_sender = spawn_writer_thread();
        let message_receiver = spawn_reader_thread(port, writer_command_sender.clone());
        Self {
            writer_command_sender,
            message_receiver,
        }
    }
}

impl DebugAdapter for TcpDebugAdapter {
    fn try_receive_message(&self) -> DebugAdapterResult<MessageEnvelope> {
        self.message_receiver.try_recv().map_err(|e| e.into())
    }

    fn send_message(&self, message: MessageEnvelope) -> DebugAdapterResult<()> {
        self.writer_command_sender
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
    SendError(#[from] SendError<WriterThreadCommand>),
}

/// Spawns a reader thread that listens, repeatedly accepts and handles TCP
/// connections.
fn spawn_reader_thread(
    port: u16,
    writer_command_sender: mpsc::Sender<WriterThreadCommand>,
) -> mpsc::Receiver<MessageEnvelope> {
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
                if let Err(e) = handle_connection(connection, &writer_command_sender, &tx) {
                    eprintln!("Debugger connection error: {}", e);
                }
            }
        })
        .expect("Unable to start the debugger reader thread");
    return rx;
}

fn handle_connection(
    connection: TcpStream,
    writer_command_sender: &mpsc::Sender<WriterThreadCommand>,
    incoming_message_sender: &mpsc::Sender<MessageEnvelope>,
) -> Result<(), Box<dyn Error>> {
    let connection_for_writer = connection.try_clone()?;
    writer_command_sender.send(WriterThreadCommand::Connect(connection_for_writer))?;
    handle_input(connection, &incoming_message_sender)?;
    writer_command_sender.send(WriterThreadCommand::Disconnect)?;
    Ok(())
}

#[derive(thiserror::Error, Debug)]
enum InputHandlingError {
    #[error("Protocol error: {0}")]
    ProtocolError(#[from] ProtocolError),

    #[error("Message parsing error: {0}. Original message:\n{1}\n")]
    ParseError(serde_json::Error, String),

    #[error("Error while sending message to the main thread: {0}")]
    SendError(#[from] SendError<MessageEnvelope>),
}

fn handle_input(
    input: impl Read,
    sender: &mpsc::Sender<MessageEnvelope>,
) -> Result<(), InputHandlingError> {
    let mut reader = BufReader::new(input);
    for raw_message_result in raw_messages(&mut reader) {
        let raw_message = raw_message_result?;
        let message = serde_json::from_slice(&raw_message).map_err(|e| {
            InputHandlingError::ParseError(e, String::from_utf8(raw_message).unwrap())
        })?;
        sender.send(message)?;
    }
    Ok(())
}

pub enum WriterThreadCommand<W: Write = TcpStream> {
    SendMessage(MessageEnvelope),
    Connect(W),
    Disconnect,
}

fn spawn_writer_thread() -> mpsc::Sender<WriterThreadCommand> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("debugger writer thread".into())
        .spawn(|| handle_writer_commands(rx))
        .expect("Unable to spawn the debugger writer thread");
    return tx;
}

fn handle_writer_commands<W: Write>(commands: impl IntoIterator<Item = WriterThreadCommand<W>>) {
    let mut stream = None;
    for command in commands {
        match command {
            WriterThreadCommand::Connect(new_stream) => stream = Some(new_stream),
            WriterThreadCommand::SendMessage(message) => {
                if let Some(ref mut stream_ref) = stream {
                    if let Err(e) = send_message(stream_ref, &message) {
                        eprintln!("{}", e);
                    }
                } else {
                    eprintln!("Debugger message dropped, no connection");
                }
            }
            WriterThreadCommand::Disconnect => stream = None,
        }
    }
}

#[derive(thiserror::Error, Debug)]
enum WriterCommunicationError {
    #[error("Unable to serialize debugger message: {0}")]
    ProtocolError(#[from] serde_json::error::Error),

    #[error("Unable to send debugger message: {0}")]
    SendError(#[from] ProtocolError),
}

fn send_message<W: Write>(
    stream: &mut W,
    message: &MessageEnvelope,
) -> Result<(), WriterCommunicationError> {
    // Note: I haven't found a way to trigger a serialization
    // error here, so this remains untested.
    let raw_message = serde_json::to_vec(message)?;
    send_raw_message(raw_message, stream)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debugger::dap_types::InitializeArguments;
    use crate::debugger::dap_types::Message;
    use crate::debugger::dap_types::Request;
    use crate::debugger::dap_types::Response;
    use crate::debugger::dap_types::ResponseEnvelope;
    use std::assert_matches::assert_matches;
    use std::fs;
    use std::path::Path;

    fn response_with_seq(seq: i64) -> MessageEnvelope {
        MessageEnvelope {
            seq,
            message: Message::Response(ResponseEnvelope {
                request_seq: 1,
                success: true,
                response: Response::Attach,
            }),
        }
    }

    fn message_seq_numbers_from_stream(stream: Vec<u8>) -> Vec<i64> {
        let mut stream_reader = stream.as_slice();
        raw_messages(&mut stream_reader)
            .map(|raw_message_result| {
                let envelope: MessageEnvelope =
                    serde_json::from_slice(&raw_message_result.unwrap()).unwrap();
                envelope.seq
            })
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
            Ok(MessageEnvelope {
                message:
                    Message::Request(Request::Initialize(InitializeArguments {
                        client_name: Some(ref client_name),
                    })),
                ..
            }) if client_name == "Visual Studio Code"
        );
        assert_matches!(
            rx.try_recv(),
            Ok(MessageEnvelope {
                message: Message::Request(Request::Disconnect(_)),
                ..
            })
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
        assert_matches!(err, InputHandlingError::ParseError(_, _));

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
    fn write_thread_handles_commands() {
        use WriterThreadCommand::*;

        let mut stream = vec![];
        let commands = vec![
            Connect(&mut stream),
            SendMessage(response_with_seq(4)),
            SendMessage(response_with_seq(5)),
        ];

        handle_writer_commands(commands);

        // Instead of inspecting the stream, which would be fragile and depend
        // on Serde implementation details, we'll parse the output and compare
        // it with the original message.
        assert_eq!(message_seq_numbers_from_stream(stream), vec![4, 5]);
    }

    #[test]
    fn write_thread_ignores_commands_between_connections() {
        use WriterThreadCommand::*;

        let mut stream1 = vec![];
        let mut stream2 = vec![];
        let commands = vec![
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

        handle_writer_commands(commands);

        assert_eq!(message_seq_numbers_from_stream(stream1), vec![3, 4]);
        assert_eq!(message_seq_numbers_from_stream(stream2), vec![7, 8]);
    }

    #[test]
    fn write_thread_handles_errors() {
        use WriterThreadCommand::*;

        // Attempt to write to an empty slice, which should cause an error, but
        // the error shouldn't result in a panic.
        let stream1: &mut [u8] = &mut [];
        let commands = vec![Connect(stream1), SendMessage(response_with_seq(1))];

        handle_writer_commands(commands);
    }
}
