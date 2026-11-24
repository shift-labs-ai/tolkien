# Where the catalog's forms come from

Each matcher of the catalog takes its form from a standard, from the page
of the vendor that issues the thing, or from usage where nothing is
written down. This table says which, and where the matcher departs from
its source. It is the `source` of each `Entry` in `rust/src/catalog.rs`,
and a test keeps the two the same.

The sources were read on 24 November 2026. A vendor changes its token
formats without notice: GitHub says its tokens may grow to 255 characters,
which is why no length here is exact where a vendor gives none.

| Kind | Source |
|---|---|
| `AWS_ACCESS_KEY_ID` | AWS IAM User Guide, IAM identifiers: `AKIA` is an access key and `ASIA` a temporary one. AWS does not publish the alphabet; base32 is that of its examples and of the gitleaks rule |
| `GITHUB_TOKEN` | GitHub Docs, About authentication to GitHub, for the prefixes; GitHub Blog, Behind GitHub's new authentication token formats (2021), for the 36 characters. The 82 after `github_pat_` are from the gitleaks rule, not from GitHub |
| `SLACK_TOKEN` | Slack developer docs, Tokens: `xoxb-`, `xoxp-`, `xapp-`, `xwfp-`, `xoxe.xapp-`. The older and rotating prefixes are from the gitleaks rules. Slack publishes no length |
| `STRIPE_KEY` | Stripe Docs, API keys: secret, publishable and restricted keys in test and live mode. Stripe publishes no length |
| `GOOGLE_API_KEY` | Not published by Google: the gitleaks rule `gcp-api-key` |
| `NPM_TOKEN` | GitHub Changelog, npm has a new access token format (2021): the prefix, and the format of GitHub's tokens |
| `JWT` | RFC 7519 and RFC 7515: three base64url parts; a JSON object in base64url starts with `eyJ`; an unsecured JWT has an empty signature (RFC 7519, section 6) |
| `PEM_HEADER` | RFC 7468, section 3: a label does not start or end with a space, and holds no two together. The standard allows any printable character but a hyphen in it, and a hyphen between words; the labels in use are words in capitals and digits |
| `CIDR` | RFC 4632, section 3.1: a prefix length from 0 to 32 |
| `IPV4` | RFC 3986, section 3.2.2, `IPv4address` |
| `IPV6` | RFC 4291, section 2.2. A zone after `%` (RFC 4007) is not read |
| `MAC_ADDRESS` | IEEE 802 as summarised in Wikipedia, MAC address: hyphens or colons between pairs, or the three groups of four that Cisco writes. Twelve hex digits with no separator are not taken |
| `URI` | RFC 3986 for the characters; the IANA registry of URI schemes for the schemes. `ftps`, `s3`, `gs`, `amqp`, `amqps`, `mysql`, `postgres`, `postgresql` and `mongodb+srv` are not registered and are taken because they are in use |
| `UUID` | RFC 9562, section 4 |
| `ETHEREUM_ADDRESS` | ERC-55: `0x` and 40 hex digits in any case. The checksum in the case of the letters is not checked |
| `HASH` | The digest lengths of MD5 (RFC 1321) and of SHA-1 and SHA-2 (FIPS 180-4), in hex |
| `HEX_NUMBER` | The `0x` literal of C and of the languages after it |
| `VERSION` | Semantic Versioning 2.0.0, items 9 and 10, with the `v` that tags carry. Two to four numbers are taken, where SemVer has three |
| `SCIENTIFIC_NUMBER` | The E notation of programming languages and of IEEE 754 decimal strings |
| `HEX_COLOR` | CSS Color Module Level 4, the hex notations. Three, four or eight digits with no letter are left out, a departure: they are more often the number of an issue or of an account |
| `FILE_PATH` | POSIX path notation. An absolute path is not taken: `/` starts too much else. Nor is `./configure`: after `./` a second slash or a full stop is asked for, since `Mr./Ms` is not a path |
| `WINDOWS_PATH` | Microsoft Learn, Naming Files, Paths, and Namespaces. A name may hold spaces and more marks than are taken here |
| `UTC_OFFSET` | The offsets in use, from UTC-12:00 to UTC+14:00 on the hour or at 30 or 45 minutes (Wikipedia, List of UTC offsets) |
| `ISBN` | The ISBN Users' Manual as summarised in Wikipedia, ISBN: 13 digits in five parts after `978` or `979`, or 10 in four parts with a check character that may be `X` |
| `IMEI` | 3GPP TS 23.003 as summarised in Wikipedia, International Mobile Equipment Identity: `AA-BBBBBB-CCCCCC-D`, and `-EE` for an IMEISV |
| `US_SSN` | Social Security Administration, as summarised in Wikipedia, Social Security number: `AAA-GG-SSSS` and the numbers never assigned |
| `CARD_NUMBER` | ISO/IEC 7812 as summarised in Wikipedia, Payment card number: 8 to 19 digits. 13 to 19 are taken, the lengths the card networks issue |
| `IBAN` | ISO 13616 as summarised in Wikipedia, International Bank Account Number: up to 34 characters, 15 at the least, printed in groups of four |
| `BITCOIN_ADDRESS` | Bitcoin Wiki, Invoice address, for the 26 to 35 base58 characters; BIP 173 and BIP 350 for bech32 |
| `CASHTAG` | twitter-text, the cashtag rule. Twitter takes any case; capitals alone are taken here, since `$path` is more often a variable |
| `COORDINATES` | ISO 6709, decimal degrees. The brackets and the count of decimals are what sets a pair apart from two numbers |
| `PHONE_NUMBER` | ITU-T E.164 for the 15 digits at the most. The grouping is usage, not a standard |
| `ORDINAL` | English usage |
| `FRACTION` | English usage |
| `DURATION` | English usage, and the SI symbols `s`, `ms`, `min` and `h` |
| `MEASUREMENT` | The SI Brochure for the metric symbols; English usage for the names and the other units |
| `ABBREVIATION` | English usage |
| `EMOTICON` | Wikipedia, List of emoticons, the Western ones |

## What was not found

- **Lengths of Slack and Stripe keys.** Neither vendor publishes one. The
  matchers take ten and sixteen characters at the least after the prefix.
- **The alphabet of an AWS access key ID and the form of a Google API
  key.** Neither vendor publishes them. Both are taken from gitleaks, whose
  rules are used widely enough to have been corrected against real keys.
- **The Social Security Administration's own page** on the numbers it
  never assigns refused the request; the rule is taken from Wikipedia's
  summary of it.
- **ISO and 3GPP standards** (IBAN, card numbers, ISBN, IMEI, coordinates)
  are not free to read. Their rules are taken from Wikipedia's summaries.

## What a form cannot say

A matcher checks shape. These have a check of their own that a caller can
run on the token: the Luhn digit of a card number and of an IMEI, the
mod-97 check of an IBAN, the check digit of an ISBN, the checksum in the
case of an Ethereum address, the checksum of a Bitcoin address, and the
checksum in the last six characters of a GitHub token.
