//! Host-driven generation. This crate owns no host callable or function pointer.
mod protocol;
pub use protocol::*;

use aimux_core::parse_tool_call::ToolCallRepair;
use aimux_core::{AiMuxError, LanguageModel};
use aimux_core::{
    generate::*, generation_control::GenerationControl, openai_output::OpenAiStreamOptions,
};
use futures::StreamExt;
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
    },
};
use tokio::{
    sync::{Notify, oneshot},
    task::JoinHandle,
};

const OUTPUT_CAPACITY: usize = 64;
struct Pending {
    id: u64,
    sender: oneshot::Sender<Reply>,
    request: Option<Event>,
}
struct State {
    terminal: Option<Result<(), AiMuxError>>,
    terminal_read: bool,
    output: VecDeque<Event>,
    pending: Option<Pending>,
    next_id: u64,
}
struct Shared {
    state: Mutex<State>,
    changed: Notify,
    readers: AtomicU8,
    control: GenerationControl,
}
impl Shared {
    fn finish(&self, result: Result<(), AiMuxError>) {
        let mut state = self.state.lock().unwrap();
        if state.terminal.is_none() {
            if result.is_err() {
                state.output.clear();
            }
            state.pending = None;
            state.terminal = Some(result);
        }
        drop(state);
        self.changed.notify_waiters();
    }
    fn observe_stop(&self) {
        if let Some(error) = self.control.failure() {
            self.finish(Err(error));
        }
    }
    async fn send(&self, event: Event) -> Result<(), AiMuxError> {
        let mut event = Some(event);
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            {
                let mut state = self.state.lock().unwrap();
                if let Some(terminal) = &state.terminal {
                    return Err(terminal
                        .clone()
                        .err()
                        .unwrap_or_else(|| AiMuxError::Other("operation ended".into())));
                }
                if state.output.len() < OUTPUT_CAPACITY {
                    state.output.push_back(event.take().unwrap());
                    drop(state);
                    self.changed.notify_waiters();
                    return Ok(());
                }
            }
            tokio::select! {
                () = &mut notified => {},
                error = self.control.stopped() => return Err(error),
            }
        }
    }
    async fn repair(
        &self,
        context: aimux_core::parse_tool_call::ToolCallRepairContext,
    ) -> Result<Option<aimux_core::parse_tool_call::RawToolCall>, AiMuxError> {
        let (sender, receiver) = oneshot::channel();
        {
            let mut state = self.state.lock().unwrap();
            if let Some(terminal) = &state.terminal {
                return Err(terminal
                    .clone()
                    .err()
                    .unwrap_or_else(|| AiMuxError::Other("operation ended".into())));
            }
            if state.pending.is_some() {
                return Err(AiMuxError::Other(
                    "concurrent repair requests are unsupported".into(),
                ));
            }
            let id = state.next_id;
            state.next_id = id
                .checked_add(1)
                .ok_or_else(|| AiMuxError::Other("repair request ID exhausted".into()))?;
            state.pending = Some(Pending {
                id,
                sender,
                request: Some(Event::RepairRequest {
                    request_id: id.to_string(),
                    context: Box::new(context.into()),
                }),
            });
        }
        self.changed.notify_waiters();
        // Core already waits for repair inside this operation's shared budget.
        receiver
            .await
            .map_err(|_| {
                self.control
                    .failure()
                    .unwrap_or_else(|| AiMuxError::Other("repair request ended".into()))
            })?
            .into_core()
    }
}

/// One host-owned invocation. Dropping the owner cancels the Rust driver.
/// Cloning an `Arc<Operation>` for an in-flight boundary call preserves its memory.
pub struct Operation {
    shared: Arc<Shared>,
    task: tokio::sync::Mutex<Option<JoinHandle<()>>>,
}
impl Operation {
    /// Start on the current Tokio runtime. No host function is captured.
    /// # Errors
    /// Rejects invalid protocol/options before any provider work.
    pub fn start(model: Arc<dyn LanguageModel>, request: StartRequest) -> Result<Self, AiMuxError> {
        let mut options = request.options()?;
        let control = GenerationControl::new(
            options.timeout.unwrap_or_default(),
            Some(Default::default()),
        )?;
        options.operation_control = Some(control.clone());
        options.abort_signal = control.signal();
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                terminal: None,
                terminal_read: false,
                output: VecDeque::new(),
                pending: None,
                next_id: 1,
            }),
            changed: Notify::new(),
            readers: AtomicU8::new(0),
            control,
        });
        if request.repair_tool_call {
            let state = shared.clone();
            options.repair_tool_call = Some(ToolCallRepair::new(move |context| {
                let state = state.clone();
                async move { state.repair(context).await }
            }));
        }
        let state = shared.clone();
        let task = tokio::spawn(async move {
            // Catch unwinding driver failures, so a receiver never silently hangs.
            use futures::FutureExt;
            let driven =
                std::panic::AssertUnwindSafe(drive(&state, model, request, options)).catch_unwind();
            let result = driven
                .await
                .unwrap_or_else(|_| Err(AiMuxError::Other("operation driver panicked".into())));
            state.finish(result);
        });
        Ok(Self {
            shared,
            task: tokio::sync::Mutex::new(Some(task)),
        })
    }

    /// Receive one control/output event. `Any` is the synchronous host coordinator.
    /// # Errors
    /// OUTPUT/ANY delivers the original terminal core error once.
    pub async fn next(&self, lane: Lane) -> Result<Next, AiMuxError> {
        if lane == Lane::Terminal {
            self.finished().await;
            return Ok(Next::Ended);
        }
        let mask = match lane {
            Lane::Control => 1,
            Lane::Output => 2,
            Lane::Any => 3,
            Lane::Terminal => unreachable!(),
        };
        let claim =
            self.shared
                .readers
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |readers| {
                    (readers & mask == 0).then_some(readers | mask)
                });
        if claim.is_err() {
            return Ok(Next::ReaderBusy);
        }
        struct Reader<'a>(&'a AtomicU8, u8);
        impl Drop for Reader<'_> {
            fn drop(&mut self) {
                self.0.fetch_and(!self.1, Ordering::Release);
            }
        }
        let _reader = Reader(&self.shared.readers, mask);
        loop {
            let notified = self.shared.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            self.shared.observe_stop();
            {
                let mut state = self.shared.state.lock().unwrap();
                if lane != Lane::Output
                    && state.terminal.is_none()
                    && let Some(event) = state
                        .pending
                        .as_mut()
                        .and_then(|pending| pending.request.take())
                {
                    return Ok(Next::Event(event));
                }
                if lane != Lane::Control
                    && let Some(event) = state.output.pop_front()
                {
                    drop(state);
                    self.shared.changed.notify_waiters();
                    return Ok(Next::Event(event));
                }
                if let Some(terminal) = state.terminal.clone() {
                    if lane != Lane::Control && !state.terminal_read {
                        state.terminal_read = true;
                        terminal?;
                    }
                    return Ok(Next::Ended);
                }
            }
            tokio::select! {
                () = &mut notified => {},
                error = self.shared.control.stopped() => self.shared.finish(Err(error)),
            }
        }
    }

    /// Observe termination without consuming the output error or queued results.
    pub async fn finished(&self) {
        loop {
            let notified = self.shared.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            self.shared.observe_stop();
            if self.shared.state.lock().unwrap().terminal.is_some() {
                return;
            }
            tokio::select! {
                () = &mut notified => {},
                error = self.shared.control.stopped() => self.shared.finish(Err(error)),
            }
        }
    }

    /// Atomically accept a reply at most once. Payload decoding happens before this call.
    #[must_use]
    pub fn reply(&self, request_id: &str, reply: Reply) -> ReplyStatus {
        let mut state = self.shared.state.lock().unwrap();
        if state.terminal.is_none()
            && let Some(error) = self.shared.control.failure()
        {
            state.pending = None;
            state.output.clear();
            state.terminal = Some(Err(error));
            self.shared.changed.notify_waiters();
        }
        if state.terminal.is_some() {
            return ReplyStatus::OperationEnded;
        }
        let Ok(id) = request_id.parse::<u64>() else {
            return ReplyStatus::UnknownRequest;
        };
        if id == 0 || request_id != id.to_string() || id >= state.next_id {
            return ReplyStatus::UnknownRequest;
        }
        if state.pending.as_ref().is_none_or(|p| p.id != id) {
            return ReplyStatus::AlreadyReplied;
        }
        let pending = state.pending.take().unwrap();
        // The send is nonblocking and stays inside the state transition.
        let _ = pending.sender.send(reply);
        ReplyStatus::Accepted
    }

    /// Cancel immediately, independently of queue capacity or host work.
    pub fn cancel(&self) {
        self.shared
            .finish(Err(AiMuxError::Aborted("request aborted".into())));
        self.shared.control.cancel();
    }

    /// Cancel and wait for the Rust driver. Never waits for a host callback.
    pub async fn close(&self) {
        self.cancel();
        let mut owner = self.task.lock().await;
        if let Some(mut task) = owner.take() {
            // Give Core a chance to record the cancellation outcome. Then force
            // cancellation of a driver that cannot cooperate; do not detach it.
            if tokio::time::timeout(std::time::Duration::from_secs(1), &mut task)
                .await
                .is_err()
            {
                task.abort();
                let _ = task.await;
            }
        }
    }
}
impl Drop for Operation {
    fn drop(&mut self) {
        self.cancel();
        if let Some(task) = self.task.get_mut().take() {
            task.abort();
        }
    }
}

fn value<T: Serialize>(v: T) -> Result<Value, AiMuxError> {
    serde_json::to_value(v).map_err(|e| AiMuxError::Other(format!("operation serialization: {e}")))
}
async fn drive(
    state: &Shared,
    model: Arc<dyn LanguageModel>,
    request: StartRequest,
    options: GenerateTextOptions,
) -> Result<(), AiMuxError> {
    let stream_options = options
        .provider_options
        .as_ref()
        .and_then(|p| p.get("openai"))
        .and_then(|p| p.get("stream_options"))
        .map(|v| OpenAiStreamOptions {
            include_usage: v
                .get("include_usage")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            include_reasoning: v
                .get("include_reasoning")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        })
        .unwrap_or_default();
    let result = match request.mode {
        Mode::GenerateText => Some(value(
            generate_text(&*model, request.prompt, options).await?,
        )?),
        Mode::GenerateObject => Some(value(
            generate_object(&*model, request.prompt, options).await?,
        )?),
        Mode::GenerateTextAsOpenai => Some(value(
            generate_text_as_openai(&*model, request.prompt, options).await?,
        )?),
        Mode::ConsumeStreamText => Some(value(
            stream_text(&*model, request.prompt, options)
                .await?
                .consume()
                .await?,
        )?),
        Mode::StreamText => {
            let mut stream = stream_text(&*model, request.prompt, options).await?.stream;
            while let Some(part) = stream.next().await {
                let part = match part {
                    Ok(p) => p,
                    Err(e) if e.is_recoverable_stream_error() => {
                        aimux_core::stream_part::StreamPart::Error { error: e }
                    }
                    Err(e) => return Err(e),
                };
                state.send(Event::Part { part: value(part)? }).await?;
            }
            None
        }
        Mode::StreamTextAsOpenai => {
            let mut stream =
                stream_text_as_openai(&*model, request.prompt, options, stream_options)
                    .await?
                    .stream;
            while let Some(part) = stream.next().await {
                state
                    .send(Event::Part {
                        part: value(part?)?,
                    })
                    .await?;
            }
            None
        }
    };
    if let Some(result) = result {
        state.send(Event::Result { result }).await?;
    }
    if let Some(error) = state.control.failure() {
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bounded_output_stops_producer_and_cancel_releases_it() {
        let shared = Shared {
            state: Mutex::new(State {
                terminal: None,
                terminal_read: false,
                output: VecDeque::new(),
                pending: None,
                next_id: 1,
            }),
            changed: Notify::new(),
            readers: AtomicU8::new(0),
            control: GenerationControl::new(Default::default(), Some(Default::default())).unwrap(),
        };
        // A host that never reads must not let the producer accumulate arbitrary output.
        let producer = async {
            for _ in 0..10_000 {
                shared.send(Event::Part { part: Value::Null }).await?;
            }
            Ok::<(), AiMuxError>(())
        };
        tokio::pin!(producer);
        assert!(
            futures::poll!(&mut producer).is_pending(),
            "output accumulated without backpressure"
        );
        shared.finish(Err(AiMuxError::Aborted("stop".into())));
        assert!(matches!(producer.await, Err(AiMuxError::Aborted(_))));
    }
}
