#!/usr/bin/env python3
"""Compare tolkien's tokens with the labelled spans of the prepared corpora.

For every labelled span (an email address, a date, a phone number...) this
asks how the lexer cut the text there:

  exact    one token covers exactly the span
  inside   one token covers the span and more
  split    several tokens cover exactly the span
  ragged   a token crosses the start or the end of the span

and, for the labels a matcher exists for, whether an exact token has the
expected kind. The labels no matcher exists for are reported too: how they
are cut is what a detector built on these tokens has to work with. With
--catalog the lexer has every matcher of the catalog as well, and the
labels those cover (phone numbers, card numbers, addresses) are scored
against their kinds.

Usage:
  python3 tools/evaluate.py                    every corpus, a summary table
  python3 tools/evaluate.py --catalog          the same, with the catalog
  python3 tools/evaluate.py -l DATE -n 30      thirty misses of one label
  python3 tools/evaluate.py --stray DATE       tokens of a kind outside every
                                               span that expects that kind

Run tools/corpora.py first, and build with `cargo build --release` in rust/.
Pure standard library.
"""

from __future__ import annotations

import argparse
import bisect
import collections
import itertools
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LABELLED = ROOT / "corpus" / "labelled"
BINARY = ROOT / "rust" / "target" / "release" / "tolkien"
# Between documents. No matcher reads across a NUL character.
SEPARATOR = "\n\0\n"

# The kinds an exact token should have, for the labels a matcher covers.
EXPECTED = {
    "EMAIL": {"LIKE_EMAIL"},
    "URL": {"LIKE_URL"},
    "DATE": {"DATE", "DATE_ISO_8601"},
    "DOB": {"DATE", "DATE_ISO_8601"},
    "TIME": {"TIME"},
    "AMOUNT": {"LIKE_NUMBER", "CURRENCY"},
    "DATETIME": {"DATE", "DATE_ISO_8601", "TIME", "LIKE_NUMBER"},
}

# The same for the labels a matcher of the catalog covers: with --catalog.
EXPECTED_WITH_CATALOG = {
    "IPV4": {"IPV4"},
    "IPV6": {"IPV6"},
    "IP": {"IPV4", "IPV6"},
    "MAC": {"MAC_ADDRESS"},
    "CREDITCARDNUMBER": {"CARD_NUMBER"},
    "IBAN": {"IBAN"},
    "BITCOINADDRESS": {"BITCOIN_ADDRESS"},
    "ETHEREUMADDRESS": {"ETHEREUM_ADDRESS"},
    "PHONENUMBER": {"PHONE_NUMBER"},
    "PHONEIMEI": {"IMEI"},
    "SSN": {"US_SSN"},
    "NEARBYGPSCOORDINATE": {"COORDINATES"},
}

# Options for the tolkien command.
OPTIONS: list[str] = []


def lex(text: str) -> list[tuple[str, int, int]]:
    """The (kind, start, end) of every token of `text`, in bytes."""
    result = subprocess.run([str(BINARY), *OPTIONS], input=text.encode("utf-8"), capture_output=True, check=True)
    tokens = []
    for line in result.stdout.decode("utf-8").splitlines():
        kind, start, end, _ = line.split("\t", 3)
        tokens.append((kind, int(start), int(end)))
    return tokens


def load(path: Path) -> list[dict]:
    """The documents of a corpus. Each gets its text as bytes (`data`), its
    tokens as (kind, start, end) in bytes (`tokens`), and its spans as
    (start, end, label) in bytes (`spans`)."""
    records = [json.loads(line) for line in path.open(encoding="utf-8")]
    tokens = lex(SEPARATOR.join(record["text"] for record in records))
    base = 0  # byte offset of the current document in the joined text
    index = 0
    for record in records:
        text = record["text"]
        record["data"] = text.encode("utf-8")
        size = len(record["data"])
        # Byte offset of every character offset of this document.
        offsets = [0, *itertools.accumulate(len(ch.encode("utf-8")) for ch in text)]
        record["spans"] = sorted((offsets[start], offsets[end], label) for start, end, label in record["spans"])
        record["tokens"] = []
        while index < len(tokens) and tokens[index][1] < base + size:
            kind, start, end = tokens[index]
            record["tokens"].append((kind, start - base, end - base))
            index += 1
        base += size + len(SEPARATOR)
        # The tokens of the separator belong to no document.
        while index < len(tokens) and tokens[index][1] < base:
            index += 1
    return records


def context(data: bytes, start: int, end: int) -> str:
    """The text of `data[start:end]` with thirty bytes around it, on one line."""
    return data[max(0, start - 30) : end + 30].decode("utf-8", "replace").replace("\n", " ")


def evaluate(records: list[dict]):
    """Yields (label, outcome, kinds, span text, the span as it was cut,
    context) for every span."""
    for record in records:
        data, tokens = record["data"], record["tokens"]
        starts = [start for _, start, _ in tokens]
        for first, last, label in record["spans"]:
            touching = tokens[bisect.bisect_right(starts, first) - 1 : bisect.bisect_left(starts, last)]
            aligned = touching[0][1] == first and touching[-1][2] == last
            if len(touching) == 1:
                outcome = "exact" if aligned else "inside"
            else:
                outcome = "split" if aligned else "ragged"
            kinds = tuple(kind for kind, _, _ in touching)
            cut = " ".join(data[a:b].decode("utf-8") for _, a, b in touching)
            yield label, outcome, kinds, data[first:last].decode("utf-8"), cut, context(data, first, last)


def stray(records: list[dict], kind: str):
    """Yields the text and context of every token of `kind` that no span
    expecting that kind touches."""
    labels = {label for label, kinds in EXPECTED.items() if kind in kinds}
    for record in records:
        data = record["data"]
        covered = [(a, b) for a, b, label in record["spans"] if label in labels]
        for name, start, end in record["tokens"]:
            if name == kind and not any(start < b and a < end for a, b in covered):
                yield data[start:end].decode("utf-8"), context(data, start, end)


def print_stray(name: str, records: list[dict], kind: str, examples: int) -> None:
    found = collections.Counter()
    contexts = {}
    for text, around in stray(records, kind):
        found[text] += 1
        contexts.setdefault(text, around)
    print(f"\n## {name}: {sum(found.values())} stray {kind} tokens, {len(found)} different")
    for text, times in found.most_common(examples):
        print(f"{times:6}  {text!r:40} …{contexts[text]}…")


def print_misses(name: str, records: list[dict], label: str, examples: int) -> None:
    misses = [
        (outcome, kinds, value, cut, around)
        for found, outcome, kinds, value, cut, around in evaluate(records)
        if found == label and (outcome != "exact" or kinds[0] not in EXPECTED.get(label, kinds))
    ]
    patterns = collections.Counter((outcome, kinds) for outcome, kinds, _, _, _ in misses)
    print(f"\n## {name}: {len(misses)} misses of {label}")
    for (outcome, kinds), times in patterns.most_common(examples):
        _, _, value, cut, around = next(m for m in misses if (m[0], m[1]) == (outcome, kinds))
        print(f"{times:6}  {outcome:7} {' '.join(kinds)}")
        print(f"        {value!r} cut as {cut!r}")
        print(f"        …{around}…")


def print_summary(name: str, records: list[dict]) -> None:
    outcomes = collections.defaultdict(collections.Counter)
    kinds_of_exact = collections.defaultdict(collections.Counter)
    for label, outcome, kinds, _, _, _ in evaluate(records):
        outcomes[label][outcome] += 1
        if outcome == "exact":
            kinds_of_exact[label][kinds[0]] += 1
    print(f"\n## {name}")
    print(f"{'label':22} {'spans':>6} {'exact':>6} {'inside':>6} {'split':>6} {'ragged':>6}  kinds of the exact tokens")
    for label, counter in sorted(outcomes.items(), key=lambda item: -sum(item[1].values())):
        total = sum(counter.values())
        shares = " ".join(f"{100 * counter[outcome] / total:5.1f}%" for outcome in ("exact", "inside", "split", "ragged"))
        top = ", ".join(f"{kind} {times}" for kind, times in kinds_of_exact[label].most_common(3))
        mark = ""
        if label in EXPECTED:
            good = sum(times for kind, times in kinds_of_exact[label].items() if kind in EXPECTED[label])
            mark = f"  [expected kind: {100 * good / total:.1f}%]"
        print(f"{label:22} {total:6} {shares}  {top}{mark}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("corpora", nargs="*", help="corpus names; all when left out")
    parser.add_argument("-l", "--label", help="print misses of this label")
    parser.add_argument("-n", "--examples", type=int, default=20, help="how many to print")
    parser.add_argument("--stray", metavar="KIND", help="print tokens of this kind outside its spans")
    parser.add_argument("--catalog", action="store_true", help="lex with every matcher of the catalog")
    args = parser.parse_args()
    if args.catalog:
        OPTIONS.append("--catalog")
        EXPECTED.update(EXPECTED_WITH_CATALOG)
    if not BINARY.exists():
        sys.exit("build first: cargo build --release, in rust/")
    paths = sorted(LABELLED.glob("*.jsonl"))
    if args.corpora:
        paths = [path for path in paths if path.stem in args.corpora]
    if not paths:
        sys.exit("no corpus: run python3 tools/corpora.py")

    for path in paths:
        records = load(path)
        if args.stray:
            print_stray(path.stem, records, args.stray, args.examples)
        elif args.label:
            print_misses(path.stem, records, args.label, args.examples)
        else:
            print_summary(path.stem, records)


if __name__ == "__main__":
    main()
