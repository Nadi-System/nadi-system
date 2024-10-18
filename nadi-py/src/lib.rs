use pyo3::prelude::*;

mod attrs;
mod functions;
mod network;
mod node;

/// A Python module implemented in Rust.
#[pymodule]
fn nadi_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<node::PyNode>()?;
    m.add_class::<network::PyNetwork>()?;
    m.add_class::<attrs::PyAttribute>()?;
    m.add_class::<functions::PyNadiFunctions>()?;
    Ok(())
}
