#!/usr/bin/env python3
"""Generate tolkien's Unicode tables from the Unicode Character Database.

Writes two files:

  src/unicode_tables.rs   the tables the matchers use
  docs/charsets.md        every member of every named set, for review

Nothing in either file is written by hand. Each table is defined below by a
rule over the UCD (a general category, a binary property, a line-break class,
or a pattern over character names), so a new Unicode version is a change to
VERSION and SHA256 followed by a review of the diff in docs/charsets.md.

Usage:
  python3 tools/gen_unicode.py           regenerate both files
  python3 tools/gen_unicode.py --check   fail if the committed files are stale

Pure standard library. The UCD files are downloaded once into tools/.ucd/
(not committed) and checked against the hashes pinned here.
"""

from __future__ import annotations

import hashlib
import re
import sys
import urllib.request
from pathlib import Path

VERSION = "18.0.0"
SHA256 = {
    "UnicodeData.txt": "0736451de439ae7baf1425136617da495e09ee5afbe6e394374db7009ea08950",
    "PropList.txt": "f438f532e8737bb8a2702126cdf9c4af5e357c58c7acf9d9eb2fc7c1a1d955d6",
    "LineBreak.txt": "ae8cf1970c73f3f1a12d77852df96c4b2b1723ca8b388e36bb5739250c6849de",
}

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / "tools" / ".ucd" / VERSION
RUST_OUT = ROOT / "src" / "unicode_tables.rs"
REVIEW_OUT = ROOT / "docs" / "charsets.md"

# ── Rules ────────────────────────────────────────────────────────────────

# Range tables: every code point whose general category is in the list.
CATEGORY_TABLES = [
    ("LETTER", ["Lu", "Ll", "Lt", "Lm", "Lo"], "General category L: letters."),
    ("DECIMAL_DIGIT", ["Nd"], "General category Nd: decimal digits."),
    ("NUMBER", ["Nd", "Nl", "No"], "General category N: digits, letter numbers, other numbers."),
    ("MATH_SYMBOL", ["Sm"], "General category Sm."),
    ("DASH_PUNCTUATION", ["Pd"], "General category Pd."),
    ("CONNECTOR_PUNCTUATION", ["Pc"], "General category Pc."),
    ("CURRENCY_SYMBOL", ["Sc"], "General category Sc."),
]

# Range tables: a binary property from PropList.txt.
PROPERTY_TABLES = [
    ("WHITE_SPACE", "White_Space", "Property White_Space."),
]

PUNCTUATION = ["Pc", "Pd", "Ps", "Pe", "Pi", "Pf", "Po"]
# The ornament forms of "!" and "?" (U+2753 and friends) are category So.
MARKS = PUNCTUATION + ["So"]

# Named sets: small lists of single characters. `name` is a regular
# expression searched in the character name, `categories` narrows it,
# `exclude` is a second expression that drops a name the first one caught,
# `prop` takes a PropList.txt property, `line_break` takes LineBreak.txt
# classes, and `extra` adds code points no rule reaches, each with its reason.
NAMED_SETS = [
    {
        "id": "FULL_STOP",
        "doc": "Full stops of every script and width.",
        "name": r"\bFULL STOP$",
        "categories": PUNCTUATION,
    },
    {
        "id": "COMMA",
        "doc": "Commas of every script and width.",
        "name": r"\bCOMMA$",
        "categories": PUNCTUATION,
    },
    {
        "id": "COLON",
        "doc": "Colons of every script and width.",
        "name": r"\bCOLON$",
        "categories": PUNCTUATION,
    },
    {
        "id": "QUESTION_MARK",
        "doc": "Question marks, including the ornament forms.",
        "name": r"\bQUESTION MARK\b",
        "categories": MARKS,
    },
    {
        "id": "EXCLAMATION_MARK",
        "doc": "Exclamation marks, including the ornament forms.",
        "name": r"\bEXCLAMATION MARK\b",
        "categories": MARKS,
        # U+1F199 SQUARED UP WITH EXCLAMATION MARK and U+1F51B ON WITH
        # EXCLAMATION MARK ... are the emoji words "UP!" and "ON!".
        "exclude": r"\bWITH\b",
    },
    {
        "id": "APOSTROPHE",
        "doc": "Apostrophes that can stand inside a word.",
        "name": r"\bAPOSTROPHE$",
        "categories": PUNCTUATION + ["Lm"],
        "extra": {
            0x2019: "RIGHT SINGLE QUOTATION MARK is the preferred apostrophe in "
            "running text (The Unicode Standard, section 6.2), but is not named one",
        },
    },
    {
        "id": "DASH",
        "doc": "Property Dash: hyphens, dashes and minus signs.",
        "prop": "Dash",
    },
    {
        "id": "UNDERSCORE",
        "doc": "Low lines (underscores).",
        "name": r"\bLOW LINE$",
        "categories": PUNCTUATION,
    },
    {
        "id": "SLASH",
        "doc": "Slashes and backslashes.",
        "name": r"^(FULLWIDTH |SMALL |BIG )?(REVERSE )?SOLIDUS$|^(FRACTION|DIVISION) SLASH$",
        "categories": PUNCTUATION + ["Sm"],
    },
    {
        "id": "PERCENT_SIGN",
        "doc": "Percent signs.",
        "name": r"\bPERCENT SIGN$",
        "categories": PUNCTUATION,
    },
    {
        "id": "NUMBER_SIGN",
        "doc": "Number signs (#).",
        "name": r"\bNUMBER SIGN$",
        "categories": PUNCTUATION,
    },
    {
        "id": "LINE_TERMINATION",
        "doc": "Characters that force a line break: line-break classes BK, CR, LF and NL.",
        "line_break": ["BK", "CR", "LF", "NL"],
    },
]

# ── UCD loading ──────────────────────────────────────────────────────────


def fetch(name: str) -> str:
    path = CACHE / name
    if not path.exists():
        CACHE.mkdir(parents=True, exist_ok=True)
        url = f"https://www.unicode.org/Public/{VERSION}/ucd/{name}"
        with urllib.request.urlopen(url, timeout=120) as response:
            path.write_bytes(response.read())
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != SHA256[name]:
        sys.exit(f"{name}: sha256 {digest} does not match the pinned {SHA256[name]}")
    return data.decode("utf-8")


def load_unicode_data() -> tuple[dict[int, tuple[str, str]], list[tuple[int, int, str]]]:
    """Returns (code point -> (name, category), [(first, last, category)])."""
    chars: dict[int, tuple[str, str]] = {}
    spans: list[tuple[int, int, str]] = []
    range_start: int | None = None
    for line in fetch("UnicodeData.txt").splitlines():
        fields = line.split(";")
        cp, name, category = int(fields[0], 16), fields[1], fields[2]
        if name.endswith(", First>"):
            range_start = cp
        elif name.endswith(", Last>"):
            assert range_start is not None
            spans.append((range_start, cp, category))
            range_start = None
        else:
            chars[cp] = (name, category)
            spans.append((cp, cp, category))
    return chars, spans


def load_property_file(name: str) -> dict[str, list[tuple[int, int]]]:
    """Parses `XXXX[..YYYY] ; Value # comment` files into value -> ranges."""
    values: dict[str, list[tuple[int, int]]] = {}
    for line in fetch(name).splitlines():
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        span, value = (part.strip() for part in line.split(";")[:2])
        first, _, last = span.partition("..")
        values.setdefault(value, []).append((int(first, 16), int(last or first, 16)))
    return values


def merge(ranges: list[tuple[int, int]]) -> list[tuple[int, int]]:
    merged: list[tuple[int, int]] = []
    for first, last in sorted(ranges):
        if merged and first <= merged[-1][1] + 1:
            merged[-1] = (merged[-1][0], max(merged[-1][1], last))
        else:
            merged.append((first, last))
    return merged


# ── Building ─────────────────────────────────────────────────────────────


def build():
    chars, spans = load_unicode_data()
    props = load_property_file("PropList.txt")
    line_breaks = load_property_file("LineBreak.txt")

    range_tables = []
    for ident, categories, doc in CATEGORY_TABLES:
        ranges = merge([(a, b) for a, b, category in spans if category in categories])
        range_tables.append((ident, doc, ranges))
    for ident, prop, doc in PROPERTY_TABLES:
        range_tables.append((ident, doc, merge(props[prop])))

    named_sets = []
    for rule in NAMED_SETS:
        members: dict[int, str] = {}
        described = []
        if "name" in rule:
            pattern = re.compile(rule["name"])
            exclude = re.compile(rule["exclude"]) if "exclude" in rule else None
            categories = rule["categories"]
            for cp, (name, category) in chars.items():
                if category not in categories or not pattern.search(name):
                    continue
                if exclude and exclude.search(name):
                    continue
                members[cp] = "name"
            text = f"name matches `{rule['name']}`"
            if exclude:
                text += f" and not `{rule['exclude']}`"
            described.append(f"{text}, category in {', '.join(categories)}")
        if "prop" in rule:
            for first, last in props[rule["prop"]]:
                for cp in range(first, last + 1):
                    members[cp] = "property"
            described.append(f"property `{rule['prop']}`")
        if "line_break" in rule:
            for cls in rule["line_break"]:
                for first, last in line_breaks[cls]:
                    for cp in range(first, last + 1):
                        members[cp] = f"line-break class {cls}"
            described.append(f"line-break class in {', '.join(rule['line_break'])}")
        for cp, reason in rule.get("extra", {}).items():
            members[cp] = f"added: {reason}"
        named_sets.append((rule, "; ".join(described), dict(sorted(members.items()))))

    return chars, range_tables, named_sets


def display_name(chars, cp: int) -> str:
    name, _ = chars.get(cp, ("<unassigned>", "Cn"))
    if name == "<control>":
        return f"<control-{cp:04X}>"
    return name


def render_rust(chars, range_tables, named_sets) -> str:
    out = [
        f"// @generated by tools/gen_unicode.py from Unicode {VERSION}. Do not edit:",
        "// change the rules in that script and run it again.",
        "",
        "/// The Unicode version these tables were generated from.",
        f'pub const UNICODE_VERSION: &str = "{VERSION}";',
    ]
    for ident, doc, ranges in range_tables:
        out += ["", f"/// {doc} Sorted, inclusive code point ranges.", "#[rustfmt::skip]"]
        out.append(f"pub(crate) const {ident}: &[(u32, u32)] = &[")
        row: list[str] = []
        for first, last in ranges:
            row.append(f"(0x{first:04X}, 0x{last:04X}),")
            if len(row) == 5:
                out.append("    " + " ".join(row))
                row = []
        if row:
            out.append("    " + " ".join(row))
        out.append("];")
    for rule, _, members in named_sets:
        out += ["", f"/// {rule['doc']} Sorted.", "#[rustfmt::skip]"]
        out.append(f"pub(crate) const {rule['id']}: &[char] = &[")
        for cp in members:
            out.append(f"    '\\u{{{cp:04X}}}', // {display_name(chars, cp)}")
        out.append("];")
    return "\n".join(out) + "\n"


def render_review(chars, range_tables, named_sets) -> str:
    out = [
        "# Character sets",
        "",
        f"Generated by `tools/gen_unicode.py` from Unicode {VERSION}. Do not edit.",
        "",
        "This file exists to be reviewed. Each named set below lists every",
        "character the rule selected, so a change to a rule or to the Unicode",
        "version shows up here as a diff a person can read.",
        "",
        "## Range tables",
        "",
        "| Table | Rule | Ranges | Code points |",
        "|---|---|---:|---:|",
    ]
    for ident, doc, ranges in range_tables:
        total = sum(last - first + 1 for first, last in ranges)
        out.append(f"| `{ident}` | {doc} | {len(ranges)} | {total} |")
    for rule, described, members in named_sets:
        out += [
            "",
            f"## {rule['id']}",
            "",
            f"{rule['doc']} {len(members)} characters.",
            "",
            f"Rule: {described}.",
            "",
            "| Code point | Char | Name | Category | Selected by |",
            "|---|---|---|---|---|",
        ]
        for cp, why in members.items():
            _, category = chars.get(cp, ("", "Cn"))
            glyph = chr(cp) if category[0] not in "CZ" else ""
            if glyph in ("|", "\\", "`"):
                glyph = "\\" + glyph
            out.append(f"| U+{cp:04X} | {glyph} | {display_name(chars, cp)} | {category} | {why} |")
    return "\n".join(out) + "\n"


def main() -> None:
    chars, range_tables, named_sets = build()
    outputs = {
        RUST_OUT: render_rust(chars, range_tables, named_sets),
        REVIEW_OUT: render_review(chars, range_tables, named_sets),
    }
    if "--check" in sys.argv[1:]:
        stale = [p for p, text in outputs.items() if not p.exists() or p.read_text("utf-8") != text]
        if stale:
            names = ", ".join(str(p.relative_to(ROOT)) for p in stale)
            sys.exit(f"stale: {names}. Run python3 tools/gen_unicode.py")
        print(f"up to date with Unicode {VERSION}")
        return
    for path, text in outputs.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, "utf-8")
        print(f"wrote {path.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
