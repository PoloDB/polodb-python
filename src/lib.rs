use std::hash::{Hash, Hasher};

use polodb_core::bson::oid::ObjectId;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;

mod helper_type_translator;
mod py_database;

use py_database::{PyCollection, PyCursor, PyDatabase, PyTransaction};

create_exception!(_rust, PoloDBError, PyException);

#[pyclass(name = "ObjectId", module = "polodb", skip_from_py_object)]
#[derive(Clone, Copy)]
pub struct PyObjectId {
    pub(crate) inner: ObjectId,
}

#[pymethods]
impl PyObjectId {
    #[new]
    #[pyo3(signature = (value=None))]
    fn new(value: Option<&str>) -> PyResult<Self> {
        let inner = match value {
            Some(value) => ObjectId::parse_str(value)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
            None => ObjectId::new(),
        };
        Ok(Self { inner })
    }

    #[getter]
    fn hex(&self) -> String {
        self.inner.to_hex()
    }

    fn __str__(&self) -> String {
        self.inner.to_hex()
    }

    fn __repr__(&self) -> String {
        format!("ObjectId('{}')", self.inner.to_hex())
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| self.inner == other.inner)
    }

    fn __hash__(&self) -> isize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.inner.bytes().hash(&mut hasher);
        hasher.finish() as isize
    }
}

#[pymodule]
fn _rust(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyDatabase>()?;
    module.add_class::<PyCollection>()?;
    module.add_class::<PyCursor>()?;
    module.add_class::<PyTransaction>()?;
    module.add_class::<PyObjectId>()?;
    module.add("PoloDBError", module.py().get_type::<PoloDBError>())?;
    module.add("POLODB_CORE_VERSION", polodb_core::Database::get_version())?;
    Ok(())
}
