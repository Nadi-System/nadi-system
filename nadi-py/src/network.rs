use crate::node::PyNode;
use nadi_core::network::Network;
use pyo3::{
    exceptions::{PyAttributeError, PyKeyError},
    prelude::*,
};

#[derive(FromPyObject, Clone)]
enum IndOrName {
    Index(usize),
    Name(String),
}

#[pyclass(name = "Network")]
#[derive(Clone)]
pub struct PyNetwork(pub Network);

#[pymethods]
impl PyNetwork {
    #[new]
    fn read_file(filename: String, _attrs_dir: Option<String>) -> PyResult<Self> {
        let net = Network::from_file(&filename)?;
        // if let Some(dir) = attrs_dir {
        //     net.load_attrs(&dir)?
        // }
        Ok(Self(net))
    }

    fn node(&self, ind: IndOrName) -> PyResult<PyNode> {
        let node = match ind {
            IndOrName::Index(i) => self.0.node(i),
            IndOrName::Name(n) => self.0.node_by_name(&n),
        };
        match node {
            Some(n) => Ok(PyNode(n.clone())),
            None => Err(PyKeyError::new_err("Node not found")),
        }
    }

    fn nodes(&self) -> Vec<PyNode> {
        self.0.nodes().map(|n| PyNode(n.clone())).collect()
    }
}
