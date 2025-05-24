use crate::attrs::PyAttribute;
use nadi_core::prelude::*;
use nadi_core::string_template::Template;
use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::*;
use std::collections::HashSet;

#[pyclass(module = "nadi", name = "Node")]
#[repr(transparent)]
#[derive(Clone)]
pub struct PyNode(pub Node);

#[pymethods]
impl PyNode {
    #[getter]
    pub fn name(&self) -> String {
        self.0.lock().name().to_string()
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

    pub fn inputs(&self) -> Vec<PyNode> {
        self.0
            .lock()
            .inputs()
            .iter()
            .map(|n| PyNode(n.clone()))
            .collect()
    }

    fn output(&self) -> Option<PyNode> {
        self.0.lock().output().map(|n| PyNode(n.clone())).into()
    }

    fn load_attr(&self, path: String) -> PyResult<()> {
        self.0.lock().load_attr(path)?;
        Ok(())
    }

    fn attrs(&self) -> HashSet<String> {
        self.0
            .lock()
            .attr_map()
            .keys()
            .map(|k| k.to_string())
            .collect()
    }

    fn move_aside(&mut self) {
        self.0.lock().move_aside();
    }

    fn move_down(&mut self) {
        self.0.lock().move_down();
    }

    fn render(&self, text: &str) -> PyResult<String> {
        let templ = Template::parse_template(text)?;
        let text = self.0.lock().render(&templ)?;
        Ok(text)
    }

    #[pyo3(signature = (name, default=None))]
    fn getattr(&self, name: String, default: Option<PyAttribute>) -> Option<impl IntoPyObject> {
        match self.0.lock().attr(&name) {
            Some(v) => Some(PyAttribute::from(v.clone())),
            None => default,
        }
    }

    fn __getattr__(&self, name: String) -> PyResult<impl IntoPyObject> {
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
