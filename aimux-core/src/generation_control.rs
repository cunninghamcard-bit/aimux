//! One cancellation signal and absolute budget shared by a generation and its host driver.
use crate::{AbortSignal, AiMuxError, options::TimeoutConfiguration, timeout};

#[derive(Clone, Debug)]
pub struct GenerationControl {
    pub(crate) timeout: timeout::OperationTimeout,
    pub(crate) first_chunk: Option<timeout::TimeoutDeadline>,
    signal: Option<AbortSignal>,
}
impl GenerationControl {
    /// Start a generation budget. Reuse this object across all phases.
    /// # Errors
    /// Invalid timeout durations are rejected before generation starts.
    pub fn new(
        configuration: TimeoutConfiguration,
        signal: Option<AbortSignal>,
    ) -> Result<Self, AiMuxError> {
        Ok(Self {
            timeout: timeout::OperationTimeout::new(configuration)?,
            first_chunk: configuration
                .first_chunk_ms
                .map(|ms| timeout::TimeoutDeadline::from_now("First chunk", ms))
                .transpose()?,
            signal,
        })
    }
    #[must_use]
    pub fn signal(&self) -> Option<AbortSignal> {
        self.signal.clone()
    }
    pub fn cancel(&self) {
        if let Some(signal) = &self.signal {
            signal.abort();
        }
    }
    /// Observe cancellation or budget exhaustion without resetting the deadline.
    #[must_use]
    pub fn failure(&self) -> Option<AiMuxError> {
        if let Some(signal) = self.signal.as_ref().filter(|s| s.is_aborted()) {
            return Some(AiMuxError::from_abort_signal(signal));
        }
        self.timeout
            .deadline()
            .filter(|d| d.at <= tokio::time::Instant::now())
            .map(timeout::TimeoutDeadline::error)
    }
    /// Wait for cancellation or the original total/step deadline.
    pub async fn stopped(&self) -> AiMuxError {
        tokio::select! {
            biased;
            () = timeout::wait_for_abort(self.signal.as_ref()) => AiMuxError::from_abort_signal(self.signal.as_ref().expect("armed abort")),
            () = timeout::wait_for_deadline(self.timeout.deadline()) => self.timeout.deadline().expect("armed deadline").error(),
        }
    }
    /// Run one phase within the original operation budget.
    /// # Errors
    /// Returns cancellation, timeout, or the phase's own error.
    pub async fn run<T>(
        &self,
        future: impl std::future::Future<Output = Result<T, AiMuxError>>,
    ) -> Result<T, AiMuxError> {
        timeout::run(future, self.signal.as_ref(), self.timeout).await
    }
}
