//! Data-only host operations (RFC-0035). No native-to-host user callback.
use super::*;
use aimux_operation::{Lane, Next, Operation, Reply, StartRequest};

fn operation_of(handle: u64) -> FfiResult<Arc<Operation>> {
    match entry_of(handle, "operation")? {
        HandleEntry::Operation(operation) => Ok(operation),
        _ => Err(FfiError::InvalidHandle {
            expected: "operation",
        }
        .into()),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn aimux_operation_start(
    model: u64,
    request_json: *const c_char,
    out_operation: *mut u64,
) -> *mut aimux_error_t {
    with_out_handle(out_operation, || {
        let model = model_of(model)?;
        let request: StartRequest = parse_json_arg(request_json, "request_json")?;
        let _entered = runtime().enter();
        let operation = Operation::start(model, request)?;
        Ok(intern_handle(HandleEntry::Operation(Arc::new(operation))))
    })
}

/// EVENT=0, WAIT_TIMEOUT=1, ENDED=2, READER_BUSY=3. Lane 2 coordinates
/// CONTROL and OUTPUT on a synchronous host's calling thread.
#[unsafe(no_mangle)]
pub extern "C" fn aimux_operation_next(
    handle: u64,
    lane: i32,
    wait_ms: i64,
    out_event_json: *mut *mut c_char,
    out_state: *mut i32,
) -> *mut aimux_error_t {
    if !out_event_json.is_null() {
        unsafe { *out_event_json = std::ptr::null_mut() };
    }
    if !out_state.is_null() {
        unsafe { *out_state = 2 };
    }
    let result: FfiResult<(i32, Option<String>)> = (|| {
        if out_event_json.is_null() {
            return Err(FfiError::NullPointer {
                argument: "out_event_json",
            }
            .into());
        }
        if out_state.is_null() {
            return Err(FfiError::NullPointer {
                argument: "out_state",
            }
            .into());
        }
        let lane = match lane {
            0 => Lane::Control,
            1 => Lane::Output,
            2 => Lane::Any,
            3 => Lane::Terminal,
            _ => return Err(AiMuxError::InvalidArgument("invalid operation lane".into()).into()),
        };
        if wait_ms < -1 {
            return Err(
                AiMuxError::InvalidArgument("wait_ms must be -1 or nonnegative".into()).into(),
            );
        }
        let operation = operation_of(handle)?;
        let next = ffi_block_on(async {
            if wait_ms == -1 {
                return Some(operation.next(lane).await);
            }
            tokio::time::timeout(
                std::time::Duration::from_millis(wait_ms as u64),
                operation.next(lane),
            )
            .await
            .ok()
        })?;
        Ok(match next {
            None => (1, None),
            Some(Ok(Next::Ended)) => (2, None),
            Some(Ok(Next::ReaderBusy)) => (3, None),
            Some(Ok(Next::Event(event))) => (0, Some(to_json(&event)?)),
            Some(Err(error)) => return Err(error.into()),
        })
    })();
    finish(result, |(state, event)| unsafe {
        *out_state = state;
        if let Some(event) = event {
            *out_event_json = into_cstring_raw(event);
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn aimux_operation_reply(
    handle: u64,
    request_id: *const c_char,
    reply_json: *const c_char,
    out_status: *mut i32,
) -> *mut aimux_error_t {
    if !out_status.is_null() {
        unsafe { *out_status = 3 };
    }
    let result: FfiResult<i32> = (|| {
        if out_status.is_null() {
            return Err(FfiError::NullPointer {
                argument: "out_status",
            }
            .into());
        }
        let operation = operation_of(handle)?;
        let id = str_arg(request_id, "request_id")?;
        let reply: Reply = parse_json_arg(reply_json, "reply_json")?;
        Ok(operation.reply(&id, reply) as i32)
    })();
    finish(result, |status| unsafe { *out_status = status })
}

#[unsafe(no_mangle)]
pub extern "C" fn aimux_operation_cancel(handle: u64) -> *mut aimux_error_t {
    no_result(|| {
        operation_of(handle)?.cancel();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn aimux_operation_drop(handle: u64) {
    // Do not allow a wrong-typed handle to release an unrelated model.
    let removed = {
        let mut registry = registry().lock().expect("aimux registry poisoned");
        if matches!(registry.get(&handle), Some(HandleEntry::Operation(_))) {
            registry.remove(&handle)
        } else {
            None
        }
    };
    if let Some(HandleEntry::Operation(operation)) = removed {
        operation.cancel();
        let _ = ffi_block_on(operation.close());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aimux_core::{
        LanguageModel,
        options::CallOptions,
        result::{GenerateResult, StreamResult},
    };
    use std::ffi::CString;

    struct PendingModel;
    #[async_trait::async_trait]
    impl LanguageModel for PendingModel {
        fn provider(&self) -> &str {
            "test"
        }
        fn model_id(&self) -> &str {
            "pending"
        }
        async fn do_generate(&self, _: &CallOptions) -> Result<GenerateResult, AiMuxError> {
            std::future::pending().await
        }
        async fn do_stream(&self, _: &CallOptions) -> Result<StreamResult, AiMuxError> {
            std::future::pending().await
        }
    }
    fn start() -> u64 {
        let model = intern_handle(HandleEntry::Language(Arc::new(PendingModel)));
        let request =
            CString::new(r#"{"protocol_version":1,"mode":"generate_text","prompt":"hi"}"#).unwrap();
        let mut handle = 0;
        assert!(aimux_operation_start(model, request.as_ptr(), &mut handle).is_null());
        aimux_drop_handle(model);
        handle
    }
    #[test]
    fn c_poll_cancel_error_ownership_and_repeated_drop() {
        let handle = start();
        let mut event = std::ptr::dangling_mut();
        let mut state = -1;
        assert!(aimux_operation_next(handle, 1, 0, &mut event, &mut state).is_null());
        assert_eq!(state, 1);
        assert!(event.is_null());
        let reply = CString::new(r#"{"type":"unchanged"}"#).unwrap();
        let id = CString::new("1").unwrap();
        let mut status = -1;
        assert!(aimux_operation_reply(handle, id.as_ptr(), reply.as_ptr(), &mut status).is_null());
        assert_eq!(status, 2);
        assert!(aimux_operation_cancel(handle).is_null());
        let error = aimux_operation_next(handle, 1, -1, &mut event, &mut state);
        assert!(!error.is_null());
        assert!(event.is_null());
        assert_eq!(aimux_error_code(error), AIMUX_E_ABORTED);
        aimux_error_free(error);
        assert!(aimux_operation_next(handle, 1, -1, &mut event, &mut state).is_null());
        assert_eq!(state, 2);
        aimux_operation_drop(handle);
        aimux_operation_drop(handle);
        aimux_drop_handle(handle);
    }
    #[test]
    fn both_drop_paths_wake_in_flight_receivers() {
        for dropper in [aimux_operation_drop, aimux_drop_handle] {
            let handle = start();
            let operation = operation_of(handle).ok().unwrap();
            let ready = Arc::new(std::sync::Barrier::new(2));
            let child_ready = ready.clone();
            let reader = std::thread::spawn(move || {
                child_ready.wait();
                runtime().block_on(operation.next(Lane::Output))
            });
            ready.wait();
            dropper(handle);
            assert!(matches!(
                reader.join().unwrap(),
                Err(AiMuxError::Aborted(_))
            ));
            assert!(operation_of(handle).is_err());
        }
    }
    #[test]
    fn malformed_reply_and_null_outputs_do_not_consume_operation() {
        let handle = start();
        let mut event = std::ptr::dangling_mut();
        let error = aimux_operation_next(handle, 1, 0, &mut event, std::ptr::null_mut());
        assert!(!error.is_null());
        assert!(event.is_null());
        aimux_error_free(error);
        let id = CString::new("1").unwrap();
        let invalid = CString::new("{}").unwrap();
        let mut status = -1;
        let error = aimux_operation_reply(handle, id.as_ptr(), invalid.as_ptr(), &mut status);
        assert!(!error.is_null());
        assert_eq!(status, 3);
        aimux_error_free(error);
        assert!(operation_of(handle).is_ok());
        aimux_operation_drop(handle);
    }
}
