use crate::expressions::{EvalError, EvalErrorType};
#[cfg(feature = "parser")]
use crate::parser::ParseError;
use crate::template::TemplateError;
pub use pyo3;
use pyo3::{exceptions::*, IntoPyObject, PyErr, PyErrArguments, PyObject, Python};

impl PyErrArguments for EvalError {
    fn arguments(self, py: Python<'_>) -> PyObject {
        self.to_string()
            .into_pyobject(py)
            .unwrap()
            .into_any()
            .unbind()
    }
}

impl From<EvalError> for PyErr {
    fn from(err: EvalError) -> PyErr {
        match &err.ty {
            EvalErrorType::UnresolvedVariable => PyAttributeError::new_err(err),
            EvalErrorType::AttributeNotFound => PyAttributeError::new_err(err),
            EvalErrorType::SeriesNotFound(_) => PyAttributeError::new_err(err),
            EvalErrorType::TimeSeriesNotFound(_) => PyAttributeError::new_err(err),
            EvalErrorType::NoInputNodes => PyAttributeError::new_err(err),
            EvalErrorType::NoOutputNode => PyAttributeError::new_err(err),
            EvalErrorType::NoRootNode => PyAttributeError::new_err(err),

            EvalErrorType::IndexError => PyIndexError::new_err(err),

            EvalErrorType::FunctionNotFound(_, _) => PyKeyError::new_err(err),
            EvalErrorType::NodeNotFound(_) => PyKeyError::new_err(err),

            EvalErrorType::FunctionError(_, _) => PyRuntimeError::new_err(err),
            EvalErrorType::UserError(_) => PyRuntimeError::new_err(err),
            EvalErrorType::UnknownFunctionType => PyKeyError::new_err(err),
            EvalErrorType::NoReturnValue(_) => PyRuntimeError::new_err(err),
            EvalErrorType::InvalidReturn(_) => PyRuntimeError::new_err(err),
            EvalErrorType::PathNotFound(_, _, _) => PyRuntimeError::new_err(err),
            EvalErrorType::AttributeError(_) => PyRuntimeError::new_err(err),
            EvalErrorType::NodeAttributeError(_, _) => PyRuntimeError::new_err(err),

            EvalErrorType::InvalidOperation => PyValueError::new_err(err),
            EvalErrorType::InvalidVariableType => PyTypeError::new_err(err),
            EvalErrorType::NotAnArray => PyTypeError::new_err(err),
            EvalErrorType::NotANumber => PyTypeError::new_err(err),
            EvalErrorType::NotABool => PyTypeError::new_err(err),

            EvalErrorType::RenderError(_) => PyNameError::new_err(err),
            EvalErrorType::RegexError(_) => PyValueError::new_err(err),
            EvalErrorType::ParseError(_) => PyValueError::new_err(err),
            EvalErrorType::DifferentLength(_, _) => PyAssertionError::new_err(err),
            EvalErrorType::DivideByZero => PyZeroDivisionError::new_err(err),
            EvalErrorType::LogicalError(_) => PyAssertionError::new_err(err),
            EvalErrorType::MutexError(_, _) => PyPermissionError::new_err(err),
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

impl PyErrArguments for TemplateError {
    fn arguments(self, py: Python<'_>) -> PyObject {
        self.to_string()
            .into_pyobject(py)
            .unwrap()
            .into_any()
            .unbind()
    }
}

impl From<TemplateError> for PyErr {
    fn from(err: TemplateError) -> PyErr {
        PySyntaxError::new_err(err.to_string())
    }
}
