"""Downloads for the scripts in this directory. Pure standard library."""

from __future__ import annotations

import hashlib
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

USER_AGENT = "tolkien-tools/0.1 (python-urllib)"


def download(url: str) -> bytes:
    """The body at `url`. Waits and tries again when the server says 429."""
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    for attempt in range(6):
        try:
            with urllib.request.urlopen(request, timeout=300) as response:
                return response.read()
        except urllib.error.HTTPError as error:
            if error.code != 429 or attempt == 5:
                raise
            time.sleep(5 * (attempt + 1))
    raise AssertionError("unreachable")


def pinned(url: str, path: Path, sha256: str) -> bytes:
    """The file at `path`, downloaded from `url` when it is missing. Exits
    when its SHA-256 is not the pinned one."""
    if not path.exists():
        print(f"fetching {path.name}")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(download(url))
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != sha256:
        sys.exit(f"{path.name}: sha256 {digest} does not match the pinned {sha256}")
    return data
