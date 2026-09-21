//! Data transport: Python wrapper owns and executes user functions.
use crate::error::{serialize_result, to_py_err, wire_json};
use aimux_operation::{Lane, Next, Operation, Reply};
use pyo3::prelude::*;

#[pyclass]
pub struct HostOperation {
    pub(crate) inner: Operation,
}

#[pymethods]
impl HostOperation {
    #[pyo3(signature = (lane=2))]
    fn next(&self, py: Python<'_>, lane: u32) -> PyResult<Option<String>> {
        let lane = match lane {
            0 => Lane::Control,
            1 => Lane::Output,
            2 => Lane::Any,
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "invalid operation lane",
                ));
            }
        };
        let next = py
            .allow_threads(|| crate::runtime().block_on(self.inner.next(lane)))
            .map_err(|e| to_py_err(&e))?;
        match next {
            Next::Ended => Ok(None),
            Next::ReaderBusy => Err(pyo3::exceptions::PyRuntimeError::new_err(
                "concurrent operation reader",
            )),
            Next::Event(event) => serialize_result(&event).map(Some),
        }
    }
    fn reply(&self, request_id: &str, reply_json: &str) -> PyResult<u32> {
        let reply: Reply = wire_json("reply_json", reply_json)?;
        Ok(self.inner.reply(request_id, reply) as u32)
    }
    fn cancel(&self) {
        self.inner.cancel();
    }
    fn finished(&self, py: Python<'_>) {
        py.allow_threads(|| crate::runtime().block_on(self.inner.finished()));
    }
    fn close(&self, py: Python<'_>) {
        py.allow_threads(|| crate::runtime().block_on(self.inner.close()));
    }
}
