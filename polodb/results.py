from __future__ import annotations

from collections.abc import Iterator, Mapping
from dataclasses import dataclass
from types import MappingProxyType
from typing import Any


@dataclass(frozen=True)
class InsertOneResult(Mapping[str, Any]):
    inserted_id: Any

    def __iter__(self) -> Iterator[str]:
        yield "inserted_id"

    def __len__(self) -> int:
        return 1

    def __getitem__(self, key: str) -> Any:
        if key == "inserted_id":
            return self.inserted_id
        raise KeyError(key)


@dataclass(frozen=True)
class InsertManyResult(Mapping[int, Any]):
    inserted_ids: Mapping[int, Any]

    def __post_init__(self) -> None:
        object.__setattr__(self, "inserted_ids", MappingProxyType(dict(self.inserted_ids)))

    def __iter__(self) -> Iterator[int]:
        return iter(self.inserted_ids)

    def __len__(self) -> int:
        return len(self.inserted_ids)

    def __getitem__(self, key: int) -> Any:
        return self.inserted_ids[key]


@dataclass(frozen=True)
class UpdateResult(Mapping[str, int]):
    matched_count: int
    modified_count: int

    def __iter__(self) -> Iterator[str]:
        yield "matched_count"
        yield "modified_count"

    def __len__(self) -> int:
        return 2

    def __getitem__(self, key: str) -> int:
        if key == "matched_count":
            return self.matched_count
        if key == "modified_count":
            return self.modified_count
        raise KeyError(key)


@dataclass(frozen=True)
class DeleteResult(Mapping[str, int]):
    deleted_count: int

    def __iter__(self) -> Iterator[str]:
        yield "deleted_count"

    def __len__(self) -> int:
        return 1

    def __getitem__(self, key: str) -> int:
        if key == "deleted_count":
            return self.deleted_count
        raise KeyError(key)
