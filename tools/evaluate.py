#!/usr/bin/env python3
"""Compare tolkien's tokens with the labelled spans of the prepared corpora.

For every labelled span (an email address, a date, a phone number...) this
asks how the lexer cut the text there:

  exact    one token covers exactly the span
  inside   one token covers the span and more
  split    several tokens cover exactly the span
  ragged   a token crosses the start or the end of the span

and, for the labels a matcher exists for, whether an exact token has the
expected kind. The labels no matcher exists for (phone numbers, card
numbers) are reported too: how they are cut today is what a detector built
on these tokens has to work with.

Usage:
  python3 tools/evaluate.py                    every corpus, a summary table
  python3 tools/evaluate.py -l DATE -n 30      thirty misses of one label
  python3 tools/evaluate.py --stray DATE       tokens of a kind outside every
                                               span that expects that kind

Run tools/corpora.py first, and build with `cargo build --release`.
Pure standard library.
"""

from __future__ import annotations

import argparse
import collections
import itertools
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LABELLED = ROOT / "corpus" / "labelled"
BINARY = ROOT / "target" / "release" / "tolkien"
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


def lex(text: str) -> list[tuple[str, int, int]]:
    """The (kind, start, end) of every token of `text`, in bytes."""
    result = subprocess.run([str(BINARY)], input=text.encode("utf-8"), capture_output=True, check=True)
    tokens = []
    for line in result.stdout.decode("utf-8").splitlines():
        kind, start, end, _ = line.split("\t", 3)
        tokens.append((kind, int(start), int(end)))
    return tokens


def evaluate(path: Path):
    """Yields (label, outcome, kinds, span text, context) for every span."""
    records = [json.loads(line) for line in path.open(encoding="utf-8")]
    whole = SEPARATOR.join(record["text"] for record in records)
    tokens = lex(whole)
    data = whole.encode("utf-8")
    starts = [start for _, start, _ in tokens]

    base = 0  # byte offset of the current document in `whole`
    index = 0  # first token that can touch the current span
    for record in records:
        text = record["text"]
        # Byte offset of every character offset of this document.
        offsets = [0, *itertools.accumulate(len(ch.encode("utf-8")) for ch in text)]
        for start, end, label in sorted(record["spans"]):
            first, last = base + offsets[start], base + offsets[end]
            while index > 0 and starts[index] > first:
                index -= 1
            while index + 1 < len(tokens) and starts[index + 1] <= first:
                index += 1
            stop = index
            while stop < len(tokens) and starts[stop] < last:
                stop += 1
            touching = tokens[index:stop]
            kinds = tuple(kind for kind, _, _ in touching)
            aligned_start = touching[0][1] == first
            aligned_end = touching[-1][2] == last
            if len(touching) == 1 and aligned_start and aligned_end:
                outcome = "exact"
            elif len(touching) == 1:
                outcome = "inside"
            elif aligned_start and aligned_end:
                outcome = "split"
            else:
                outcome = "ragged"
            cut = " ".join(data[a:b].decode("utf-8") for _, a, b in touching)
            context = data[max(base, first - 30) : last + 30].decode("utf-8", "replace")
            yield label, outcome, kinds, text[start:end], cut, context.replace("\n", " ")
        base += len(text.encode("utf-8")) + len(SEPARATOR)


def stray(path: Path, kind: str):
    """Yields the text and context of tokens of `kind` that no span expecting
    that kind touches."""
    labels = {label for label, kinds in EXPECTED.items() if kind in kinds}
    records = [json.loads(line) for line in path.open(encoding="utf-8")]
    for record in records:
        text = record["text"]
        data = text.encode("utf-8")
        offsets = [0, *itertools.accumulate(len(ch.encode("utf-8")) for ch in text)]
        covered = [(offsets[s], offsets[e]) for s, e, label in record["spans"] if label in labels]
        record["data"], record["covered"] = data, covered
    whole = SEPARATOR.join(record["text"] for record in records)
    tokens = iter(lex(whole))
    base = 0
    token = next(tokens, None)
    for record in records:
        size = len(record["data"])
        while token and token[1] < base + size:
            name, start, end = token[0], token[1] - base, token[2] - base
            if name == kind and not any(start < b and a < end for a, b in record["covered"]):
                context = record["data"][max(0, start - 30) : end + 30].decode("utf-8", "replace")
                yield record["data"][start:end].decode("utf-8"), context.replace("\n", " ")
            token = next(tokens, None)
        base += size + len(SEPARATOR)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("corpora", nargs="*", help="corpus names; all when left out")
    parser.add_argument("-l", "--label", help="print misses of this label")
    parser.add_argument("-n", "--examples", type=int, default=20, help="how many to print")
    parser.add_argument("--stray", metavar="KIND", help="print tokens of this kind outside its spans")
    args = parser.parse_args()
    if not BINARY.exists():
        sys.exit("build first: cargo build --release")
    paths = sorted(LABELLED.glob("*.jsonl"))
    if args.corpora:
        paths = [path for path in paths if path.stem in args.corpora]
    if not paths:
        sys.exit("no corpus: run python3 tools/corpora.py")

    for path in paths:
        if args.stray:
            found = collections.Counter()
            contexts = {}
            for text, context in stray(path, args.stray):
                found[text] += 1
                contexts.setdefault(text, context)
            print(f"\n## {path.stem}: {sum(found.values())} stray {args.stray} tokens, {len(found)} different")
            for text, times in found.most_common(args.examples):
                print(f"{times:6}  {text!r:40} …{contexts[text]}…")
            continue

        outcomes = collections.defaultdict(collections.Counter)
        kinds_of_exact = collections.defaultdict(collections.Counter)
        misses = []
        for label, outcome, kinds, value, cut, context in evaluate(path):
            outcomes[label][outcome] += 1
            if outcome == "exact":
                kinds_of_exact[label][kinds[0]] += 1
            wrong = outcome != "exact" or (label in EXPECTED and kinds[0] not in EXPECTED[label])
            if label == args.label and wrong:
                misses.append((outcome, kinds, value, cut, context))

        if args.label:
            patterns = collections.Counter((outcome, kinds) for outcome, kinds, _, _, _ in misses)
            print(f"\n## {path.stem}: {len(misses)} misses of {args.label}")
            for (outcome, kinds), times in patterns.most_common(args.examples):
                example = next(m for m in misses if (m[0], m[1]) == (outcome, kinds))
                print(f"{times:6}  {outcome:7} {' '.join(kinds)}")
                print(f"        {example[2]!r} cut as {example[3]!r}")
                print(f"        …{example[4]}…")
            continue

        print(f"\n## {path.stem}")
        print(f"{'label':22} {'spans':>6} {'exact':>6} {'inside':>6} {'split':>6} {'ragged':>6}  kinds of the exact tokens")
        for label, counter in sorted(outcomes.items(), key=lambda item: -sum(item[1].values())):
            total = sum(counter.values())
            share = lambda name: f"{100 * counter[name] / total:5.1f}%"
            top = ", ".join(f"{kind} {times}" for kind, times in kinds_of_exact[label].most_common(3))
            mark = ""
            if label in EXPECTED:
                good = sum(times for kind, times in kinds_of_exact[label].items() if kind in EXPECTED[label])
                mark = f"  [expected kind: {100 * good / total:.1f}%]"
            print(f"{label:22} {total:6} {share('exact')} {share('inside')} {share('split')} {share('ragged')}  {top}{mark}")


if __name__ == "__main__":
    main()
