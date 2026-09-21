//! Data-only protocol. Host functions never cross this boundary.
use aimux_core::AiMuxError;
use aimux_core::generate::GenerateTextOptions;
use aimux_core::message::{ModelMessage, ModelPrompt};
use aimux_core::parse_tool_call::{RawToolCall, ToolCallRepairContext};
use aimux_core::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    GenerateText,
    GenerateObject,
    ConsumeStreamText,
    StreamText,
    GenerateTextAsOpenai,
    StreamTextAsOpenai,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartRequest {
    pub protocol_version: u32,
    pub mode: Mode,
    pub prompt: ModelPrompt,
    #[serde(default)]
    pub options: Value,
    #[serde(default)]
    pub repair_tool_call: bool,
}
impl StartRequest {
    pub(crate) fn options(&self) -> Result<GenerateTextOptions, AiMuxError> {
        if self.protocol_version != 1 {
            return Err(AiMuxError::InvalidArgument(
                "unsupported operation protocol version".into(),
            ));
        }
        for name in ["repair_tool_call", "repairToolCall", "abort_signal"] {
            if self.options.get(name).is_some() {
                return Err(AiMuxError::InvalidArgument(format!(
                    "{name} is not a data option"
                )));
            }
        }
        if self.options.is_null() {
            return Ok(GenerateTextOptions::default());
        }
        serde_json::from_value(self.options.clone())
            .map_err(|e| AiMuxError::InvalidArgument(e.to_string()))
    }
}

#[derive(Debug, Serialize)]
pub struct RepairContext {
    pub tool_call: RawToolCall,
    pub error: AiMuxError,
    pub input_schema: Value,
    pub tools: Vec<Tool>,
    pub messages: Vec<ModelMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}
impl From<ToolCallRepairContext> for RepairContext {
    fn from(c: ToolCallRepairContext) -> Self {
        let input_schema = c.input_schema(&c.tool_call.tool_name);
        Self {
            tool_call: c.tool_call,
            error: c.error,
            input_schema,
            tools: c.tools,
            messages: c.messages,
            instructions: c.instructions,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    RepairRequest {
        request_id: String,
        context: Box<RepairContext>,
    },
    Part {
        part: Value,
    },
    Result {
        result: Value,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    Repaired { tool_call: RawToolCall },
    Unchanged,
    Failed { message: String },
}
impl Reply {
    pub(crate) fn into_core(self) -> Result<Option<RawToolCall>, AiMuxError> {
        match self {
            Self::Repaired { tool_call } => Ok(Some(tool_call)),
            Self::Unchanged => Ok(None),
            Self::Failed { message } => Err(AiMuxError::Other(message)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Lane {
    Control = 0,
    Output = 1,
    Any = 2,
    Terminal = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ReplyStatus {
    Accepted = 0,
    AlreadyReplied = 1,
    UnknownRequest = 2,
    OperationEnded = 3,
}

#[derive(Debug)]
pub enum Next {
    Event(Event),
    Ended,
    ReaderBusy,
}
