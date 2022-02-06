use debugserver_types::AttachRequest;
use debugserver_types::AttachResponse;
use debugserver_types::DisconnectRequest;
use debugserver_types::EvaluateResponse;
use debugserver_types::InitializeRequest;
use debugserver_types::InitializeResponse;
use debugserver_types::InitializedEvent;
use debugserver_types::NextResponse;
use debugserver_types::SetExceptionBreakpointsRequest;
use debugserver_types::SetExceptionBreakpointsResponse;
use debugserver_types::StackTraceRequest;
use debugserver_types::StackTraceResponse;
use debugserver_types::StoppedEvent;
use debugserver_types::ThreadsRequest;
use debugserver_types::ThreadsResponse;
use lazy_static::lazy_static;
use regex::Regex;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::iter;
use std::num::ParseIntError;

/// Incoming messages of the Debug Adapter Protocol.
#[derive(Debug, PartialEq, Clone)]
pub enum IncomingMessage {
    Initialize(InitializeRequest),
    SetExceptionBreakpoints(SetExceptionBreakpointsRequest),
    Attach(AttachRequest),
    Threads(ThreadsRequest),
    StackTrace(StackTraceRequest),
    Disconnect(DisconnectRequest),

    Unknown(serde_json::Value),
}

/// Outgoing messages of the Debug Adapter Protocol.
#[derive(Clone, PartialEq, Debug)]
pub enum OutgoingMessage {
    Initialize(InitializeResponse),
    SetExceptionBreakpoints(SetExceptionBreakpointsResponse),
    Attach(AttachResponse),
    Threads(ThreadsResponse),
    StackTrace(StackTraceResponse),
    Next(NextResponse),
    Evaluate(EvaluateResponse),

    Initialized(InitializedEvent),
    Stopped(StoppedEvent),
}

#[derive(thiserror::Error, Debug)]
pub enum ProtocolError {
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    #[error("No Content-Length header")]
    NoContentLengthHeader,

    #[error("Error while parsing a header: {0}")]
    HeaderParseError(#[from] ParseIntError),
}

pub type ProtocolResult<T> = Result<T, ProtocolError>;

/// Extracts raw message buffers from a Debug Adapter Protocol stream. Each
/// buffer contains exactly one message body without headers.
pub fn raw_messages<'a>(
    input: &'a mut impl BufRead,
) -> impl Iterator<Item = ProtocolResult<Vec<u8>>> + 'a {
    iter::from_fn(move || match read_headers(input) {
        Ok(Some(content_length)) => {
            let mut message_body = vec![0; content_length];
            Some(match input.read_exact(&mut message_body) {
                Ok(_) => Ok(message_body),
                Err(e) => Err(e.into()),
            })
        }
        Ok(None) => None,
        Err(e) => Some(Err(e)),
    })
}

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("Unable to parse debugger message: {0}")]
    JsonParserError(#[from] serde_json::Error),

    #[error("Unsupported message type: {0}")]
    UnsupportedMessageType(serde_json::Value),

    #[error("Unsupported command: {0}")]
    UnsupportedCommand(serde_json::Value),
}

/// Parses a DAP message from a byte buffer.
pub fn parse_message(raw_message: Vec<u8>) -> Result<IncomingMessage, ParseError> {
    // Note: the `debugserver_types` crate doesn't play well with internally
    // tagged types, which are used by the DAP protocol, so we need to jump
    // through a couple of hoops here instead of using `serde`'s built-in
    // mechanism.
    // println!("-> {}", std::str::from_utf8(&raw_message).unwrap());
    let message_value: serde_json::Value = serde_json::from_slice(&raw_message)?;
    match &message_value["type"] {
        serde_json::Value::String(s) if s == "request" => {}
        unknown => return Err(ParseError::UnsupportedMessageType(unknown.clone())),
    }
    let command_value = &message_value["command"];
    return match &command_value.as_str() {
        Some("initialize") => Ok(IncomingMessage::Initialize(serde_json::from_value(
            message_value,
        )?)),
        Some("setExceptionBreakpoints") => Ok(IncomingMessage::SetExceptionBreakpoints(
            serde_json::from_value(message_value)?,
        )),
        Some("attach") => Ok(IncomingMessage::Attach(serde_json::from_value(
            message_value,
        )?)),
        Some("threads") => Ok(IncomingMessage::Threads(serde_json::from_value(
            message_value,
        )?)),
        Some("stackTrace") => Ok(IncomingMessage::StackTrace(serde_json::from_value(
            message_value,
        )?)),
        Some("disconnect") => Ok(IncomingMessage::Disconnect(serde_json::from_value(
            message_value,
        )?)),
        _ => Err(ParseError::UnsupportedCommand(message_value)),
    };
}

/// Reads DAP headers from an input stream. Note that while the only header
/// currently specified in the DAP specification is `Content-Length`, this
/// function is future-proof: it simply ignores unknown headers.
///
/// In a typical case, the message size is returned, as indicated by the
/// `Content-Length` header. Otherwise, `None` denotes end of stream has been
/// reached.
fn read_headers(input: &mut impl BufRead) -> ProtocolResult<Option<usize>> {
    lazy_static! {
        static ref HEADER_REGEX: Regex = Regex::new(r#"Content-Length:\s*(.*)"#).unwrap();
    }

    let mut message_started = false;
    let mut content_length = None;
    for header_line in input.lines() {
        message_started = true;
        let header_text = header_line.map_err(|e| ProtocolError::from(e))?;
        if header_text == "" {
            return match content_length {
                Some(length) => Ok(Some(length)),
                None => Err(ProtocolError::NoContentLengthHeader),
            };
        } else {
            if let Some(captures) = HEADER_REGEX.captures(&header_text) {
                content_length = Some(
                    captures
                        .get(1)
                        .unwrap()
                        .as_str()
                        .parse()
                        .map_err(|e| ProtocolError::from(e))?,
                );
            }
        }
    }
    return if message_started {
        Err(ProtocolError::NoContentLengthHeader)
    } else {
        Ok(None)
    };
}

/// Sends a raw byte buffer using the DAP protocol.
pub fn send_raw_message(message_bytes: Vec<u8>, output: &mut impl Write) -> ProtocolResult<()> {
    // println!("<- {}", std::str::from_utf8(&message_bytes).unwrap());
    output.write_fmt(format_args!(
        "Content-Length: {}\r\n\r\n",
        message_bytes.len()
    ))?;
    output.write_all(&message_bytes)?;

    Ok(())
}

/// A thin wrapper over `serde_json::Error`, just to make the API a bit cleaner.
#[derive(thiserror::Error, Debug)]
#[error("Unable to serialize debugger message: {0}")]
pub struct SerializeError(#[from] serde_json::Error);

/// Serializes a DAP protocol message as JSON.
pub fn serialize_message(message: &OutgoingMessage) -> Result<Vec<u8>, SerializeError> {
    use OutgoingMessage::*;
    match message {
        Next(msg) => serde_json::to_vec(msg),
        Evaluate(msg) => serde_json::to_vec(msg),
        Initialize(msg) => serde_json::to_vec(msg),
        Attach(msg) => serde_json::to_vec(msg),
        Threads(msg) => serde_json::to_vec(msg),
        StackTrace(msg) => serde_json::to_vec(msg),

        Initialized(msg) => serde_json::to_vec(msg),
        SetExceptionBreakpoints(msg) => serde_json::to_vec(msg),
        Stopped(msg) => serde_json::to_vec(msg),
    }
    // Note: there's no way to test it, and I doubt it would ever happen, but
    // anyway, let's map the error.
    .map_err(|e| e.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use debugserver_types::DisconnectArguments;
    use debugserver_types::InitializeRequestArguments;
    use serde_json::json;
    use std::fs;
    use std::io::BufReader;
    use std::io::Read;
    use std::iter;
    use std::path::Path;

    #[test]
    fn no_commands() {
        let mut input = "".as_bytes();
        itertools::assert_equal(
            raw_messages(&mut input).map(Result::unwrap),
            iter::empty::<Vec<u8>>(),
        );
    }

    #[test]
    fn one_command() {
        let mut input = "Content-Length: 3\r\n\r\nfoo".as_bytes();
        itertools::assert_equal(
            raw_messages(&mut input).map(Result::unwrap),
            vec![b"foo".to_vec()],
        );
    }

    #[test]
    fn many_commands() {
        let mut input = "Content-Length: 20\r\n\
            \r\n\
            01234567890123456789\
            Content-Length: 8\r\n\
            \r\n\
            abcdefgh\
            Content-Length: 7\r\n\
            \r\n\
            A\r\n\
            B\r\n\
            C"
        .as_bytes();
        itertools::assert_equal(
            raw_messages(&mut input).map(Result::unwrap),
            vec![
                b"01234567890123456789".to_vec(),
                b"abcdefgh".to_vec(),
                b"A\r\nB\r\nC".to_vec(),
            ],
        );
    }

    #[test]
    fn unsupported_headers() {
        let mut input = "Content-Length: 3\r\n\
            User-agent: teapot\r\n\
            \r\n\
            foo\
            Another-Dummy-Header: dummy\r\n\
            Content-Length: 4\r\n\
            \r\n\
            asdf"
            .as_bytes();
        itertools::assert_equal(
            raw_messages(&mut input).map(Result::unwrap),
            vec![b"foo".to_vec(), b"asdf".to_vec()],
        );
    }

    struct BrokenReader {}

    impl Read for BrokenReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::ConnectionReset.into())
        }
    }

    struct BrokenWriter {
        broken_now: bool,
        trigger: Option<u8>,
    }

    impl Write for BrokenWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if let Some(trigger) = self.trigger {
                if bytes.contains(&trigger) {
                    self.broken_now = true;
                }
            }
            return if self.broken_now {
                Err(io::ErrorKind::ConnectionReset.into())
            } else {
                Ok(bytes.len())
            };
        }

        fn flush(&mut self) -> io::Result<()> {
            return if self.broken_now {
                Err(io::ErrorKind::ConnectionReset.into())
            } else {
                Ok(())
            };
        }
    }

    #[test]
    fn unexpected_eof() {
        let mut input = "Content-Length: 56\r\n\r\nFoo".as_bytes();
        let actual: Vec<_> = raw_messages(&mut input).collect();
        assert_matches!(
            &actual[0],
            Err(ProtocolError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof,
        );
    }

    #[test]
    fn no_content_length_header() {
        let mut input = "Foo: bar\r\n\r\n".as_bytes();
        let actual: Vec<_> = raw_messages(&mut input).collect();
        assert_eq!(actual.len(), 1);
        assert_matches!(&actual[0], Err(ProtocolError::NoContentLengthHeader));

        let mut input = "\r\n".as_bytes(); // No headers at all
        let actual: Vec<_> = raw_messages(&mut input).collect();
        assert_eq!(actual.len(), 1);
        assert_matches!(&actual[0], Err(ProtocolError::NoContentLengthHeader));
    }

    #[test]
    fn connection_reset_while_reading_headers() {
        let mut input = BufReader::new("Content-Len".as_bytes().chain(BrokenReader {}));
        let actual: Vec<_> = raw_messages(&mut input).take(1).collect();
        assert_matches!(
            &actual[0],
            Err(ProtocolError::IoError(e)) if e.kind() == io::ErrorKind::ConnectionReset,
        );
    }

    #[test]
    fn connection_reset_while_reading_body() {
        let mut input = BufReader::new(
            "Content-Length: 10\r\n\r\nFoo"
                .as_bytes()
                .chain(BrokenReader {}),
        );
        let actual: Vec<_> = raw_messages(&mut input).take(1).collect();
        assert_matches!(
            &actual[0],
            Err(ProtocolError::IoError(e)) if e.kind() == io::ErrorKind::ConnectionReset,
        );
    }

    macro_rules! test_illegal_content_length {
        ($fn_name:ident, $length:expr) => {
            #[test]
            fn $fn_name() {
                let mut input = concat!("Content-Length: ", $length, "\r\n\r\n").as_bytes();
                let actual: Vec<_> = raw_messages(&mut input).collect();
                assert_matches!(&actual[0], Err(ProtocolError::HeaderParseError(_)));
            }
        };
    }

    test_illegal_content_length!(content_length_not_a_number, "dummy");
    test_illegal_content_length!(content_length_negative, "-123");
    test_illegal_content_length!(content_length_empty, "");

    fn read_test_data(name: &str) -> Vec<u8> {
        fs::read(
            Path::new("src")
                .join("debugger")
                .join("test_data")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn deserializes_messages() {
        let initialize_request = parse_message(read_test_data("initialize_request.json"));
        let set_exception_breakpoints_request =
            parse_message(read_test_data("set_exception_breakpoints_request.json"));
        let attach_request = parse_message(read_test_data("attach_request.json"));
        let threads_request = parse_message(read_test_data("threads_request.json"));
        let stack_trace_request = parse_message(read_test_data("stack_trace_request.json"));
        let disconnect_request = parse_message(read_test_data("disconnect_request.json"));

        assert_matches!(
            initialize_request,
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
            set_exception_breakpoints_request,
            Ok(IncomingMessage::SetExceptionBreakpoints(
                SetExceptionBreakpointsRequest { command, .. }
            )) if command == "setExceptionBreakpoints"
        );
        assert_matches!(
            attach_request,
            Ok(IncomingMessage::Attach(AttachRequest { command, .. })) if command == "attach"
        );
        assert_matches!(
            threads_request,
            Ok(IncomingMessage::Threads(ThreadsRequest { command, .. })) if command == "threads"
        );
        assert_matches!(
            stack_trace_request,
            Ok(IncomingMessage::StackTrace(StackTraceRequest {
                command,
                ..
            })) if command == "stackTrace"
        );
        assert_matches!(
            disconnect_request,
            Ok(IncomingMessage::Disconnect(DisconnectRequest {
                arguments: Some(DisconnectArguments {
                    restart: Some(false),
                    ..
                }),
                ..
            }))
        );
    }

    #[test]
    fn message_deserialization_errors() {
        let invalid_request = parse_message(String::from(r#"{"foo": "bar"}"#).into_bytes());
        let unknown_command = parse_message(
            String::from(r#"{"type": "request", "command": "beam me up"}"#).into_bytes(),
        );
        let empty_message = parse_message(vec![]);

        invalid_request.unwrap_err();
        assert_matches!(
            unknown_command.unwrap_err(),
            ParseError::UnsupportedCommand(value) if value == json!({
                "type": "request",
                "command": "beam me up",
            }),
        );
        empty_message.unwrap_err();
    }

    #[test]
    fn sends_messages() {
        let raw_message = "foo".as_bytes().to_owned();
        let mut output = Vec::new();
        send_raw_message(raw_message, &mut output).unwrap();

        assert_eq!(&output, "Content-Length: 3\r\n\r\nfoo".as_bytes());
    }

    #[test]
    fn serializes_messages() {
        // Since we don't want to rely on the serializer implementation details,
        // instead of comparing to a golden result, we just read the serialized
        // messages back again and compare with the originals.
        let evaluate_response: EvaluateResponse = serde_json::from_value(json!({
            "type": "response",
            "request_seq": 1,
            "success": true,
            "command": "evaluate",
            "seq": 2,
            "body": {
                "result": "something",
                "variablesReference": 0,
            },
        }))
        .unwrap();
        let next_response: NextResponse = serde_json::from_value(json!({
            "type": "response",
            "request_seq": 3,
            "success": true,
            "command": "next",
            "seq": 4,
        }))
        .unwrap();

        let evaluate_response_bytes =
            serialize_message(&OutgoingMessage::Evaluate(evaluate_response.clone())).unwrap();
        let next_response_bytes =
            serialize_message(&OutgoingMessage::Next(next_response.clone())).unwrap();

        let actual_evaluate_response: EvaluateResponse =
            serde_json::from_slice(evaluate_response_bytes.as_slice()).unwrap();
        let actual_next_response: NextResponse =
            serde_json::from_slice(next_response_bytes.as_slice()).unwrap();

        assert_eq!(actual_evaluate_response, evaluate_response);
        assert_eq!(actual_next_response, next_response);
    }

    #[test]
    fn error_when_sending_headers() {
        let mut output = BrokenWriter {
            broken_now: true,
            trigger: None,
        };
        let result = send_raw_message(vec![1, 2, 3], &mut output);

        assert_matches!(
            result,
            Err(ProtocolError::IoError(e)) if e.kind() == io::ErrorKind::ConnectionReset,
        );
    }

    #[test]
    fn error_when_sending_message_body() {
        let mut output = BrokenWriter {
            broken_now: false,
            trigger: Some(0xFF),
        };
        let result = send_raw_message(vec![0xFF, 2, 3], &mut output);

        assert_matches!(
            result,
            Err(ProtocolError::IoError(e)) if e.kind() == io::ErrorKind::ConnectionReset,
        );
    }
}
