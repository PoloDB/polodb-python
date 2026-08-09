"""Python bindings for the embedded PoloDB document database."""

from ._rust import POLODB_CORE_VERSION, ObjectId, PoloDBError
from .core import Collection, Cursor, Document, PoloDB, PoloDBConfig, Transaction
from .results import DeleteResult, InsertManyResult, InsertOneResult, UpdateResult
from .version import __version__

__all__ = [
    "POLODB_CORE_VERSION",
    "Collection",
    "Cursor",
    "DeleteResult",
    "Document",
    "InsertManyResult",
    "InsertOneResult",
    "ObjectId",
    "PoloDB",
    "PoloDBConfig",
    "PoloDBError",
    "Transaction",
    "UpdateResult",
    "__version__",
]
