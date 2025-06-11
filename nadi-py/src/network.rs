use crate::node::PyNode;
use nadi_core::network::{Network, PropOrder, StrPath};
use pyo3::{
    exceptions::{PyKeyError, PyValueError},
    prelude::*,
};
use std::str::FromStr;

#[derive(FromPyObject, Clone)]
enum NodeIndOrName {
    Index(usize),
    Name(String),
    Obj(PyNode),
}

#[pyclass(module = "nadi", name = "Network")]
#[repr(transparent)]
#[derive(Clone)]
pub struct PyNetwork(pub Network);

#[pymethods]
impl PyNetwork {
    #[new]
    #[pyo3(signature = (filename, attrs_dir=None))]
    fn read_file(filename: String, attrs_dir: Option<String>) -> PyResult<Self> {
        let net = Network::from_file(&filename)?;
        if let Some(dir) = attrs_dir {
            net.load_attrs(&dir)?
        }
        Ok(Self(net))
    }

    #[staticmethod]
    fn from_str(network: String) -> PyResult<Self> {
        let net = Network::from_str(&network)?;
        Ok(Self(net))
    }

    #[staticmethod]
    fn from_edges(edges: Vec<(String, String)>) -> PyResult<Self> {
        let edges: Vec<(&str, &str)> = edges.iter().map(|p| (p.0.as_str(), p.1.as_str())).collect();
        let net = Network::from_edges(&edges).map_err(PyValueError::new_err)?;
        Ok(Self(net))
    }

    fn node(&self, ind: NodeIndOrName) -> PyResult<PyNode> {
        let node = match ind {
            NodeIndOrName::Index(i) => self.0.node(i),
            NodeIndOrName::Name(n) => self.0.node_by_name(&n),
            // if node object, only return it if that node is in network
            NodeIndOrName::Obj(n) => self.0.node_by_name(n.0.lock().name()),
        };
        match node {
            Some(n) => Ok(PyNode(n.clone())),
            None => Err(PyKeyError::new_err("Node not found in the network")),
        }
    }

    /// Will return empty vec if the path doesn't exist
    fn nodes_path(&self, start: NodeIndOrName, end: NodeIndOrName) -> PyResult<Vec<PyNode>> {
        let start = self.node(start)?.name();
        let end = self.node(end)?.name();
        let path = StrPath::new(start.into(), end.into());
        let path = self
            .0
            .nodes_path(&PropOrder::Auto, &path)
            .map_err(PyKeyError::new_err)?
            .into_iter()
            .map(PyNode)
            .collect();
        Ok(path)
    }

    #[getter]
    fn nodes(&self) -> Vec<PyNode> {
        self.0.nodes().map(|n| PyNode(n.clone())).collect()
    }

    fn nodes_rev(&self) -> Vec<PyNode> {
        self.0.nodes_rev().map(|n| PyNode(n.clone())).collect()
    }

    fn node_names(&self) -> Vec<String> {
        self.0.node_names().map(|s| s.to_string()).collect()
    }

    fn nodes_count(&self) -> usize {
        self.0.nodes_count()
    }

    // fn nodes_propagation(&self, )
}
