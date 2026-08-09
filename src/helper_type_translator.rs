use polodb_core::bson::{Binary, Bson, DateTime, Document, Regex, spec::BinarySubtype};
use pyo3::exceptions::{PyOverflowError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyByteArray, PyBytes, PyDict, PyList, PyTuple};

use crate::PyObjectId;

pub fn py_to_document(value: &Bound<'_, PyAny>) -> PyResult<Document> {
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("expected a mapping with string keys"))?;
    let mut document = Document::new();
    for (key, value) in dict.iter() {
        document.insert(key.extract::<String>()?, py_to_bson(&value)?);
    }
    Ok(document)
}

pub fn py_iterable_to_documents(value: &Bound<'_, PyAny>) -> PyResult<Vec<Document>> {
    value
        .try_iter()?
        .map(|item| py_to_document(&item?))
        .collect()
}

pub fn py_to_bson(value: &Bound<'_, PyAny>) -> PyResult<Bson> {
    if value.is_none() {
        return Ok(Bson::Null);
    }
    if value.is_instance_of::<PyBool>() {
        return Ok(Bson::Boolean(value.extract()?));
    }
    if let Ok(object_id) = value.extract::<PyRef<'_, PyObjectId>>() {
        return Ok(Bson::ObjectId(object_id.inner));
    }
    if let Ok(integer) = value.extract::<i64>() {
        return Ok(Bson::Int64(integer));
    }
    if value.is_instance(&value.py().import("decimal")?.getattr("Decimal")?)? {
        return Err(PyTypeError::new_err(
            "decimal.Decimal is not supported because PoloDB does not preserve Decimal128 values",
        ));
    }
    if let Ok(float) = value.extract::<f64>() {
        return Ok(Bson::Double(float));
    }
    if let Ok(string) = value.extract::<String>() {
        return Ok(Bson::String(string));
    }
    if let Ok(bytes) = value.cast::<PyBytes>() {
        return Ok(Bson::Binary(Binary {
            subtype: BinarySubtype::Generic,
            bytes: bytes.as_bytes().to_vec(),
        }));
    }
    if let Ok(bytes) = value.cast::<PyByteArray>() {
        // SAFETY: the bytearray is only read while the GIL is held.
        let bytes = unsafe { bytes.as_bytes() }.to_vec();
        return Ok(Bson::Binary(Binary {
            subtype: BinarySubtype::Generic,
            bytes,
        }));
    }
    if value.is_instance(&value.py().import("datetime")?.getattr("datetime")?)? {
        let seconds: f64 = value.call_method0("timestamp")?.extract()?;
        let millis = seconds * 1000.0;
        if !millis.is_finite() || millis < i64::MIN as f64 || millis > i64::MAX as f64 {
            return Err(PyOverflowError::new_err("datetime is outside BSON's range"));
        }
        return Ok(Bson::DateTime(DateTime::from_millis(millis.round() as i64)));
    }
    if let Ok(dict) = value.cast::<PyDict>() {
        return py_to_document(dict.as_any()).map(Bson::Document);
    }
    if let Ok(list) = value.cast::<PyList>() {
        return list
            .iter()
            .map(|item| py_to_bson(&item))
            .collect::<PyResult<Vec<_>>>()
            .map(Bson::Array);
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        return tuple
            .iter()
            .map(|item| py_to_bson(&item))
            .collect::<PyResult<Vec<_>>>()
            .map(Bson::Array);
    }
    if value.hasattr("pattern")? && value.hasattr("flags")? {
        let pattern: String = value.getattr("pattern")?.extract()?;
        let flags: u32 = value.getattr("flags")?.extract()?;
        let re = value.py().import("re")?;
        let mut options = String::new();
        for (name, option) in [
            ("IGNORECASE", 'i'),
            ("MULTILINE", 'm'),
            ("DOTALL", 's'),
            ("VERBOSE", 'x'),
        ] {
            let flag: u32 = re.getattr(name)?.extract()?;
            if flags & flag != 0 {
                options.push(option);
            }
        }
        return Ok(Bson::RegularExpression(Regex { pattern, options }));
    }

    Err(PyTypeError::new_err(format!(
        "unsupported value of type '{}' for BSON conversion",
        value.get_type().name()?
    )))
}

pub fn document_to_py(py: Python<'_>, document: Document) -> PyResult<Py<PyDict>> {
    let result = PyDict::new(py);
    for (key, value) in document {
        result.set_item(key, bson_to_py(py, &value)?)?;
    }
    Ok(result.unbind())
}

pub fn bson_to_py(py: Python<'_>, value: &Bson) -> PyResult<Py<PyAny>> {
    match value {
        Bson::Double(value) => Ok(value.into_pyobject(py)?.into_any().unbind()),
        Bson::String(value) => Ok(value.into_pyobject(py)?.into_any().unbind()),
        Bson::Array(values) => {
            let items = values
                .iter()
                .map(|value| bson_to_py(py, value))
                .collect::<PyResult<Vec<_>>>()?;
            Ok(PyList::new(py, items)?.into_any().unbind())
        }
        Bson::Document(value) => Ok(document_to_py(py, value.clone())?.into_any()),
        Bson::Boolean(value) => Ok(value.into_pyobject(py)?.to_owned().into_any().unbind()),
        Bson::Null | Bson::Undefined => Ok(py.None()),
        Bson::Int32(value) => Ok(value.into_pyobject(py)?.into_any().unbind()),
        Bson::Int64(value) => Ok(value.into_pyobject(py)?.into_any().unbind()),
        Bson::ObjectId(value) => Ok(Py::new(py, PyObjectId { inner: *value })?.into_any()),
        Bson::DateTime(value) => {
            let datetime = py.import("datetime")?;
            let timezone = datetime.getattr("timezone")?.getattr("utc")?;
            let seconds = value.timestamp_millis() as f64 / 1000.0;
            Ok(datetime
                .getattr("datetime")?
                .call_method1("fromtimestamp", (seconds, timezone))?
                .unbind())
        }
        Bson::RegularExpression(value) => {
            let re = py.import("re")?;
            let mut flags = 0_u32;
            for option in value.options.chars() {
                let name = match option {
                    'i' => Some("IGNORECASE"),
                    'm' => Some("MULTILINE"),
                    's' => Some("DOTALL"),
                    'x' => Some("VERBOSE"),
                    _ => None,
                };
                if let Some(name) = name {
                    flags |= re.getattr(name)?.extract::<u32>()?;
                }
            }
            Ok(re
                .call_method1("compile", (&value.pattern, flags))?
                .unbind())
        }
        Bson::Binary(value) => Ok(PyBytes::new(py, &value.bytes).into_any().unbind()),
        Bson::Timestamp(value) => Ok((value.time, value.increment)
            .into_pyobject(py)?
            .into_any()
            .unbind()),
        Bson::Decimal128(value) => Ok(py
            .import("decimal")?
            .getattr("Decimal")?
            .call1((value.to_string(),))?
            .unbind()),
        Bson::JavaScriptCode(value) | Bson::Symbol(value) => {
            Ok(value.into_pyobject(py)?.into_any().unbind())
        }
        Bson::JavaScriptCodeWithScope(value) => {
            let result = PyDict::new(py);
            result.set_item("code", &value.code)?;
            result.set_item("scope", document_to_py(py, value.scope.clone())?)?;
            Ok(result.into_any().unbind())
        }
        Bson::MinKey | Bson::MaxKey | Bson::DbPointer(_) => Ok(py.None()),
    }
}
