use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use polodb_core::bson::{Bson, Document};
use polodb_core::options::UpdateOptions;
use polodb_core::results::{DeleteResult, InsertManyResult, InsertOneResult, UpdateResult};
use polodb_core::{
    ClientCursor, Collection, CollectionT, Config, Database, IndexModel, IndexOptions, Transaction,
    TransactionalCollection,
};
use pyo3::exceptions::{PyOSError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};

use crate::PoloDBError;
use crate::helper_type_translator::{
    bson_to_py, document_to_py, py_iterable_to_documents, py_to_document,
};

fn database_error(error: polodb_core::Error) -> PyErr {
    PoloDBError::new_err(error.to_string())
}

fn empty_or_document(value: Option<&Bound<'_, PyAny>>) -> PyResult<Document> {
    value.map_or_else(|| Ok(Document::new()), py_to_document)
}

fn insert_one_result(py: Python<'_>, result: InsertOneResult) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("inserted_id", bson_to_py(py, &result.inserted_id)?)?;
    Ok(dict.unbind())
}

fn insert_many_result(py: Python<'_>, result: InsertManyResult) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    for (index, value) in result.inserted_ids {
        dict.set_item(index, bson_to_py(py, &value)?)?;
    }
    Ok(dict.unbind())
}

fn update_result(py: Python<'_>, result: UpdateResult) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("matched_count", result.matched_count)?;
    dict.set_item("modified_count", result.modified_count)?;
    Ok(dict.unbind())
}

fn delete_result(py: Python<'_>, result: DeleteResult) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("deleted_count", result.deleted_count)?;
    Ok(dict.unbind())
}

enum CollectionHandle {
    Database(Collection<Document>),
    Transaction(TransactionalCollection<Document>),
}

#[pyclass(name = "_Cursor")]
pub struct PyCursor {
    inner: Mutex<ClientCursor<Document>>,
}

#[pymethods]
impl PyCursor {
    fn __iter__(cursor: PyRef<'_, Self>) -> PyRef<'_, Self> {
        cursor
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<Py<PyDict>>> {
        let mut cursor = self
            .inner
            .lock()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        match cursor.advance().map_err(database_error)? {
            true => cursor
                .deserialize_current()
                .map_err(database_error)
                .and_then(|document| document_to_py(py, document))
                .map(Some),
            false => Ok(None),
        }
    }
}

#[pyclass(name = "_Collection")]
pub struct PyCollection {
    inner: CollectionHandle,
}

impl PyCollection {
    fn from_database(database: &Database, name: &str) -> Self {
        Self {
            inner: CollectionHandle::Database(database.collection(name)),
        }
    }

    fn from_transaction(transaction: &Transaction, name: &str) -> Self {
        Self {
            inner: CollectionHandle::Transaction(transaction.collection(name)),
        }
    }
}

macro_rules! collection_call {
    ($self:expr, $method:ident ( $($argument:expr),* $(,)? )) => {
        match &$self.inner {
            CollectionHandle::Database(collection) => collection.$method($($argument),*),
            CollectionHandle::Transaction(collection) => collection.$method($($argument),*),
        }
    };
}

#[pymethods]
impl PyCollection {
    #[getter]
    fn name(&self) -> &str {
        match &self.inner {
            CollectionHandle::Database(collection) => collection.name(),
            CollectionHandle::Transaction(collection) => collection.name(),
        }
    }

    fn insert_one(&self, py: Python<'_>, document: &Bound<'_, PyAny>) -> PyResult<Py<PyDict>> {
        let document = py_to_document(document)?;
        let result = collection_call!(self, insert_one(&document)).map_err(database_error)?;
        insert_one_result(py, result)
    }

    fn insert_many(&self, py: Python<'_>, documents: &Bound<'_, PyAny>) -> PyResult<Py<PyDict>> {
        let documents = py_iterable_to_documents(documents)?;
        let result = collection_call!(self, insert_many(documents)).map_err(database_error)?;
        insert_many_result(py, result)
    }

    #[pyo3(signature = (filter=None, *, skip=0, limit=0, sort=None))]
    fn find(
        &self,
        filter: Option<&Bound<'_, PyAny>>,
        skip: u64,
        limit: u64,
        sort: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyCursor> {
        let filter = empty_or_document(filter)?;
        let sort = sort.map(py_to_document).transpose()?;

        let cursor = match &self.inner {
            CollectionHandle::Database(collection) => {
                let mut find = collection.find(filter);
                if skip != 0 {
                    find = find.skip(skip);
                }
                if limit != 0 {
                    find = find.limit(limit);
                }
                if let Some(sort) = sort {
                    find = find.sort(sort);
                }
                find.run().map_err(database_error)?
            }
            CollectionHandle::Transaction(collection) => {
                let mut find = collection.find(filter);
                if skip != 0 {
                    find = find.skip(skip);
                }
                if limit != 0 {
                    find = find.limit(limit);
                }
                if let Some(sort) = sort {
                    find = find.sort(sort);
                }
                find.run().map_err(database_error)?
            }
        };
        Ok(PyCursor {
            inner: Mutex::new(cursor),
        })
    }

    #[pyo3(signature = (filter=None))]
    fn find_one(
        &self,
        py: Python<'_>,
        filter: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<Py<PyDict>>> {
        let filter = empty_or_document(filter)?;
        collection_call!(self, find_one(filter))
            .map_err(database_error)?
            .map(|document| document_to_py(py, document))
            .transpose()
    }

    #[pyo3(signature = (filter, update, *, upsert=false))]
    fn update_one(
        &self,
        py: Python<'_>,
        filter: &Bound<'_, PyAny>,
        update: &Bound<'_, PyAny>,
        upsert: bool,
    ) -> PyResult<Py<PyDict>> {
        let filter = py_to_document(filter)?;
        let update = py_to_document(update)?;
        let options = UpdateOptions::builder().upsert(upsert).build();
        let result = collection_call!(self, update_one_with_options(filter, update, options))
            .map_err(database_error)?;
        update_result(py, result)
    }

    #[pyo3(signature = (filter, update, *, upsert=false))]
    fn update_many(
        &self,
        py: Python<'_>,
        filter: &Bound<'_, PyAny>,
        update: &Bound<'_, PyAny>,
        upsert: bool,
    ) -> PyResult<Py<PyDict>> {
        let filter = py_to_document(filter)?;
        let update = py_to_document(update)?;
        let options = UpdateOptions::builder().upsert(upsert).build();
        let result = collection_call!(self, update_many_with_options(filter, update, options))
            .map_err(database_error)?;
        update_result(py, result)
    }

    #[pyo3(signature = (filter=None))]
    fn delete_one(
        &self,
        py: Python<'_>,
        filter: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyDict>> {
        let filter = empty_or_document(filter)?;
        let result = collection_call!(self, delete_one(filter)).map_err(database_error)?;
        delete_result(py, result)
    }

    #[pyo3(signature = (filter=None))]
    fn delete_many(
        &self,
        py: Python<'_>,
        filter: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyDict>> {
        let filter = empty_or_document(filter)?;
        let result = collection_call!(self, delete_many(filter)).map_err(database_error)?;
        delete_result(py, result)
    }

    fn count_documents(&self) -> PyResult<u64> {
        collection_call!(self, count_documents()).map_err(database_error)
    }

    fn aggregate(&self, py: Python<'_>, pipeline: &Bound<'_, PyAny>) -> PyResult<Vec<Py<PyDict>>> {
        let pipeline = py_iterable_to_documents(pipeline)?;
        let documents: Vec<Document> = match &self.inner {
            CollectionHandle::Database(collection) => collection
                .aggregate(pipeline)
                .run()
                .map_err(database_error)?
                .collect::<Result<_, _>>()
                .map_err(database_error)?,
            CollectionHandle::Transaction(collection) => collection
                .aggregate(pipeline)
                .run()
                .map_err(database_error)?
                .collect::<Result<_, _>>()
                .map_err(database_error)?,
        };
        documents
            .into_iter()
            .map(|document| document_to_py(py, document))
            .collect()
    }

    #[pyo3(signature = (keys, *, name=None, unique=false))]
    fn create_index(
        &self,
        keys: &Bound<'_, PyAny>,
        name: Option<String>,
        unique: bool,
    ) -> PyResult<String> {
        let keys = py_to_document(keys)?;
        let generated_name = name.clone().unwrap_or_else(|| {
            keys.iter()
                .map(|(key, value)| {
                    let direction = match value {
                        Bson::Int32(value) => value.to_string(),
                        Bson::Int64(value) => value.to_string(),
                        Bson::Double(value) => value.to_string(),
                        value => value.to_string(),
                    };
                    format!("{key}_{direction}")
                })
                .collect::<Vec<_>>()
                .join("_")
        });
        let model = IndexModel {
            keys,
            options: Some(IndexOptions {
                name: Some(generated_name.clone()),
                unique: Some(unique),
            }),
        };
        collection_call!(self, create_index(model)).map_err(database_error)?;
        Ok(generated_name)
    }

    fn drop_index(&self, name: &str) -> PyResult<()> {
        collection_call!(self, drop_index(name)).map_err(database_error)
    }

    fn drop(&self) -> PyResult<()> {
        collection_call!(self, drop()).map_err(database_error)
    }
}

#[pyclass(name = "_Transaction")]
pub struct PyTransaction {
    inner: Transaction,
    active: AtomicBool,
}

#[pymethods]
impl PyTransaction {
    fn collection(&self, name: &str) -> PyResult<PyCollection> {
        if !self.active.load(Ordering::Acquire) {
            return Err(PyRuntimeError::new_err("transaction is no longer active"));
        }
        Ok(PyCollection::from_transaction(&self.inner, name))
    }

    fn commit(&self) -> PyResult<()> {
        if !self.active.swap(false, Ordering::AcqRel) {
            return Err(PyRuntimeError::new_err("transaction is no longer active"));
        }
        self.inner.commit().map_err(database_error)
    }

    fn rollback(&self) -> PyResult<()> {
        if !self.active.swap(false, Ordering::AcqRel) {
            return Err(PyRuntimeError::new_err("transaction is no longer active"));
        }
        self.inner.rollback().map_err(database_error)
    }

    #[getter]
    fn active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}

#[pyclass(name = "_Database")]
pub struct PyDatabase {
    inner: Database,
}

#[pymethods]
impl PyDatabase {
    #[new]
    #[pyo3(signature = (
        path,
        *,
        init_block_count=16,
        journal_full_size=1000,
        lsm_page_size=4096,
        lsm_block_size=4 * 1024 * 1024,
        sync_log_count=1000,
    ))]
    fn new(
        path: &str,
        init_block_count: u64,
        journal_full_size: u64,
        lsm_page_size: u32,
        lsm_block_size: u32,
        sync_log_count: u64,
    ) -> PyResult<Self> {
        Self::open_path(
            path,
            init_block_count,
            journal_full_size,
            lsm_page_size,
            lsm_block_size,
            sync_log_count,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (
        path,
        *,
        init_block_count=16,
        journal_full_size=1000,
        lsm_page_size=4096,
        lsm_block_size=4 * 1024 * 1024,
        sync_log_count=1000,
    ))]
    fn open_path(
        path: &str,
        init_block_count: u64,
        journal_full_size: u64,
        lsm_page_size: u32,
        lsm_block_size: u32,
        sync_log_count: u64,
    ) -> PyResult<Self> {
        let config = Config {
            init_block_count,
            journal_full_size,
            lsm_page_size,
            lsm_block_size,
            sync_log_count,
        };
        Database::open_path_with_config(path, config)
            .map(|inner| Self { inner })
            .map_err(|error| PyOSError::new_err(error.to_string()))
    }

    fn create_collection(&self, name: &str) -> PyResult<()> {
        self.inner.create_collection(name).map_err(database_error)
    }

    fn collection(&self, name: &str) -> PyCollection {
        PyCollection::from_database(&self.inner, name)
    }

    fn drop_collection(&self, name: &str) -> PyResult<()> {
        self.inner
            .collection::<Document>(name)
            .drop()
            .map_err(database_error)
    }

    fn list_collection_names(&self) -> PyResult<Vec<String>> {
        self.inner.list_collection_names().map_err(database_error)
    }

    fn start_transaction(&self) -> PyResult<PyTransaction> {
        self.inner
            .start_transaction()
            .map(|inner| PyTransaction {
                inner,
                active: AtomicBool::new(true),
            })
            .map_err(database_error)
    }

    fn metrics(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let metrics = self.inner.metrics();
        let result = PyDict::new(py);
        result.set_item("find_by_index_count", metrics.find_by_index_count())?;
        Ok(result.unbind())
    }

    fn enable_metrics(&self) {
        self.inner.metrics().enable();
    }

    #[staticmethod]
    fn set_log(enabled: bool) {
        Database::set_log(enabled);
    }
}
