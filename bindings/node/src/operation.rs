//! Native data transport only. JS owns and invokes all host functions.
use crate::{
    Model,
    error::{AimuxResult, BindingError},
};
use aimux_operation::{Lane, Next, Operation, Reply, StartRequest};
use napi_derive::napi;
use serde_json::Value;

#[napi]
pub struct HostOperation {
    pub(crate) inner: Operation,
}

#[napi]
impl Model {
    #[napi]
    pub async fn start_operation(&self, request: Value) -> AimuxResult<HostOperation> {
        AimuxResult((|| {
            let request: StartRequest = serde_json::from_value(request)
                .map_err(|e| aimux_core::AiMuxError::InvalidArgument(e.to_string()))?;
            Ok(HostOperation {
                inner: Operation::start(self.inner.clone(), request)?,
            })
        })())
    }
}

#[napi]
impl HostOperation {
    #[napi]
    pub async fn next(&self, lane: u32) -> AimuxResult<Option<Value>> {
        AimuxResult(
            async {
                let lane = match lane {
                    0 => Lane::Control,
                    1 => Lane::Output,
                    2 => Lane::Any,
                    _ => {
                        return Err(aimux_core::AiMuxError::InvalidArgument(
                            "invalid operation lane".into(),
                        )
                        .into());
                    }
                };
                match self.inner.next(lane).await? {
                    Next::Ended => Ok(None),
                    Next::ReaderBusy => Err(BindingError::InvariantViolation {
                        message: "concurrent operation reader".into(),
                    }
                    .into()),
                    Next::Event(event) => serde_json::to_value(event).map(Some).map_err(|e| {
                        BindingError::ResultSerialization {
                            message: e.to_string(),
                        }
                        .into()
                    }),
                }
            }
            .await,
        )
    }
    #[napi]
    pub fn reply(&self, request_id: String, reply: Value) -> AimuxResult<u32> {
        AimuxResult((|| {
            let reply: Reply = serde_json::from_value(reply)
                .map_err(|e| aimux_core::AiMuxError::InvalidArgument(e.to_string()))?;
            Ok(self.inner.reply(&request_id, reply) as u32)
        })())
    }
    #[napi]
    pub fn cancel(&self) {
        self.inner.cancel();
    }
    #[napi]
    pub async fn finished(&self) {
        self.inner.finished().await;
    }
    #[napi]
    pub async fn close(&self) {
        self.inner.close().await;
    }
}
