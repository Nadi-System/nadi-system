use crate::attrs::PyAttribute;
use nadi_core::prelude::*;
use nadi_core::template::Template;
use pyo3::exceptions::{PyAttributeError, PyRuntimeError};
use pyo3::prelude::*;
use std::collections::HashSet;
use std::str::FromStr;

#[pyclass(module = "nadi", name = "Node")]
#[repr(transparent)]
#[derive(Clone)]
pub struct PyNode(pub Node);

#[pymethods]
impl PyNode {
    #[getter]
    pub fn name(&self) -> String {
        self.0.name().to_string()
    }

    #[getter]
    pub fn index(&self) -> usize {
        self.0.lock().index()
    }

    #[getter]
    pub fn level(&self) -> u64 {
        self.0.lock().level()
    }

    #[getter]
    pub fn order(&self) -> u64 {
        self.0.lock().order()
    }

    #[getter]
    fn input(&self) -> Option<PyNode> {
        self.0.lock().input().map(|n| PyNode(n.clone())).into()
    }

    #[getter]
    pub fn inputs(&self) -> Vec<PyNode> {
        self.0
            .lock()
            .inputs()
            .iter()
            .map(|n| PyNode(n.clone()))
            .collect()
    }

    #[getter]
    pub fn input_names(&self) -> Vec<String> {
        self.0
            .lock()
            .inputs()
            .iter()
            .map(|n| n.name().to_string())
            .collect()
    }

    #[getter]
    fn output(&self) -> Option<PyNode> {
        self.0.lock().output().map(|n| PyNode(n.clone())).into()
    }

    #[getter]
    fn outputs(&self) -> Vec<PyNode> {
        self.0
            .lock()
            .outputs()
            .iter()
            .map(|n| PyNode(n.clone()))
            .collect()
    }

    #[getter]
    pub fn output_names(&self) -> Vec<String> {
        self.0
            .lock()
            .outputs()
            .iter()
            .map(|n| n.name().to_string())
            .collect()
    }

    fn load_attr(&self, path: String) -> PyResult<()> {
        self.0.lock().load_attr(path)?;
        Ok(())
    }

    #[getter]
    fn attrs(&self) -> HashSet<String> {
        self.0
            .lock()
            .attr_map()
            .keys()
            .map(|k| k.to_string())
            .collect()
    }

    fn move_aside(&mut self) -> PyResult<()> {
        self.0
            .lock()
            .move_aside()
            .map_err(|e| PyRuntimeError::new_err(e))
    }

    // fn move_down(&mut self) {
    //     self.0.lock().move_down();
    // }

    fn render(&self, text: &str) -> PyResult<String> {
        let templ = Template::from_str(text)?;
        let text = templ.render(&self.0.lock())?;
        Ok(text)
    }

    #[pyo3(signature = (name, default=None))]
    fn getattr(&self, name: String, default: Option<PyAttribute>) -> Option<impl IntoPyObject<'_>> {
        match self.0.lock().attr(&name) {
            Some(v) => Some(PyAttribute::from(v.clone())),
            None => default,
        }
    }

    fn __getattr__(&self, name: String) -> PyResult<impl IntoPyObject<'_>> {
        match self.0.lock().attr(&name) {
            Some(v) => Ok(PyAttribute::from(v.clone())),
            None => Err(PyAttributeError::new_err("Attribute Not Found")),
        }
    }

    fn __setattr__(&mut self, name: String, value: PyAttribute) -> PyResult<()> {
        self.0.lock().set_attr(&name, Attribute::from(value));
        Ok(())
    }

    fn __delattr__(&mut self, name: String) {
        self.0.lock().del_attr(&name);
    }

    fn __repr__(&self) -> PyResult<String> {
        let node = self.0.lock();
        Ok(format!("<Node {}: {}>", node.index(), node.name()))
    }
}
