#!/usr/bin/env python3
"""Fetch and prepare the texts tolkien is fuzzed and scored against.

Nothing this script writes is committed: the datasets are large and carry
their own licences. Everything goes under corpus/ (ignored by git):

  corpus/raw/        the files as downloaded
  corpus/docs/       one file per corpus, documents separated by a NUL
                     character: the input of `cargo run --example fuzz`
  corpus/labelled/   one JSON object per line, {"text", "spans"}, where a
                     span is [start, end, label] in characters: the input
                     of tools/evaluate.py

Corpora:

  ai4privacy-en, -de   pii-masking-200k: synthetic sentences with 56 kinds
                       of labelled personal data (licence: see
                       corpus/raw/ai4privacy-license.md; free for
                       individuals, small companies and evaluation)
  tab                  Text Anonymization Benchmark: court cases with
                       hand-labelled entities (MIT)
  wikipedia            one article in each of 24 languages and scripts
                       (CC BY-SA), not pinned: the text is whatever the
                       article says today
  markit               the documents of markit's benchmark corpus converted
                       to markdown, when ~/Projects/markit is built
  code                 this repository's own sources

Usage:
  python3 tools/corpora.py            fetch what is missing, then prepare
  python3 tools/corpora.py --offline  prepare from corpus/raw/ only

Pure standard library.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RAW = ROOT / "corpus" / "raw"
DOCS = ROOT / "corpus" / "docs"
LABELLED = ROOT / "corpus" / "labelled"
MARKIT = Path.home() / "Projects" / "markit"
USER_AGENT = "tolkien-corpora/0.1 (lexer test corpus; python-urllib)"

# name -> (url, sha256)
PINNED = {
    "english_pii_43k.jsonl": (
        "https://huggingface.co/datasets/ai4privacy/pii-masking-200k/resolve/main/english_pii_43k.jsonl",
        "1a7aac12051b87390a18fe8ece2010cfa8c1631e0b69c46e6ed5a995b374a7f8",
    ),
    "german_pii_52k.jsonl": (
        "https://huggingface.co/datasets/ai4privacy/pii-masking-200k/resolve/main/german_pii_52k.jsonl",
        "25636c4caf531d466b7dfc2675bf81fb4d3c66419246338b2ed2b6cdbca32eb7",
    ),
    "ai4privacy-license.md": (
        "https://huggingface.co/datasets/ai4privacy/pii-masking-200k/resolve/main/license.md",
        "0af41e1a406e3ac5af79d07728e69fdf94d4b5d3d568fa40d6ad487e55135cd1",
    ),
    "echr_dev.json": (
        "https://raw.githubusercontent.com/NorskRegnesentral/text-anonymization-benchmark/master/echr_dev.json",
        "8c3c7306f46b8d54debeb38ae11d8b0b8bcf4bdccbc3b6f13c12ad7be16893ec",
    ),
}

# (language, article): one capital city per script or language family.
WIKIPEDIA = [
    ("am", "አዲስ አበባ"), ("ar", "القاهرة"), ("bn", "ঢাকা"), ("de", "Berlin"),
    ("el", "Αθήνα"), ("es", "Madrid"), ("fa", "تهران"), ("fr", "Paris"),
    ("he", "ירושלים"), ("hi", "दिल्ली"), ("hy", "Երևան"), ("ja", "東京都"),
    ("ka", "თბილისი"), ("km", "ភ្នំពេញ"), ("ko", "서울특별시"), ("my", "နေပြည်တော်"),
    ("pl", "Warszawa"), ("ru", "Москва"), ("ta", "சென்னை"), ("th", "กรุงเทพมหานคร"),
    ("tr", "İstanbul"), ("ur", "اسلام آباد"), ("vi", "Hà Nội"), ("zh", "北京市"),
]


def download(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    for attempt in range(6):
        try:
            with urllib.request.urlopen(request, timeout=300) as response:
                return response.read()
        except urllib.error.HTTPError as error:
            # Wikipedia answers 429 to requests that come too close together.
            if error.code != 429 or attempt == 5:
                raise
            time.sleep(5 * (attempt + 1))
    raise AssertionError("unreachable")


def fetch_pinned() -> None:
    for name, (url, sha256) in PINNED.items():
        path = RAW / name
        if not path.exists():
            print(f"fetching {name}")
            path.write_bytes(download(url))
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if sha256 is None:
            sys.exit(f"{name}: no hash pinned; its sha256 is {digest}")
        if digest != sha256:
            sys.exit(f"{name}: sha256 {digest} does not match the pinned {sha256}")


def fetch_wikipedia() -> None:
    for lang, title in WIKIPEDIA:
        path = RAW / f"wikipedia-{lang}.txt"
        if path.exists():
            continue
        print(f"fetching wikipedia {lang}")
        query = urllib.parse.urlencode(
            {
                "action": "query",
                "prop": "extracts",
                "explaintext": 1,
                "redirects": 1,
                "format": "json",
                "titles": title,
            }
        )
        pages = json.loads(download(f"https://{lang}.wikipedia.org/w/api.php?{query}"))
        text = "".join(page.get("extract", "") for page in pages["query"]["pages"].values())
        if len(text) < 1000:
            sys.exit(f"wikipedia {lang}: {title} came back with {len(text)} characters")
        path.write_text(text, "utf-8")
        time.sleep(1)


def write_docs(name: str, docs: list[str]) -> None:
    docs = [doc.replace("\0", "") for doc in docs if doc]
    (DOCS / f"{name}.txt").write_text("\0".join(docs), "utf-8")
    size = sum(len(doc.encode("utf-8")) for doc in docs)
    print(f"docs/{name}.txt: {len(docs)} documents, {size / 1e6:.1f} MB")


def write_labelled(name: str, records: list[dict]) -> None:
    with (LABELLED / f"{name}.jsonl").open("w", encoding="utf-8") as out:
        for record in records:
            out.write(json.dumps(record, ensure_ascii=False) + "\n")
    spans = sum(len(record["spans"]) for record in records)
    print(f"labelled/{name}.jsonl: {len(records)} documents, {spans} spans")


def prepare_ai4privacy(name: str, raw: str) -> None:
    records = []
    for line in (RAW / raw).open(encoding="utf-8"):
        row = json.loads(line)
        text = row["source_text"]
        spans = []
        for mask in row["privacy_mask"]:
            start, end = mask["start"], mask["end"]
            if text[start:end] != mask["value"]:
                sys.exit(f"{raw}: span {start}..{end} of row {row['id']} is not {mask['value']!r}")
            spans.append([start, end, mask["label"]])
        records.append({"text": text, "spans": spans})
    write_labelled(name, records)
    write_docs(name, [record["text"] for record in records])


def prepare_tab() -> None:
    records = []
    for case in json.loads((RAW / "echr_dev.json").read_text("utf-8")):
        text = case["text"]
        # Every annotator marks the same text; their mentions are merged.
        spans = set()
        for annotation in case["annotations"].values():
            for mention in annotation["entity_mentions"]:
                start, end = mention["start_offset"], mention["end_offset"]
                if text[start:end] != mention["span_text"]:
                    sys.exit(f"tab: span {start}..{end} of {case['doc_id']} does not match")
                spans.add((start, end, mention["entity_type"]))
        records.append({"text": text, "spans": sorted(map(list, spans))})
    write_labelled("tab", records)
    write_docs("tab", [record["text"] for record in records])


def prepare_wikipedia() -> None:
    docs = [path.read_text("utf-8") for path in sorted(RAW.glob("wikipedia-*.txt"))]
    if docs:
        write_docs("wikipedia", docs)


def prepare_markit() -> None:
    cli = MARKIT / "dist" / "main.js"
    sources = sorted((MARKIT / "benchmark" / "corpus").glob("*.*"))
    if not cli.exists() or not sources:
        print("markit: not built at ~/Projects/markit, skipped")
        return
    docs = []
    for source in sources:
        result = subprocess.run(
            ["node", str(cli), str(source), "-q", "--no-images"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        if result.returncode == 0 and result.stdout.strip():
            docs.append(result.stdout)
        else:
            print(f"markit: could not convert {source.name}")
    write_docs("markit", docs)


def prepare_code() -> None:
    paths = sorted(
        path
        for pattern in ("src/**/*.rs", "tests/*.rs", "tools/*.py", "*.md", "docs/*.md")
        for path in ROOT.glob(pattern)
    )
    write_docs("code", [path.read_text("utf-8") for path in paths])


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--offline", action="store_true", help="download nothing")
    args = parser.parse_args()
    for directory in (RAW, DOCS, LABELLED):
        directory.mkdir(parents=True, exist_ok=True)
    if not args.offline:
        fetch_pinned()
        fetch_wikipedia()
    prepare_ai4privacy("ai4privacy-en", "english_pii_43k.jsonl")
    prepare_ai4privacy("ai4privacy-de", "german_pii_52k.jsonl")
    prepare_tab()
    prepare_wikipedia()
    prepare_markit()
    prepare_code()


if __name__ == "__main__":
    main()
