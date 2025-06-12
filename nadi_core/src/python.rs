use crate::expressions::EvalError;
#[cfg(feature = "parser")]
use crate::parser::ParseError;
pub use pyo3;
use pyo3::{exceptions::*, IntoPyObject, PyErr, PyErrArguments, PyObject, Python};

impl PyErrArguments for EvalError {
    fn arguments(self, py: Python<'_>) -> PyObject {
        self.message()
            .into_pyobject(py)
            .unwrap()
            .into_any()
            .unbind()
    }
}

impl From<EvalError> for PyErr {
    fn from(err: EvalError) -> PyErr {
        match &err {
            EvalError::UnresolvedVariable => PyAttributeError::new_err(err),
            EvalError::AttributeNotFound => PyAttributeError::new_err(err),
            EvalError::NoOutputNode => PyAttributeError::new_err(err),

            EvalError::FunctionNotFound(_, _) => PyKeyError::new_err(err),
            EvalError::NodeNotFound(_) => PyKeyError::new_err(err),

            EvalError::FunctionError(_, _) => PyRuntimeError::new_err(err),
            EvalError::NoReturnValue(_) => PyRuntimeError::new_err(err),
            EvalError::PathNotFound(_, _, _) => PyRuntimeError::new_err(err),
            EvalError::AttributeError(_) => PyRuntimeError::new_err(err),
            EvalError::NodeAttributeError(_, _) => PyRuntimeError::new_err(err),

            EvalError::InvalidOperation => PyValueError::new_err(err),
            EvalError::InvalidVariableType => PyTypeError::new_err(err),
            EvalError::NotANumber => PyTypeError::new_err(err),
            EvalError::NotABool => PyTypeError::new_err(err),

            EvalError::RegexError(_) => PyValueError::new_err(err),
            EvalError::DifferentLength(_, _) => PyAssertionError::new_err(err),
            EvalError::DivideByZero => PyZeroDivisionError::new_err(err),
            EvalError::LogicalError(_) => PyAssertionError::new_err(err),
            EvalError::MutexError(_, _) => PyPermissionError::new_err(err),
        }
    }
}

#[cfg(feature = "parser")]
impl PyErrArguments for ParseError {
    fn arguments(self, py: Python<'_>) -> PyObject {
        self.to_string()
            .into_pyobject(py)
            .unwrap()
            .into_any()
            .unbind()
    }
}

#[cfg(feature = "parser")]
impl From<ParseError> for PyErr {
    fn from(err: ParseError) -> PyErr {
        PySyntaxError::new_err(err.to_string())
    }
}
