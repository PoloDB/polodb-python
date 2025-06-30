from __future__ import annotations

try:
    from importlib.metadata import version, PackageNotFoundError
except ImportError:                       # Python < 3.8 fallback
    from importlib_metadata import version, PackageNotFoundError


def _detect_version() -> str:
    """
    Return the installed version.  Works in three situations:

    1. Normal wheel/sdist install → use .dist-info metadata
    2. Editable install (PEP 660)  → metadata still available
    3. Plain source tree (no build) → fall back to pyproject.toml
    """
    dist_name = "polodb-python"          # *distribution* name, not module

    # 1 & 2 — installed package: read from dist-info
    try:
        return version(dist_name)
    except PackageNotFoundError:
        pass

    # 3 — running from a working copy: read pyproject.toml
    import pathlib, sys

    # tomli is the back-port of tomllib for < 3.11
    if sys.version_info >= (3, 11):
        import tomllib
    else:
        import tomli as tomllib

    root = pathlib.Path(__file__).resolve().parents[1]
    with (root / "pyproject.toml").open("rb") as f:
        data = tomllib.load(f)

    return data["project"]["version"]


__version__: str = _detect_version()
del _detect_version, version, PackageNotFoundError
