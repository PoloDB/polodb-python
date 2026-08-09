from __future__ import annotations

import os
from collections.abc import Iterable, Iterator, Mapping
from typing import Any, Union

from ._rust import _Collection, _Cursor, _Database, _Transaction
from .results import DeleteResult, InsertManyResult, InsertOneResult, UpdateResult

Document = dict[str, Any]
Filter = Mapping[str, Any]


class Cursor(Iterator[Document]):
    """A lazy, chainable cursor over query results."""

    def __init__(self, native: _Collection, filter: Filter | None = None) -> None:
        self._native = native
        self._filter = filter
        self._skip = 0
        self._limit = 0
        self._sort: Filter | None = None
        self._cursor: _Cursor | None = None

    def __iter__(self) -> Cursor:
        return self

    def __next__(self) -> Document:
        if self._cursor is None:
            self._cursor = self._native.find(
                self._filter,
                skip=self._skip,
                limit=self._limit,
                sort=self._sort,
            )
        return next(self._cursor)

    def _ensure_not_started(self) -> None:
        if self._cursor is not None:
            raise RuntimeError("cursor options cannot be changed after iteration has started")

    def skip(self, value: int) -> Cursor:
        self._ensure_not_started()
        if value < 0:
            raise ValueError("skip must be non-negative")
        self._skip = value
        return self

    def limit(self, value: int) -> Cursor:
        self._ensure_not_started()
        if value < 0:
            raise ValueError("limit must be non-negative")
        self._limit = value
        return self

    def sort(self, value: Filter) -> Cursor:
        self._ensure_not_started()
        self._sort = value
        return self

    def to_list(self) -> list[Document]:
        return list(self)


class Collection:
    """A MongoDB-like collection backed by PoloDB."""

    def __init__(self, native: _Collection) -> None:
        self._native = native

    def __repr__(self) -> str:
        return f"Collection({self.name()!r})"

    def name(self) -> str:
        """Return the collection name (kept as a method for 0.1 compatibility)."""
        return str(self._native.name)

    def insert_one(self, document: Filter) -> InsertOneResult:
        result = self._native.insert_one(document)
        return InsertOneResult(result["inserted_id"])

    def insert_many(self, documents: Iterable[Filter]) -> InsertManyResult:
        result = self._native.insert_many(documents)
        return InsertManyResult(dict(result))

    def find_one(self, filter: Filter | None = None) -> Document | None:
        return self._native.find_one(filter)

    def find(
        self,
        filter: Filter | None = None,
        *,
        skip: int = 0,
        limit: int = 0,
        sort: Filter | None = None,
    ) -> Cursor:
        """Return a lazy cursor, optionally sorted, skipped, and limited."""
        cursor = Cursor(self._native, filter)
        if sort is not None:
            cursor.sort(sort)
        return cursor.skip(skip).limit(limit)

    def find_iter(
        self,
        filter: Filter | None = None,
        *,
        skip: int = 0,
        limit: int = 0,
        sort: Filter | None = None,
    ) -> Cursor:
        """Compatibility alias for :meth:`find`."""
        return self.find(filter, skip=skip, limit=limit, sort=sort)

    def update_one(
        self,
        filter: Filter,
        update: Filter,
        *,
        upsert: bool = False,
    ) -> UpdateResult:
        result = self._native.update_one(filter, update, upsert=upsert)
        return UpdateResult(result["matched_count"], result["modified_count"])

    def update_many(
        self,
        filter: Filter,
        update: Filter,
        *,
        upsert: bool = False,
    ) -> UpdateResult:
        result = self._native.update_many(filter, update, upsert=upsert)
        return UpdateResult(result["matched_count"], result["modified_count"])

    def delete_one(self, filter: Filter | None = None) -> DeleteResult:
        result = self._native.delete_one(filter)
        return DeleteResult(result["deleted_count"])

    def delete_many(self, filter: Filter | None = None) -> DeleteResult:
        result = self._native.delete_many(filter)
        return DeleteResult(result["deleted_count"])

    def count_documents(self) -> int:
        return int(self._native.count_documents())

    def __len__(self) -> int:
        return self.count_documents()

    def len(self) -> int:
        """Compatibility alias for ``len(collection)``."""
        return len(self)

    def aggregate(self, pipeline: Iterable[Filter]) -> list[Document]:
        return list(self._native.aggregate(pipeline))

    def create_index(
        self,
        keys: Filter,
        *,
        name: str | None = None,
        unique: bool = False,
    ) -> str:
        return str(self._native.create_index(keys, name=name, unique=unique))

    def drop_index(self, name: str) -> None:
        self._native.drop_index(name)

    def drop(self) -> None:
        self._native.drop()


class Transaction:
    """An explicit PoloDB transaction."""

    def __init__(self, native: _Transaction) -> None:
        self._native = native

    @property
    def active(self) -> bool:
        return bool(self._native.active)

    def collection(self, name: str) -> Collection:
        return Collection(self._native.collection(name))

    def __getitem__(self, name: str) -> Collection:
        return self.collection(name)

    def commit(self) -> None:
        self._native.commit()

    def rollback(self) -> None:
        self._native.rollback()

    def __enter__(self) -> Transaction:
        return self

    def __exit__(self, exc_type: object, exc_value: object, traceback: object) -> None:
        if exc_type is None:
            self.commit()
        else:
            self.rollback()


class PoloDB:
    """An embedded PoloDB database.

    ``path`` accepts strings and any object implementing ``os.PathLike``.
    """

    def __init__(self, path: Union[str, os.PathLike[str]]) -> None:  # noqa: UP007
        self._path: str = os.fspath(path)
        self._native: _Database | None = _Database(self._path)

    def __repr__(self) -> str:
        return f"PoloDB({self._path!r})"

    @property
    def path(self) -> str:
        return self._path

    def _db(self) -> _Database:
        if self._native is None:
            raise RuntimeError("database is closed")
        return self._native

    def __enter__(self) -> PoloDB:
        if self._native is None:
            self._native = _Database(self._path)
        return self

    def __exit__(self, exc_type: object, exc_value: object, traceback: object) -> None:
        self.close()

    def close(self) -> None:
        self._native = None

    def collection(self, name: str) -> Collection:
        database = self._db()
        if name not in database.list_collection_names():
            database.create_collection(name)
        return Collection(database.collection(name))

    def __getitem__(self, name: str) -> Collection:
        return self.collection(name)

    def __getattr__(self, name: str) -> Collection:
        if name.startswith("_"):
            raise AttributeError(name)
        return self.collection(name)

    def __iter__(self) -> Iterator[str]:
        return iter(self.list_collection_names())

    def __contains__(self, name: object) -> bool:
        return isinstance(name, str) and name in self.list_collection_names()

    def list_collection_names(self) -> list[str]:
        return list(self._db().list_collection_names())

    def drop_collection(self, name: str) -> None:
        self._db().drop_collection(name)

    def transaction(self) -> Transaction:
        return Transaction(self._db().start_transaction())

    def enable_metrics(self) -> None:
        self._db().enable_metrics()

    def metrics(self) -> dict[str, int]:
        return dict(self._db().metrics())

    @staticmethod
    def set_log(enabled: bool) -> None:
        _Database.set_log(enabled)
