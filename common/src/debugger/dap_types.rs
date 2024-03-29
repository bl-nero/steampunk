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

use crate::debugger::core::StopReason;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MessageEnvelope {
    pub seq: i64,

    #[serde(flatten)]
    pub message: Message,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Message {
    Request(Request),
    Response(ResponseEnvelope),
    Event(Event),
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "command", content = "arguments", rename_all = "camelCase")]
pub enum Request {
    Initialize(InitializeArguments),
    SetExceptionBreakpoints {},
    SetInstructionBreakpoints(SetInstructionBreakpointsArguments),
    Attach {},
    Threads,
    StackTrace {},
    Scopes(ScopesArguments),
    Variables(VariablesArguments),
    Disassemble(DisassembleArguments),
    ReadMemory(ReadMemoryArguments),

    Continue {},
    Pause {},
    Next {},
    StepIn {},
    StepOut {},

    Disconnect(Option<DisconnectArguments>),
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InitializeArguments {
    pub client_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SetInstructionBreakpointsArguments {
    pub breakpoints: Vec<InstructionBreakpoint>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScopesArguments {
    pub frame_id: i64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VariablesArguments {
    pub variables_reference: i64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DisassembleArguments {
    /// Base address of the disassembled memory region. Note (you won't read
    /// this in the protocol spec!): this address needs to observe the same
    /// conventions as [`DisassembledInstruction::address`].
    pub memory_reference: String,

    /// Offset (in bytes), relative to the [`memory_reference`].
    pub offset: Option<i64>,

    /// Offset (in number of instructions), relative to [`memory_reference`]` + `[`offset`].
    pub instruction_offset: Option<i64>,

    pub instruction_count: i64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadMemoryArguments {
    pub memory_reference: String,
    pub offset: Option<i64>,
    pub count: i64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ResponseEnvelope {
    pub request_seq: i64,
    pub success: bool,

    #[serde(flatten)]
    pub response: Response,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "command", content = "body", rename_all = "camelCase")]
pub enum Response {
    Initialize(Capabilities),
    SetExceptionBreakpoints,
    SetInstructionBreakpoints(SetInstructionBreakpointsResponse),
    Attach,
    Threads(ThreadsResponse),
    StackTrace(StackTraceResponse),
    Scopes(ScopesResponse),
    Variables(VariablesResponse),
    Disassemble(DisassembleResponse),
    ReadMemory(ReadMemoryResponse),

    Continue {},
    Pause,
    Next,
    StepIn,
    StepOut,

    Disconnect,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub supports_disassemble_request: bool,
    pub supports_instruction_breakpoints: bool,
    pub supports_read_memory_request: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SetInstructionBreakpointsResponse {
    pub breakpoints: Vec<Breakpoint>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadsResponse {
    pub threads: Vec<Thread>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StackTraceResponse {
    pub stack_frames: Vec<StackFrame>,
    pub total_frames: i64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScopesResponse {
    pub scopes: Vec<Scope>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    pub name: String,
    pub presentation_hint: Option<ScopePresentationHint>,
    pub variables_reference: i64,
    pub expensive: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ScopePresentationHint {
    Registers,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VariablesResponse {
    pub variables: Vec<Variable>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DisassembleResponse {
    pub instructions: Vec<DisassembledInstruction>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadMemoryResponse {
    pub address: String,
    pub data: String,
    pub unreadable_bytes: i64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DisassembledInstruction {
    /// The instruction address; if it's preceded by "0x", it's treated as
    /// hexadecimal.
    pub address: String,
    pub instruction_bytes: String,
    pub instruction: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub variables_reference: i64,
    pub memory_reference: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "event", content = "body", rename_all = "camelCase")]
pub enum Event {
    Initialized,
    Stopped(StoppedEvent),
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoppedEvent {
    pub reason: StopReason,
    pub thread_id: i64,
    pub all_threads_stopped: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StackFrame {
    pub id: i64,
    pub name: String,
    pub line: i64,
    pub column: i64,
    pub instruction_pointer_reference: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub id: i64,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstructionBreakpoint {
    pub instruction_reference: String,
    pub offset: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Breakpoint {
    pub verified: bool,
    pub instruction_reference: String,
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
        set_instruction_breakpoints_request: MessageEnvelope {
            seq: 3,
            message: Message::Request(Request::SetInstructionBreakpoints(
                SetInstructionBreakpointsArguments {
                    breakpoints: vec![
                        InstructionBreakpoint {
                            instruction_reference: "0xAB12".to_string(),
                            offset: None,
                        },
                        InstructionBreakpoint {
                            instruction_reference: "0x12AB".to_string(),
                            offset: Some(-12),
                        }
                    ]
                }
            )),
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
        scopes_request: MessageEnvelope {
            seq: 7,
            message: Message::Request(Request::Scopes (ScopesArguments {
                frame_id: 1,
            })),
        },
        variables_request: MessageEnvelope {
            seq: 8,
            message: Message::Request(Request::Variables(VariablesArguments {
                variables_reference: 1,
            })),
        },
        disassemble_request: MessageEnvelope {
            seq: 9,
            message: Message::Request(Request::Disassemble(DisassembleArguments {
                memory_reference: "0xBEEF".to_string(),
                offset: Some(0),
                instruction_offset: Some(-200),
                instruction_count: 400,
            })),
        },
        read_memory_request: MessageEnvelope {
            seq: 15,
            message: Message::Request(Request::ReadMemory(ReadMemoryArguments {
                memory_reference: "0xFCE2".to_string(),
                offset: Some(0),
                count: 131072,
            })),
        },
        continue_request: MessageEnvelope {
            seq: 10,
            message: Message::Request(Request::Continue {}),
        },
        pause_request: MessageEnvelope {
            seq: 10,
            message: Message::Request(Request::Pause {}),
        },
        next_request: MessageEnvelope {
            seq: 9,
            message: Message::Request(Request::Next {}),
        },
        step_in_request: MessageEnvelope {
            seq: 9,
            message: Message::Request(Request::StepIn {}),
        },
        step_out_request: MessageEnvelope {
            seq: 9,
            message: Message::Request(Request::StepOut {}),
        },
        disconnect_request: MessageEnvelope {
            seq: 2,
            message: Message::Request(Request::Disconnect(Some(DisconnectArguments {}))),
        },
        disconnect_request_no_args: MessageEnvelope {
            seq: 2,
            message: Message::Request(Request::Disconnect(None)),
        },

        initialize_response: MessageEnvelope {
            seq: 1,
            message: Message::Response(ResponseEnvelope {
                request_seq: 11,
                success: true,
                response: Response::Initialize(Capabilities {
                    supports_disassemble_request: true,
                    supports_instruction_breakpoints: true,
                    supports_read_memory_request: true,
                }),
            }),
        },
        set_exception_breakpoints_response: MessageEnvelope {
            seq: 2,
            message: Message::Response(ResponseEnvelope {
                request_seq: 12,
                success: true,
                response: Response::SetExceptionBreakpoints,
            }),
        },
        set_instruction_breakpoints_response: MessageEnvelope {
            seq: 2,
            message: Message::Response(ResponseEnvelope {
                request_seq: 76,
                success: true,
                response: Response::SetInstructionBreakpoints(
                    SetInstructionBreakpointsResponse {
                        breakpoints: vec![Breakpoint {
                            verified: true,
                            instruction_reference: "0x9876".to_string(),
                        }]
                    }
                ),
            }),
        },
        attach_response: MessageEnvelope {
            seq: 3,
            message: Message::Response(ResponseEnvelope {
                request_seq: 13,
                success: true,
                response: Response::Attach,
            }),
        },
        threads_response: MessageEnvelope {
            seq: 54,
            message: Message::Response(ResponseEnvelope {
                request_seq: 14,
                success: true,
                response: Response::Threads(ThreadsResponse {
                    threads: vec![Thread {
                        id: 1,
                        name: "main thread".to_string(),
                    }],
                }),
            }),
        },
        stack_trace_response: MessageEnvelope {
            seq: 75,
            message: Message::Response(ResponseEnvelope {
                request_seq: 19,
                success: true,
                response: Response::StackTrace(StackTraceResponse {
                    stack_frames: vec![StackFrame {
                        id: 1,
                        name: "foo".to_string(),
                        line: 0,
                        column: 0,
                        instruction_pointer_reference: "0x1234".to_string(),
                    }],
                    total_frames: 1,
                }),
            }),
        },
        scopes_response: MessageEnvelope {
            seq: 65,
            message: Message::Response(ResponseEnvelope {
                request_seq: 82,
                success: true,
                response: Response::Scopes(ScopesResponse {
                    scopes: vec![Scope {
                        name: "Registers".to_string(),
                        presentation_hint: Some(ScopePresentationHint::Registers),
                        variables_reference: 1,
                        expensive: false,
                    }]
                }),
            }),
        },
        variables_response: MessageEnvelope {
            seq: 45,
            message: Message::Response(ResponseEnvelope {
                request_seq: 74,
                success: true,
                response: Response::Variables(VariablesResponse {
                    variables: vec![Variable {
                        name: "A".to_string(),
                        value: "$43".to_string(),
                        variables_reference: 0,
                        memory_reference: None,
                    }]
                }),
            }),
        },
        disassemble_response: MessageEnvelope {
            seq: 98,
            message: Message::Response(ResponseEnvelope {
                request_seq: 63,
                success: true,
                response: Response::Disassemble(DisassembleResponse {
                    instructions: vec![
                        DisassembledInstruction {
                            address: "0xBEEF".to_string(),
                            instruction_bytes: "A9 76".to_string(),
                            instruction: "LDA #$76".to_string(),
                        },
                        DisassembledInstruction {
                            address: "0xBEF1".to_string(),
                            instruction_bytes: "8D 4F C9".to_string(),
                            instruction: "STA $C94F".to_string(),
                        },
                    ],
                }),
            }),
        },
        read_memory_response: MessageEnvelope {
            seq: 76,
            message: Message::Response(ResponseEnvelope {
                request_seq: 83,
                success: true,
                response: Response::ReadMemory(ReadMemoryResponse {
                    address: "0xDEAD".to_string(),
                    data: "vu8=".to_string(),
                    unreadable_bytes: 0,
                }),
            }),
        },
        continue_response: MessageEnvelope {
            seq: 11,
            message: Message::Response(ResponseEnvelope {
                request_seq: 9,
                success: true,
                response: Response::Continue{},
            }),
        },
        pause_response: MessageEnvelope {
            seq: 12,
            message: Message::Response(ResponseEnvelope {
                request_seq: 10,
                success: true,
                response: Response::Pause,
            }),
        },
        next_response: MessageEnvelope {
            seq: 78,
            message: Message::Response(ResponseEnvelope {
                request_seq: 87,
                success: true,
                response: Response::Next,
            }),
        },
        step_in_response: MessageEnvelope {
            seq: 61,
            message: Message::Response(ResponseEnvelope {
                request_seq: 13,
                success: true,
                response: Response::StepIn,
            }),
        },
        step_out_response: MessageEnvelope {
            seq: 74,
            message: Message::Response(ResponseEnvelope {
                request_seq: 72,
                success: true,
                response: Response::StepOut,
            }),
        },
        disconnect_response: MessageEnvelope {
            seq: 64,
            message: Message::Response(ResponseEnvelope {
                request_seq: 89,
                success: true,
                response: Response::Disconnect,
            }),
        },

        initialized_event: MessageEnvelope {
            seq: 74,
            message: Message::Event(Event::Initialized),
        },
        stopped_event: MessageEnvelope {
            seq: 10,
            message: Message::Event(Event::Stopped(StoppedEvent {
                reason: StopReason::Entry,
                thread_id: 1,
                all_threads_stopped: true,
            })),
        },
    }
}
