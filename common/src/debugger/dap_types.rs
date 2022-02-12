//! This module contains data types for the Debug Adapter Protocol. Note that we
//! don't use the `debugserver_types` crate, because it's automatically
//! generated from an outdated JSON schema.  Generating types on our own using
//! `schemafy` has a disadvantage: even if we put it in a separate crate, it
//! still has a big negative impact on the Rust Language Server perfomance. On
//! top of that, using `schemafy` results in a type system that is not easy to
//! use.
//!
//! Note that this crate deliberately doesn't contain all of the types, and the
//! types only have the fields that we really use.

use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct MessageEnvelope {
    pub seq: i64,

    #[serde(flatten)]
    pub message: Message,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Message {
    Request(Request),
    Response(Response),
    Event,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "command", content = "arguments", rename_all = "camelCase")]
pub enum Request {
    Initialize(InitializeArguments),
    SetExceptionBreakpoints {},
    Attach {},
    Threads,
    StackTrace {},
    Disconnect(Option<DisconnectArguments>),
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "command", rename_all = "camelCase")]
pub enum Response {
    Initialize,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InitializeArguments {
    pub client_name: Option<String>,
}

/// This empty struct is here only because `Serde` doesn't allow us to use an
/// unit enum in place where the content (`arguments`) _can_ appear, but is
/// optional. That's why [`Request::Disconnect`] is parametrized by
/// `Option<`[`DisconnectArguments`]`>`.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectArguments {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn read_test_string(name: &str) -> String {
        fs::read_to_string(
            Path::new("src")
                .join("debugger")
                .join("test_data")
                .join(name),
        )
        .unwrap()
    }

    macro_rules! message_serialization_tests {
        ($($name:ident: $message:expr,)*) => {$(
            #[test]
            fn $name() {
                let message = $message;
                let serialized = serde_json::to_string(&message).unwrap();
                let deserialized: MessageEnvelope = serde_json::from_str(&serialized).unwrap();
                assert_eq!(deserialized, message);

                let file_name = concat!(stringify!($name), ".json");
                let deserialized2: MessageEnvelope =
                    serde_json::from_str(&read_test_string(file_name)).unwrap();
                assert_eq!(deserialized2, message);
            }
        )*}
    }

    message_serialization_tests! {
        initialize_request: MessageEnvelope {
            seq: 1,
            message: Message::Request(Request::Initialize(InitializeArguments {
                client_name: Some("Visual Studio Code".to_string()),
            })),
        },
        set_exception_breakpoints_request: MessageEnvelope {
            seq: 3,
            message: Message::Request(Request::SetExceptionBreakpoints {}),
        },
        attach_request: MessageEnvelope {
            seq: 2,
            message: Message::Request(Request::Attach {}),
        },
        threads_request: MessageEnvelope {
            seq: 4,
            message: Message::Request(Request::Threads),
        },
        stack_trace_request: MessageEnvelope {
            seq: 6,
            message: Message::Request(Request::StackTrace {}),
        },
        disconnect_request: MessageEnvelope {
            seq: 2,
            message: Message::Request(Request::Disconnect(Some(DisconnectArguments {}))),
        },
        disconnect_request_no_args: MessageEnvelope {
            seq: 2,
            message: Message::Request(Request::Disconnect(None)),
        },
    }
}
