use crate::network::PyNetwork;
use nadi_core::attrs::AttrMap;
use nadi_core::parser::{tasks, tokenizer};
use nadi_core::prelude::EvalError;
use nadi_core::tasks::{TaskContext, TaskMessage};
use pyo3::prelude::*;
use std::sync::mpsc::channel;
use std::thread;

#[pyclass(unsendable, module = "nadi", name = "TaskContext")]
#[derive(Clone)]
/// Task Context for NADI, this is used to run tasks as strings
pub struct PyTaskContext(pub TaskContext);

// /// Message
// #[pyclass(module = "nadi", name = "TaskMessage")]
// #[derive(Clone)]
// pub struct PyTaskMessage(pub TaskMessage);

#[pymethods]
impl PyTaskContext {
    #[new]
    #[pyo3(signature = (net=None))]
    fn new(net: Option<PyNetwork>) -> Self {
        let (sender, receiver) = channel::<TaskMessage>();
        thread::spawn(|| {
            for msg in receiver {
                // getting callback function from the python didn't
                // work due to threads problem. (not Sync)
                msg.print();
            }
        });
        Self(TaskContext::new(net.map(|n| n.0), sender))
    }

    /// Clear the context of network and env variables
    fn clear(&mut self) {
        self.0.clear();
    }

    /// Execute the given tasks in the context
    fn execute(&mut self, tasks: String) -> PyResult<Option<String>> {
        let tokens = tokenizer::get_tokens(&tasks);
        let tasks = tasks::parse(tokens)?;
        let mut locals = AttrMap::new();
        let responses: Result<Vec<Option<String>>, EvalError> = tasks
            .into_iter()
            .map(|t| self.0.execute(t, &mut locals))
            .collect();
        match responses {
            Ok(v) => {
                // This will be better once we return values from task
                // execution instead of strings
                let vals: Vec<String> = v.into_iter().flatten().collect();
                if vals.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(vals.join("\n")))
                }
            }
            Err(e) => Err(e.into()),
        }
    }
}
