use debugserver_types::AttachRequest;
use debugserver_types::AttachResponse;
use debugserver_types::DisconnectRequest;
use debugserver_types::EvaluateResponse;
use debugserver_types::InitializeRequest;
use debugserver_types::InitializeResponse;
use debugserver_types::NextResponse;
use lazy_static::lazy_static;
use regex::Regex;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::iter;
use std::num::ParseIntError;
use thiserror::Error;

/// A Debug Adapter Protocol request.
#[derive(Debug, PartialEq)]
pub enum Request {
    Initialize(InitializeRequest),
    Attach(AttachRequest),
    Disconnect(DisconnectRequest),
}

// A Debug Adapter Protocol response.
pub enum Response {
    Initialize(InitializeResponse),
    Attach(AttachResponse),
    Next(NextResponse),
    Evaluate(EvaluateResponse),
}

#[derive(Error, Debug)]
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

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Unable to parse debugger message: {0}")]
    JsonParserError(#[from] serde_json::Error),

    #[error("Unsupported message type: {0}")]
    UnsupportedMessageType(serde_json::Value),

    #[error("Unsupported command: {0}")]
    UnsupportedCommand(serde_json::Value),
}

/// Parses a DAP request from a byte buffer.
pub fn parse_request(raw_message: Vec<u8>) -> Result<Request, ParseError> {
    // Note: the `debugserver_types` crate doesn't play well with internally
    // tagged types, which are used by the DAP protocol, so we need to jump
    // through a couple of hoops here instead of using `serde`'s built-in
    // mechanism.
    let message_value: serde_json::Value = serde_json::from_slice(&raw_message)?;
    match &message_value["type"] {
        serde_json::Value::String(s) if s == "request" => {}
        unknown => return Err(ParseError::UnsupportedMessageType(unknown.clone())),
    }
    let command_value = &message_value["command"];
    return match &command_value.as_str() {
        Some("initialize") => Ok(Request::Initialize(serde_json::from_value(message_value)?)),
        Some("disconnect") => Ok(Request::Disconnect(serde_json::from_value(message_value)?)),
        Some("attach") => Ok(Request::Attach(serde_json::from_value(message_value)?)),
        _ => Err(ParseError::UnsupportedCommand(command_value.clone())),
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
    output.write_fmt(format_args!(
        "Content-Length: {}\r\n\r\n",
        message_bytes.len()
    ))?;
    output.write_all(&message_bytes)?;

    Ok(())
}

/// A thin wrapper over `serde_json::Error`, just to make the API a bit cleaner.
#[derive(Error, Debug)]
#[error("Unable to serialize debugger message: {0}")]
pub struct SerializeError(#[from] serde_json::Error);

/// Serializes a DAP protocol response as JSON.
pub fn serialize_response(response: &Response) -> Result<Vec<u8>, SerializeError> {
    use Response::*;
    match response {
        Next(msg) => serde_json::to_vec(msg),
        Evaluate(msg) => serde_json::to_vec(msg),
        Initialize(msg) => serde_json::to_vec(msg),
        Attach(msg) => serde_json::to_vec(msg),
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
    fn deserializes_requests() {
        let initialize_request = parse_request(read_test_data("initialize_request.json"));
        let attach_request = parse_request(read_test_data("attach_request.json"));
        let disconnect_request = parse_request(read_test_data("disconnect_request.json"));

        assert_matches!(
            initialize_request,
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
            attach_request,
            Ok(Request::Attach(AttachRequest { command, .. })) if command == "attach"
        );
        assert_matches!(
            disconnect_request,
            Ok(Request::Disconnect(DisconnectRequest {
                arguments: Some(DisconnectArguments {
                    restart: Some(false),
                    ..
                }),
                ..
            }))
        );
    }

    #[test]
    fn request_deserialization_errors() {
        let invalid_request = parse_request(String::from(r#"{"foo": "bar"}"#).into_bytes());
        let unknown_command = parse_request(
            String::from(r#"{"type": "request", "command": "beam me up"}"#).into_bytes(),
        );
        let empty_request = parse_request(vec![]);

        assert_eq!(true, invalid_request.is_err());
        assert_eq!(true, unknown_command.is_err());
        assert_eq!(true, empty_request.is_err());
    }

    #[test]
    fn sends_messages() {
        let raw_message = "foo".as_bytes().to_owned();
        let mut output = Vec::new();
        send_raw_message(raw_message, &mut output).unwrap();

        assert_eq!(&output, "Content-Length: 3\r\n\r\nfoo".as_bytes());
    }

    #[test]
    fn serializes_responses() {
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
            serialize_response(&Response::Evaluate(evaluate_response.clone())).unwrap();
        let next_response_bytes =
            serialize_response(&Response::Next(next_response.clone())).unwrap();

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
