use crate::network::PyNetwork;
use nadi_core::parser::{tasks, tokenizer};
use nadi_core::tasks::TaskContext;
use pyo3::{exceptions::PyRuntimeError, prelude::*};

#[pyclass(unsendable, module = "nadi", name = "TaskContext")]
#[derive(Clone)]
/// Task Context for NADI, this is used to run tasks as strings
pub struct PyTaskContext(pub TaskContext);

#[pymethods]
impl PyTaskContext {
    #[new]
    #[pyo3(signature = (net=None))]
    fn new(net: Option<PyNetwork>) -> Self {
        Self(TaskContext::new(net.map(|n| n.0)))
    }

    /// Clear the context of network and env variables
    fn clear(&mut self) {
        self.0.clear();
    }

    /// Execute the given tasks in the context
    fn execute(&mut self, tasks: String) -> PyResult<Option<String>> {
        let tokens = tokenizer::get_tokens(&tasks);
        let tasks = tasks::parse(tokens)?;
        let responses: Result<Vec<Option<String>>, String> =
            tasks.into_iter().map(|t| self.0.execute(t)).collect();
        match responses {
            Ok(v) => {
                // This will be better once we return values from task
                // execution instead of strings
                let vals: Vec<String> = v.into_iter().filter_map(|t| t).collect();
                if vals.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(vals.join("\n")))
                }
            }
            Err(e) => Err(PyRuntimeError::new_err(e)),
        }
    }
}
