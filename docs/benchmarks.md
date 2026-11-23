# Tokens against labelled text

How tolkien cuts the spans that people labelled in two public datasets, and
what the fuzzer found. Measured on 23 November 2026; reproduce with:

```bash
python3 tools/corpora.py
cd rust
cargo build --release && python3 ../tools/evaluate.py
cargo run --release --example fuzz -- --seconds 60 ../corpus/docs/*.txt
```

## Corpora

| Corpus | What it is | Documents | Labelled spans |
|---|---|---:|---:|
| `ai4privacy-en` | pii-masking-200k, English: synthetic sentences, 56 kinds of personal data | 43,501 | 137,093 |
| `ai4privacy-de` | the same, German | 52,817 | 163,844 |
| `tab` | Text Anonymization Benchmark, dev split: court cases labelled by hand | 127 | 10,078 |
| `wikipedia` | one article in each of 24 languages and scripts | 24 | none |
| `markit` | markit's benchmark documents converted to markdown | 10 | none |
| `code` | this repository's sources | 31 | none |

The ai4privacy text is generated, and it shows: currencies are glued to
amounts (`764,336.48Rufiyaa`), and a "time" can be the bare number `20`. Its
numbers say how a shape is cut, not how often real text holds that shape.
TAB is real prose.

## What the outcomes mean

For each labelled span: **exact** means one token covers exactly the span,
**inside** that one token covers the span and more, **split** that several
tokens cover exactly the span, and **ragged** that a token crosses its start
or its end. Ragged is the bad one: no combination of whole tokens gives the
span back.

## Labels a matcher exists for

Share of spans that are one token of the expected kind. "Before" is the
lexer as it stood before this run (commit `6d06d83`).

| Corpus | Label | Spans | Before | After | What is left |
|---|---|---:|---:|---:|---|
| ai4privacy-en | `EMAIL` | 4,092 | 99.8% | 99.8% | 9 addresses glued to other text in the source |
| ai4privacy-en | `URL` | 2,910 | 98.6% | 99.8% | 39 took a closing bracket before; that is fixed |
| ai4privacy-en | `DATE` | 4,555 | 67.8% | 67.8% | 32% have no year: `20th September`, `1/20` |
| ai4privacy-en | `DOB` | 2,354 | 67.2% | 67.2% | the same |
| ai4privacy-en | `TIME` | 2,883 | 54.3% | 66.0% | `4 PM` is new; the rest are bare numbers (`20`) and words (`quarter to 23`) |
| ai4privacy-en | `AMOUNT` | 3,103 | 20.4% | 14.4% | see below |
| tab | `DATETIME` | 2,864 | 71.6% | 74.6% | no year (`30 January`), no day (`March 2006`), durations (`three months`) |

`AMOUNT` fell because at least 146 of the "before" hits were wrong: `582k`
was read as 582 kina. The label marks the number only, while a `CURRENCY`
token holds the amount and its currency, and more than half of the amounts
carry a magnitude letter (`0.6m`, `998k`) or a currency code in front
(`Rp0.218m`), neither of which tolkien reads.

## Labels made of digits

No built-in matcher is aimed at these; the catalog has matchers for most
of them, scored further down. They count because a detector for card
numbers or IP addresses is built on the tokens. Share that is exactly one
token with the default lexer:

| Label | Spans | Before | After |
|---|---:|---:|---:|
| `IPV4` | 2,720 | 45.8% | 99.6% |
| `CREDITCARDNUMBER` | 2,574 | 56.9% | 99.4% |
| `ACCOUNTNUMBER` | 2,831 | 51.6% | 97.7% |
| `BUILDINGNUMBER` | 2,714 | 54.8% | 98.4% |
| `ZIPCODE` | 2,322 | 49.4% | 98.9% |
| `PHONEIMEI` | 1,875 | 50.5% | 99.6% |
| `CREDITCARDCVV` | 869 | 50.6% | 98.8% |
| `PIN` | 738 | 53.5% | 99.6% |
| `MASKEDNUMBER` | 2,131 | 47.3% | 91.2% |
| `SSN` | 2,045 | 42.9% | 75.7% |
| `PHONENUMBER` | 2,425 | 17.2% | 37.9% |

Two defects accounted for the "before" column. A number kept the full stop
or comma after it (`215.114.180.213.`), which hit every number that ended a
sentence. And a numeric date was any three short numbers, so the first three
parts of `29.28.197.248` were a date and the fourth a number.

What is left: an SSN or a phone number written with spaces is several
tokens (`838 44 5162`, `+390 599-968-1230`), and a masked number glued to a
currency sign or a `#` is part of that token.

## Tokens that claim a kind outside every label

A token of a kind with no label of that kind under it. Some are real (the
datasets do not label everything), so these are upper bounds on false
positives, English and TAB together:

| Kind | Before | After | What is left |
|---|---:|---:|---|
| `DATE` | 2,942 | 47 | `4.1.10` in a browser version; `07-27-75` at the start of a phone number; 11 real dates TAB did not label |
| `TIME` | 173 | 120 | mostly real times the dataset did not label (`5 pm`, `10AM`); the tail of a MAC address |
| `CURRENCY` | 1,549 | 802 | lone symbols (`$`, `£`), which are real; `21 para` |
| `LIKE_EMAIL` | 13 | 13 | all real addresses |
| `LIKE_URL` | 3 | 3 | all real addresses |

Before, `DATE` took version numbers (`2.9.165`, `10_7_2`) and `CURRENCY`
took the start of any string that began with a digit and a letter the
currency list knows in some case (`3L7gwx…`, `0da8`).

## The catalog

The matchers of the catalog, added with `--catalog`, against the labels
they cover in `ai4privacy-en`. Share of spans that are one token of the
expected kind; "default" is the share that is one token of any kind without
the catalog, where such a span is at best a number or an alphanumeric
sequence: the catalog gives it its kind, and makes one token of the
addresses and numbers that were several.

| Label | Spans | One token, default | Expected kind, with the catalog | What is left |
|---|---:|---:|---:|---|
| `IPV4` | 2,720 | 99.6% | 99.7% | |
| `IPV6` | 2,401 | 0% | 99.6% | |
| `MAC` | 1,114 | 0% | 99.8% | |
| `CREDITCARDNUMBER` | 2,574 | 99.4% | 99.3% | glued to other text in the source |
| `IBAN` | 1,973 | 99.9% | 99.9% | |
| `PHONEIMEI` | 1,875 | 99.6% | 99.7% | |
| `NEARBYGPSCOORDINATE` | 1,651 | 0% | 99.9% | |
| `ETHEREUMADDRESS` | 1,624 | 99.7% | 99.7% | |
| `PHONENUMBER` | 2,425 | 37.9% | 91.3% | a space between groups with no leading zero or `+` |
| `BITCOINADDRESS` | 2,403 | 99.5% | 66.0% | the dataset's addresses are up to 36 characters; a real one has at most 35 |
| `SSN` | 2,045 | 75.7% | 48.6% | half are not of the United States form: `756.4511.8112`, eleven digits together |

The labels a built-in matcher covers are unchanged with the catalog:
`DATE` 67.8%, `TIME` 66.0%, `EMAIL` and `URL` 99.8%.

Labels say what a matcher finds, not what it finds by mistake. For that,
the tokens of every kind of the catalog were read in the four corpora
without labels (court cases, Wikipedia, converted documents, this
repository). The first version was wrong in eight ways, each now a refused
example of its entry:

| What was found | Taken for | Now |
|---|---|---|
| ISBNs, `978-3-931911-18-8` and `3-922296-21-1` | a card number, a telephone number | `ISBN` |
| `192.168.1.20` | a telephone number | needs a group of four digits |
| Case numbers, `682/02` | a fraction | no leading zero |
| Issue numbers, `#516` | a colour | three or four digits need a letter |
| `2008 s`, `22 h` | a duration | a unit of one letter stands against its number |
| `:*` and `:\|` in markdown, `=(` in code, `:3` in a ratio | an emoticon | left out |
| `./C.` in `W./C. 6608` | a file path | two characters of a name |
| A column of numbers, `1 22 333 4444 55555` | a card number | the first group has four digits |

What a shape cannot tell apart is still there: a 19-digit number is a
`CARD_NUMBER`, `$HOME` a `CASHTAG`, `2E18` a `SCIENTIFIC_NUMBER`.

## What the fuzzer found

Nothing, in 22 MB of documents and 5.1 million mutations over three seeds:
no gap in coverage, no token that breaks its kind's rules, no token that
depends on what stands before it, and no text slower than its length
allows. Its checks were tested by planting twelve bugs in the matchers, one
at a time. It reported eleven within five seconds and cut each text down to
a handful of characters (`"a@e.c."`, `"25:00"`, `"2023 8-8"`). It missed
one, a word list that accepted any prefix of a month name; the date check
now holds its own list of months and catches it.

The fuzzer has two more checks since that run, for the faster lexer: the
tokens are those of asking every matcher at every position, and they are
the same when the text is lexed in pieces. Twenty wrong shortcut rules,
planted one at a time, were each caught twice: by the texts of
`shortcuts_and_hints_change_no_token`, and by the fuzzer within eight
seconds.

It did not find the defects above, and could not have: every one of them
was a rule doing what it said. They came from comparing the tokens with
what people had labelled.

## Decisions these numbers ask for

Each is a new rule or a new matcher, not a repair. The catalog answers
three of them and leaves the default lexer as it was; the others are open.

| Gap | Size | What it would take |
|---|---|---|
| Dates without a year or a day | 32% of ai4privacy dates, about 9% of TAB's | Accept `30 January`, `20th September`, `March 2006`. fastlex rejected these on purpose. `1/20` cannot be told from a fraction |
| Phone numbers | 62% are several tokens | Answered by the catalog's `PHONE_NUMBER`: 91.3% are one token |
| SSNs with spaces | 23% are three tokens | Answered by the catalog's `US_SSN` for the United States form |
| IPv6 and MAC addresses | 0% are one token; a MAC tail reads as a `TIME` | Answered by the catalog's `IPV6` and `MAC_ADDRESS`, which also take the tail from `TIME` |
| Amounts with a magnitude or a leading code | more than half of ai4privacy amounts | `$5k`, `0.6m`; `USD 5`, `Rp 500`. A leading code brings back collisions (`K9`, `R2`) |
| Possessives | 4.5% of first names are `Abby's` | Cut `'s` off a word, or leave it to the consumer |
| `para`, `K` and other currency words | 45 of 71 stray `CURRENCY` tokens in TAB | Prune the currency list to names nobody uses as words |
