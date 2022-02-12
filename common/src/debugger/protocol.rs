use lazy_static::lazy_static;
use regex::Regex;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::iter;
use std::num::ParseIntError;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches::assert_matches;
    use std::io::BufReader;
    use std::io::Read;
    use std::iter;

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

    #[test]
    fn sends_messages() {
        let raw_message = "foo".as_bytes().to_owned();
        let mut output = Vec::new();
        send_raw_message(raw_message, &mut output).unwrap();

        assert_eq!(&output, "Content-Length: 3\r\n\r\nfoo".as_bytes());
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
