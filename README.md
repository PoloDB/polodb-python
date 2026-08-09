# PoloDB for Python

Fast, typed Python bindings for [PoloDB](https://www.polodb.org), an embedded document database with a MongoDB-like API. The database runs in-process and stores its data locally—there is no server to install or manage.

Version 0.2 uses PoloDB Core 5.3, PyO3 0.29, and CPython's stable ABI. Published wheels support CPython 3.10 and newer on Linux, macOS, and Windows.

## Installation

```bash
uv add polodb-python
```

The distribution name is `polodb-python`; `uv add polodb` refers to a different, obsolete package.

## Quick start

```python
from polodb import PoloDB

with PoloDB("app.db") as db:
    books = db["books"]
    inserted = books.insert_one(
        {"title": "The Three-Body Problem", "author": "Liu Cixin", "year": 2008}
    )

    book = books.find_one({"_id": inserted.inserted_id})
    print(book)

    recent = (
        books.find(
            {"year": {"$gte": 2000}},
        )
        .sort({"year": -1})
        .limit(10)
    )
```

Collections can also be accessed as attributes (`db.books`), though item access is preferable when a name is dynamic or collides with a database attribute.

## Database configuration

`PoloDBConfig` exposes PoloDB Core's storage settings while preserving its defaults:

```python
from polodb import PoloDB, PoloDBConfig

config = PoloDBConfig(
    init_block_count=32,
    journal_full_size=2_000,
    lsm_page_size=8_192,
    lsm_block_size=8 * 1024 * 1024,
    sync_log_count=500,
)

with PoloDB("app.db", config=config) as db:
    print(db.config)
```

The same configuration is retained if a `PoloDB` context is closed and later reopened.

## Collection API

### Insert and query

```python
result = books.insert_many(
    [
        {"title": "1984", "author": "George Orwell", "year": 1949},
        {"title": "Animal Farm", "author": "George Orwell", "year": 1945},
    ]
)
print(result.inserted_ids)

book = books.find_one({"title": "1984"})
all_orwell = books.find({"author": "George Orwell"}, sort={"year": 1})
for book in books.find_iter({"year": {"$lt": 1950}}):
    print(book)
```

`find()` returns a lazy cursor, so documents are decoded as they are consumed rather than loaded into memory at once. Cursors support chainable `skip()`, `limit()`, and `sort()` methods; the equivalent keyword arguments on `find()` remain available. An omitted filter means an empty filter.

### Update and delete

```python
updated = books.update_one(
    {"title": "1984"},
    {"$set": {"in_print": True}},
)
print(updated.matched_count, updated.modified_count)

books.update_many(
    {"author": "Octavia E. Butler"},
    {"$set": {"featured": True}},
    upsert=False,
)

deleted = books.delete_many({"in_print": False})
print(deleted.deleted_count)
```

### Aggregation

```python
authors = books.aggregate(
    [
        {"$match": {"year": {"$gte": 2000}}},
        {"$sort": {"year": -1}},
        {"$limit": 10},
    ]
)
```

### Indexes

```python
index_name = books.create_index({"title": 1}, unique=True)
books.drop_index(index_name)
```

### Counts and drops

```python
print(len(books))
print(books.count_documents())
books.drop()

# Equivalent database-level operation:
db.drop_collection("books")
```

## Transactions

Transactions commit when their context exits normally and roll back when an exception escapes:

```python
with db.transaction() as transaction:
    accounts = transaction["accounts"]
    accounts.update_one({"name": "Ada"}, {"$inc": {"balance": -100}})
    accounts.update_one({"name": "Grace"}, {"$inc": {"balance": 100}})
```

Manual `commit()` and `rollback()` are also available.

## BSON values

The binding round-trips the common BSON-compatible Python values:

- `None`, `bool`, `int`, `float`, `str`
- nested dictionaries, lists, and tuples
- `bytes` and `bytearray`
- timezone-aware or naive `datetime.datetime` values (stored with millisecond precision and returned in UTC)
- compiled regular expressions
- `polodb.ObjectId`

Generated `_id` values are returned as `ObjectId` instances, so they can be passed directly into later filters:

```python
from polodb import ObjectId

identifier = ObjectId()  # new value
same_identifier = ObjectId(identifier.hex)
assert identifier == same_identifier
```

## Results and errors

Write operations return typed, immutable result objects:

- `InsertOneResult.inserted_id`
- `InsertManyResult.inserted_ids`
- `UpdateResult.matched_count` and `.modified_count`
- `DeleteResult.deleted_count`

They also implement `Mapping`, preserving dictionary-style reads such as `result["modified_count"]`. Database-operation failures raise `PoloDBError`; invalid Python values raise standard `TypeError` or `ValueError` exceptions.

## Migrating from 0.1

Most CRUD code continues to work. Notable improvements and changes in 0.2 are:

- generated IDs are `ObjectId` values instead of lossy strings; use `str(id)` or `id.hex` when text is required;
- write results are typed mapping objects rather than plain dictionaries;
- `find()` returns a lazy, chainable cursor instead of an optional list; call `.to_list()` when a list is needed;
- `len(collection)` and `count_documents()` are preferred; `collection.len()` remains as a compatibility alias;
- context managers now close databases and correctly commit or roll back transactions;
- unsupported BSON values raise an exception instead of silently becoming `None` or panicking the interpreter.

## Development

```bash
uv sync
uv run maturin develop
uv run pytest
uv run ruff check .
uv run mypy polodb
uv run ty check
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Releases are built from `vX.Y.Z` tags. The tag must match both `pyproject.toml` and `Cargo.toml`. PyPI publication uses the repository's `PYPI_TOKEN` secret.

## License

Apache-2.0. See [LICENSE.txt](LICENSE.txt).
