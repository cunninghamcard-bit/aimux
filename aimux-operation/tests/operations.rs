use aimux_core::{AiMuxError, LanguageModel};
use aimux_core::{
    generate::{GenerateTextOptions, generate_text},
    message::ModelPrompt,
    options::CallOptions,
    parse_tool_call::ToolCallRepair,
    result::{GenerateContent, GenerateResult, StreamResult},
    stream_part::StreamPart,
    tool::{FunctionTool, Tool},
    types::*,
};
use aimux_operation::{Event, Lane, Mode, Next, Operation, Reply, ReplyStatus, StartRequest};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Model {
    requests: AtomicUsize,
    prefix: usize,
}
#[async_trait::async_trait]
impl LanguageModel for Model {
    fn provider(&self) -> &str {
        "mock"
    }
    fn model_id(&self) -> &str {
        "repair"
    }
    async fn do_generate(&self, _: &CallOptions) -> Result<GenerateResult, AiMuxError> {
        self.requests.fetch_add(1, Ordering::Relaxed);
        Ok(GenerateResult {
            content: vec![GenerateContent::ToolCall {
                tool_call_id: "c1".into(),
                tool_name: "weather".into(),
                input: "{".into(),
                provider_executed: None,
                dynamic: None,
                thought_signature: None,
                provider_metadata: None,
            }],
            finish_reason: FinishReason {
                unified: FinishReasonUnified::ToolCalls,
                raw: None,
            },
            usage: Usage::default(),
            warnings: vec![],
            provider_metadata: None,
            response: ResponseMetadata::default(),
            request_body: None,
            response_headers: None,
        })
    }
    async fn do_stream(&self, _: &CallOptions) -> Result<StreamResult, AiMuxError> {
        let mut parts = (0..self.prefix)
            .map(|_| {
                Ok(StreamPart::TextDelta {
                    id: "text".into(),
                    delta: "x".into(),
                    provider_metadata: None,
                })
            })
            .collect::<Vec<_>>();
        parts.extend([
            Ok(StreamPart::ToolCall {
                tool_call_id: "c1".into(),
                tool_name: "weather".into(),
                input: json!("{"),
                provider_executed: None,
                dynamic: None,
                thought_signature: None,
                provider_metadata: None,
                invalid: None,
                error: None,
            }),
            Ok(StreamPart::Finish {
                finish_reason: FinishReason {
                    unified: FinishReasonUnified::ToolCalls,
                    raw: None,
                },
                usage: Usage::default(),
                provider_metadata: None,
            }),
        ]);
        Ok(StreamResult {
            stream: Box::pin(futures::stream::iter(parts)),
            request_body: None,
            response_headers: None,
        })
    }
}
fn model() -> Arc<Model> {
    Arc::new(Model {
        requests: AtomicUsize::new(0),
        prefix: 0,
    })
}
fn options() -> serde_json::Value {
    json!({"tools":[{"type":"function","name":"weather","input_schema":{"type":"object"}}]})
}
fn request(mode: Mode) -> StartRequest {
    StartRequest {
        protocol_version: 1,
        mode,
        prompt: ModelPrompt::Text("hi".into()),
        options: options(),
        repair_tool_call: true,
    }
}
async fn repair_id(op: &Operation) -> String {
    match op.next(Lane::Control).await.unwrap() {
        Next::Event(Event::RepairRequest {
            request_id,
            context,
        }) => {
            assert_eq!(context.tool_call.input, "{");
            assert_eq!(context.input_schema, json!({"type":"object"}));
            request_id
        }
        other => panic!("{other:?}"),
    }
}
fn fixed() -> Reply {
    serde_json::from_value(json!({"type":"repaired","tool_call":{"tool_call_id":"c1","tool_name":"weather","input":"{}"}})).unwrap()
}

#[tokio::test]
async fn repair_result_and_stale_replies() {
    let m = model();
    let op = Operation::start(m.clone(), request(Mode::GenerateText)).unwrap();
    let id = repair_id(&op).await;
    assert_eq!(
        op.reply("99", Reply::Unchanged),
        ReplyStatus::UnknownRequest
    );
    assert_eq!(op.reply(&id, fixed()), ReplyStatus::Accepted);
    assert_eq!(op.reply(&id, Reply::Unchanged), ReplyStatus::AlreadyReplied);
    match op.next(Lane::Output).await.unwrap() {
        Next::Event(Event::Result { result }) => {
            assert_eq!(result["tool_calls"][0]["input"], json!({}));
            assert!(result["tool_calls"][0].get("invalid").is_none());
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(op.next(Lane::Output).await.unwrap(), Next::Ended));
    assert_eq!(op.reply(&id, fixed()), ReplyStatus::OperationEnded);
    assert_eq!(m.requests.load(Ordering::Relaxed), 1);
    op.close().await;
}

#[tokio::test]
async fn cancel_during_host_wait_preserves_error_and_wakes_both_lanes() {
    let op = Operation::start(model(), request(Mode::GenerateText)).unwrap();
    let id = repair_id(&op).await;
    op.cancel();
    assert_eq!(op.reply(&id, fixed()), ReplyStatus::OperationEnded);
    assert!(matches!(op.next(Lane::Control).await.unwrap(), Next::Ended));
    assert!(matches!(
        op.next(Lane::Output).await,
        Err(AiMuxError::Aborted(_))
    ));
    assert!(matches!(op.next(Lane::Output).await.unwrap(), Next::Ended));
    op.close().await;
}

#[tokio::test(start_paused = true)]
async fn original_deadline_bounds_repair() {
    let mut req = request(Mode::GenerateText);
    req.options["timeout"] = json!({"total_ms":50});
    let op = Operation::start(model(), req).unwrap();
    let id = repair_id(&op).await;
    tokio::time::advance(std::time::Duration::from_millis(51)).await;
    assert_eq!(op.reply(&id, fixed()), ReplyStatus::OperationEnded);
    assert!(matches!(
        op.next(Lane::Output).await,
        Err(AiMuxError::Timeout(_))
    ));
    op.close().await;
}

#[tokio::test]
async fn host_error_is_a_tool_error_not_an_operation_failure() {
    let op = Operation::start(model(), request(Mode::GenerateText)).unwrap();
    let id = repair_id(&op).await;
    assert_eq!(
        op.reply(
            &id,
            Reply::Failed {
                message: "boom".into()
            }
        ),
        ReplyStatus::Accepted
    );
    match op.next(Lane::Output).await.unwrap() {
        Next::Event(Event::Result { result }) => assert_eq!(
            result["tool_calls"][0]["error"]["ToolCallRepair"]["cause"]["Other"],
            "boom"
        ),
        other => panic!("{other:?}"),
    }
    op.close().await;
}

#[tokio::test]
async fn streaming_and_nested_operation_use_same_protocol() {
    let outer = Operation::start(model(), request(Mode::StreamText)).unwrap();
    let id = repair_id(&outer).await;
    let inner = Operation::start(model(), request(Mode::GenerateText)).unwrap();
    let inner_id = repair_id(&inner).await;
    assert_eq!(inner.reply(&inner_id, fixed()), ReplyStatus::Accepted);
    assert!(matches!(
        inner.next(Lane::Output).await.unwrap(),
        Next::Event(Event::Result { .. })
    ));
    inner.close().await;
    assert_eq!(outer.reply(&id, fixed()), ReplyStatus::Accepted);
    let mut repaired = false;
    loop {
        match outer.next(Lane::Output).await.unwrap() {
            Next::Event(Event::Part { part }) if part.get("ToolCall").is_some() => {
                assert_eq!(part["ToolCall"]["input"], json!({}));
                repaired = true;
            }
            Next::Ended => break,
            _ => {}
        }
    }
    assert!(repaired);
    outer.close().await;
}

#[tokio::test(start_paused = true)]
async fn direct_rust_repair_has_the_same_deadline() {
    let opts = GenerateTextOptions {
        tools: Some(vec![Tool::Function(FunctionTool::new(
            "weather",
            json!({"type":"object"}),
        ))]),
        timeout: Some(aimux_core::options::TimeoutConfiguration {
            total_ms: Some(50),
            ..Default::default()
        }),
        repair_tool_call: Some(ToolCallRepair::new(|_| async {
            std::future::pending().await
        })),
        ..Default::default()
    };
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        generate_text(&*model(), "hi", opts),
    )
    .await
    .expect("repair exceeded the operation deadline");
    assert!(matches!(result, Err(AiMuxError::Timeout(_))));
}

#[tokio::test]
async fn invalid_start_has_no_provider_side_effect() {
    let m = model();
    let mut req = request(Mode::GenerateText);
    req.protocol_version = 99;
    assert!(Operation::start(m.clone(), req).is_err());
    let mut req = request(Mode::GenerateText);
    req.options["repair_tool_call"] = json!(12);
    assert!(Operation::start(m.clone(), req).is_err());
    assert_eq!(m.requests.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn unchanged_and_invalid_repair_preserve_tool_error_semantics() {
    for reply in [Reply::Unchanged, serde_json::from_value(json!({"type":"repaired","tool_call":{"tool_call_id":"c1","tool_name":"missing","input":"{}"}})).unwrap()] {
        let op = Operation::start(model(), request(Mode::GenerateText)).unwrap();
        let id = repair_id(&op).await;
        let unchanged = matches!(reply, Reply::Unchanged);
        assert_eq!(op.reply(&id, reply), ReplyStatus::Accepted);
        match op.next(Lane::Output).await.unwrap() {
            Next::Event(Event::Result { result }) => {
                assert_eq!(result["tool_calls"][0]["invalid"], true);
                assert_eq!(result["tool_calls"][0]["error"].get("ToolCallRepair").is_none(), unchanged);
            }, other => panic!("{other:?}"),
        }
        assert!(matches!(op.next(Lane::Control).await.unwrap(), Next::Ended));
        op.close().await;
    }
}

#[tokio::test]
async fn single_reader_claim_is_released_when_wait_is_dropped() {
    let op = Operation::start(model(), request(Mode::GenerateText)).unwrap();
    let _id = repair_id(&op).await;
    let mut first = Box::pin(op.next(Lane::Output));
    assert!(futures::poll!(&mut first).is_pending());
    assert!(matches!(
        tokio::time::timeout(std::time::Duration::from_millis(50), op.next(Lane::Output))
            .await
            .expect("single-reader guard missing")
            .unwrap(),
        Next::ReaderBusy
    ));
    assert!(matches!(
        op.next(Lane::Any).await.unwrap(),
        Next::ReaderBusy
    ));
    drop(first);
    op.cancel();
    assert!(matches!(
        op.next(Lane::Any).await,
        Err(AiMuxError::Aborted(_))
    ));
    op.close().await;
}

#[tokio::test]
async fn terminal_observer_never_consumes_results_or_errors() {
    let op = Operation::start(model(), request(Mode::GenerateText)).unwrap();
    let id = repair_id(&op).await;
    assert_eq!(op.reply(&id, fixed()), ReplyStatus::Accepted);
    assert!(matches!(
        op.next(Lane::Terminal).await.unwrap(),
        Next::Ended
    ));
    assert!(matches!(
        op.next(Lane::Output).await.unwrap(),
        Next::Event(Event::Result { .. })
    ));
    op.close().await;
}

#[tokio::test]
async fn cancellation_and_close_do_not_require_output_drain() {
    let m = Arc::new(Model {
        requests: AtomicUsize::new(0),
        prefix: 200,
    });
    let op = Operation::start(m, request(Mode::StreamText)).unwrap();
    // Let the driver fill OUTPUT. Future control requests can only be produced
    // when Core is polled again; cancellation must not depend on that progress.
    let _ = op.next(Lane::Output).await.unwrap();
    tokio::task::yield_now().await;
    op.cancel();
    tokio::join!(op.close(), op.close());
    assert!(matches!(
        op.next(Lane::Output).await,
        Err(AiMuxError::Aborted(_))
    ));
}

#[tokio::test(start_paused = true)]
async fn control_request_is_available_with_a_full_output_queue() {
    let m = Arc::new(Model {
        requests: AtomicUsize::new(0),
        prefix: 64,
    });
    let op = Operation::start(m, request(Mode::StreamText)).unwrap();
    // There is no OUTPUT receiver. Issued repair requests have a separate path.
    let id = tokio::time::timeout(std::time::Duration::from_secs(1), repair_id(&op))
        .await
        .expect("control was blocked behind output");
    assert_eq!(op.reply(&id, fixed()), ReplyStatus::Accepted);
    op.close().await;
}

#[tokio::test]
async fn openai_stream_exposes_only_repaired_arguments() {
    let op = Operation::start(model(), request(Mode::StreamTextAsOpenai)).unwrap();
    let id = repair_id(&op).await;
    assert_eq!(op.reply(&id, fixed()), ReplyStatus::Accepted);
    let mut arguments = String::new();
    loop {
        match op.next(Lane::Output).await.unwrap() {
            Next::Event(Event::Part { part }) => {
                if let Some(s) =
                    part["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"].as_str()
                {
                    arguments.push_str(s);
                }
            }
            Next::Ended => break,
            _ => {}
        }
    }
    assert_eq!(arguments, "{}");
    op.close().await;
}

#[tokio::test]
async fn no_hook_preserves_invalid_calls_without_control_requests() {
    let mut req = request(Mode::GenerateText);
    req.repair_tool_call = false;
    let op = Operation::start(model(), req).unwrap();
    assert!(matches!(op.next(Lane::Control).await.unwrap(), Next::Ended));
    match op.next(Lane::Output).await.unwrap() {
        Next::Event(Event::Result { result }) => {
            assert_eq!(result["tool_calls"][0]["invalid"], true)
        }
        other => panic!("{other:?}"),
    }
    op.close().await;
}

#[tokio::test(start_paused = true)]
async fn cancel_cannot_replace_an_existing_timeout_or_success() {
    let mut req = request(Mode::GenerateText);
    req.options["timeout"] = json!({"total_ms": 50});
    let op = Operation::start(model(), req).unwrap();
    let _id = repair_id(&op).await;
    op.finished().await;
    op.cancel();
    assert!(matches!(
        op.next(Lane::Output).await,
        Err(AiMuxError::Timeout(_))
    ));
    op.close().await;

    let mut req = request(Mode::GenerateText);
    req.repair_tool_call = false;
    let op = Operation::start(model(), req).unwrap();
    op.finished().await;
    op.cancel();
    assert!(matches!(
        op.next(Lane::Output).await.unwrap(),
        Next::Event(Event::Result { .. })
    ));
    op.close().await;
}
