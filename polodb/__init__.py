"""Python bindings for the embedded PoloDB document database."""

from ._rust import POLODB_CORE_VERSION, ObjectId, PoloDBError
from .core import Collection, Document, PoloDB, Transaction
from .results import DeleteResult, InsertManyResult, InsertOneResult, UpdateResult
from .version import __version__

__all__ = [
    "POLODB_CORE_VERSION",
    "Collection",
    "DeleteResult",
    "Document",
    "InsertManyResult",
    "InsertOneResult",
    "ObjectId",
    "PoloDB",
    "PoloDBError",
    "Transaction",
    "UpdateResult",
    "__version__",
]
