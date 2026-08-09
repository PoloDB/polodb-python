import re
from datetime import datetime, timezone
from decimal import Decimal
from pathlib import Path

import pytest

from polodb import (
    POLODB_CORE_VERSION,
    Collection,
    Cursor,
    DeleteResult,
    InsertManyResult,
    InsertOneResult,
    ObjectId,
    PoloDB,
    PoloDBConfig,
    PoloDBError,
    UpdateResult,
)


def test_custom_database_config(db_path: Path) -> None:
    config = PoloDBConfig(
        init_block_count=32,
        journal_full_size=2048,
        lsm_page_size=8192,
        lsm_block_size=8 * 1024 * 1024,
        sync_log_count=500,
    )

    configured = PoloDB(db_path, config=config)
    with configured as configured_db:
        assert configured_db.config == config
        configured_db["configured"].insert_one({"works": True})

    with configured as reopened_db:
        assert reopened_db.config == config
        assert reopened_db["configured"].find_one({"works": True}) is not None


def test_database_protocols(db: PoloDB, db_path: Path) -> None:
    assert db.path == str(db_path)
    assert repr(db) == f"PoloDB({str(db_path)!r})"
    assert POLODB_CORE_VERSION == "5.3.0"

    collection = db["books"]
    assert isinstance(collection, Collection)
    assert collection.name() == "books"
    assert "books" in db
    assert list(db) == ["books"]
    assert db.books.name() == "books"


def test_insert_find_and_bson_round_trip(db: PoloDB) -> None:
    collection = db["values"]
    created_at = datetime(2026, 8, 10, 12, 34, 56, 789000, tzinfo=timezone.utc)
    result = collection.insert_one(
        {
            "none": None,
            "boolean": True,
            "integer": 42,
            "float": 3.5,
            "bytes": b"polo",
            "bytearray": bytearray(b"db"),
            "array": [1, "two", False],
            "tuple": (1, 2),
            "nested": {"answer": 42},
            "created_at": created_at,
            "pattern": re.compile("^polo", re.IGNORECASE),
        }
    )

    assert isinstance(result, InsertOneResult)
    assert isinstance(result.inserted_id, ObjectId)
    assert result["inserted_id"] == result.inserted_id
    assert ObjectId(result.inserted_id.hex) == result.inserted_id
    assert result.inserted_id != result.inserted_id.hex

    found = collection.find_one({"_id": result.inserted_id})
    assert found is not None
    assert found["none"] is None
    assert found["boolean"] is True
    assert found["integer"] == 42
    assert found["bytes"] == b"polo"
    assert found["bytearray"] == b"db"
    assert found["tuple"] == [1, 2]
    assert found["nested"] == {"answer": 42}
    assert found["created_at"] == created_at
    assert found["pattern"].pattern == "^polo"
    assert found["pattern"].flags & re.IGNORECASE

    with pytest.raises(TypeError, match="Decimal"):
        collection.insert_one({"unsupported": Decimal("12.34")})


def test_insert_many_find_sort_skip_and_limit(db: PoloDB) -> None:
    collection = db["scores"]
    result = collection.insert_many({"score": score} for score in [3, 1, 2, 4])

    assert isinstance(result, InsertManyResult)
    assert len(result.inserted_ids) == 4
    assert all(isinstance(value, ObjectId) for value in result.values())
    assert len(collection) == 4
    assert collection.len() == 4

    documents = collection.find({}).sort({"score": 1}).skip(1).limit(2)
    assert isinstance(documents, Cursor)
    assert [document["score"] for document in documents] == [2, 3]
    assert [document["score"] for document in collection.find_iter({"score": {"$gt": 2}})] == [3, 4]

    with pytest.raises(ValueError, match="non-negative"):
        collection.find(skip=-1)

    started = collection.find({})
    next(started)
    with pytest.raises(RuntimeError, match="after iteration"):
        started.limit(1)


def test_update_delete_and_aggregate(db: PoloDB) -> None:
    collection = db["people"]
    collection.insert_many(
        [
            {"name": "Ada", "team": "compiler", "level": 1},
            {"name": "Grace", "team": "compiler", "level": 2},
            {"name": "Linus", "team": "kernel", "level": 2},
        ]
    )

    updated = collection.update_many(
        {"team": "compiler"},
        {"$inc": {"level": 1}},
    )
    assert updated == UpdateResult(matched_count=2, modified_count=2)
    assert updated["matched_count"] == 2

    upserted = collection.update_one(
        {"name": "Margaret"},
        {"$set": {"team": "space", "level": 3}},
        upsert=True,
    )
    assert isinstance(upserted, UpdateResult)
    assert collection.find_one({"name": "Margaret"}) is not None

    aggregate = collection.aggregate(
        [
            {"$match": {"level": {"$gte": 3}}},
            {"$sort": {"name": 1}},
        ]
    )
    assert [document["name"] for document in aggregate] == ["Grace", "Margaret"]

    deleted = collection.delete_one({"name": "Linus"})
    assert deleted == DeleteResult(deleted_count=1)
    assert deleted["deleted_count"] == 1
    assert collection.delete_many({"team": "compiler"}).deleted_count == 2


def test_indexes_and_drop(db: PoloDB) -> None:
    collection = db["users"]
    assert collection.create_index({"email": 1}, unique=True) == "email_1"
    collection.insert_one({"email": "user@example.com"})

    with pytest.raises(PoloDBError):
        collection.insert_one({"email": "user@example.com"})

    collection.drop_index("email_1")
    collection.insert_one({"email": "user@example.com"})
    collection.drop()
    assert "users" not in db


def test_transaction_commit_and_rollback(db: PoloDB) -> None:
    collection = db["ledger"]

    with db.transaction() as transaction:
        transaction["ledger"].insert_one({"amount": 10})
    assert collection.count_documents() == 1

    with pytest.raises(RuntimeError, match="abort"), db.transaction() as transaction:
        transaction["ledger"].insert_one({"amount": 20})
        raise RuntimeError("abort")
    assert collection.count_documents() == 1


def test_drop_collection_metrics_and_close(db: PoloDB) -> None:
    db["temporary"]
    db.drop_collection("temporary")
    assert "temporary" not in db.list_collection_names()

    db.enable_metrics()
    assert db.metrics()["find_by_index_count"] == 0

    db.close()
    with pytest.raises(RuntimeError, match="closed"):
        db.list_collection_names()
