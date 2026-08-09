from importlib.metadata import PackageNotFoundError, version

try:
    __version__ = version("polodb-python")
except PackageNotFoundError:  # Source tree without an installed distribution.
    __version__ = "0.2.1"

__all__ = ["__version__"]
