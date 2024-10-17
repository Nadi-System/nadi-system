use nadi_core::node::Node;
use pyo3::prelude::*;

#[pyclass(name = "Node")]
#[derive(Clone)]
pub struct PyNode(pub Node);

#[pymethods]
impl PyNode {
    #[getter]
    fn index(&self) -> PyResult<usize> {
        Ok(self.0.lock().index())
    }

    #[getter]
    fn level(&self) -> PyResult<u64> {
        Ok(self.0.lock().level())
    }

    #[getter]
    fn order(&self) -> PyResult<u64> {
        Ok(self.0.lock().order())
    }

    fn __repr__(&self) -> PyResult<String> {
        let node = self.0.lock();
        Ok(format!("<Node {}: {}>", node.index(), node.name()))
    }
}
