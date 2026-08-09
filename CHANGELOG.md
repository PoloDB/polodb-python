# Changelog

## 0.2.0

- Upgrade to PoloDB Core 5.3.0, PyO3 0.29.2, maturin 1.14, and Rust 2024.
- Build ABI3 wheels for CPython 3.9 and newer.
- Add transactions, index creation/removal, collection drops, query sorting/pagination, metrics, and logging controls.
- Add lazy, chainable query cursors with streaming iteration, `skip()`, `limit()`, `sort()`, and `to_list()`.
- Add safe round-trip conversion for ObjectId, datetime, regex, binary, null, nested documents, lists, and tuples.
- Add typed result objects, native extension stubs, `py.typed`, path-like paths, and standard Python container/context protocols.
- Replace panic-prone conversions and silent errors with Python exceptions.
- Split CI from hardened, tag-only release publishing.
- Raise the minimum supported Python version to 3.10 and update vulnerable locked dependencies.
- Modernize developer tooling with current mypy, Ruff, uv-based CI, and Astral ty checks.
