from pathlib import Path

import pytest

from polodb import PoloDB


@pytest.fixture
def db_path(tmp_path: Path) -> Path:
    return tmp_path / "database"


@pytest.fixture
def db(db_path: Path):
    database = PoloDB(db_path)
    yield database
    database.close()
