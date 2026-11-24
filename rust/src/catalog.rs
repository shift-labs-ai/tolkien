//! More matchers, to add to the built-in ones.
//!
//! The built-in matchers are what any text holds: words, numbers, dates.
//! The catalog holds what some texts hold and some users look for: network
//! addresses, identifiers, secrets, payment details, personal numbers and
//! the smaller forms of written text. None is in [`Lexer::default`]: each
//! costs a little wherever it can start, and each claims text the built-in
//! matchers would cut differently.
//!
//! ```
//! use tolkien::{catalog, Lexer};
//!
//! let lexer = Lexer::with(catalog::all()).unwrap();
//! let found: Vec<_> = lexer
//!     .tokens("Ping 192.168.1.20 (1d:d8:97:09:54:39) at +1 (555) 010-9999")
//!     .filter(|token| catalog::entries().iter().any(|entry| entry.kind == token.kind))
//!     .map(|token| (token.kind, token.text))
//!     .collect();
//! assert_eq!(
//!     found,
//!     [
//!         ("IPV4", "192.168.1.20"),
//!         ("MAC_ADDRESS", "1d:d8:97:09:54:39"),
//!         ("PHONE_NUMBER", "+1 (555) 010-9999"),
//!     ]
//! );
//! ```
//!
//! Take all of them with [`all`], a [`Group`] with [`group`], or one by
//! its kind with [`named`]. They are given in an order that matters: where
//! two could take the same text, the more specific stands first.
//!
//! A matcher here checks shape, as the built-in ones do. A card number has
//! the digits of one; whether its checksum holds is for the caller to say.

use crate::chars::is_letter_or_digit;
use crate::matchers::{rest_of_address, MONTH_NAMES};
use crate::pattern::{Class, Pattern, Step};
use crate::{Cursor, Matcher, Trie};

/// What the matchers of the catalog are grouped by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// Addresses of machines and of things on a network.
    Network,
    /// Keys, tokens and other credentials.
    Secrets,
    /// Payment and account details.
    Finance,
    /// Numbers that identify a person or a device.
    Personal,
    /// Identifiers and numbers of technical text.
    Technical,
    /// The smaller forms of written text.
    Text,
}

impl Group {
    /// Every group.
    pub const ALL: [Group; 6] = [
        Group::Network,
        Group::Secrets,
        Group::Finance,
        Group::Personal,
        Group::Technical,
        Group::Text,
    ];

    /// The name of the group in lower case, as bindings and files give it.
    pub fn name(self) -> &'static str {
        match self {
            Group::Network => "network",
            Group::Secrets => "secrets",
            Group::Finance => "finance",
            Group::Personal => "personal",
            Group::Technical => "technical",
            Group::Text => "text",
        }
    }
}

/// A matcher of the catalog, and what it is known to do.
pub struct Entry {
    /// The kind of its tokens.
    pub kind: &'static str,
    /// The group it belongs to.
    pub group: Group,
    /// What it matches, in a line.
    pub summary: &'static str,
    /// Where its form is taken from, and where it departs from that.
    pub source: &'static str,
    /// Texts, and the token each starts with.
    pub matches: &'static [(&'static str, &'static str)],
    /// Texts that start with no token of this kind.
    pub rejects: &'static [&'static str],
    build: fn() -> Box<dyn Matcher>,
}

impl Entry {
    /// The matcher.
    pub fn matcher(&self) -> Box<dyn Matcher> {
        (self.build)()
    }
}

/// Every matcher of the catalog, in the order they are to be tried.
pub fn all() -> Vec<Box<dyn Matcher>> {
    entries().iter().map(Entry::matcher).collect()
}

/// The matchers of `group`, in the order they are to be tried.
pub fn group(group: Group) -> Vec<Box<dyn Matcher>> {
    let of_group = entries().iter().filter(|entry| entry.group == group);
    of_group.map(Entry::matcher).collect()
}

/// The matcher whose tokens are of the kind `kind`.
pub fn named(kind: &str) -> Option<Box<dyn Matcher>> {
    let entry = entries().iter().find(|entry| entry.kind == kind);
    entry.map(Entry::matcher)
}

// ── Classes and steps the matchers share ─────────────────────────────

fn digit() -> Class {
    Class::range('0', '9')
}

fn upper() -> Class {
    Class::range('A', 'Z')
}

fn lower() -> Class {
    Class::range('a', 'z')
}

fn hex() -> Class {
    digit() | Class::range('a', 'f') | Class::range('A', 'F')
}

/// ASCII letters and digits.
fn alnum() -> Class {
    upper() | lower() | digit()
}

/// Letters and digits of any script: what a token must not run into.
fn letter_or_digit() -> Class {
    Class::letter() | Class::digit()
}

/// The token is not the start of something longer.
fn end() -> Step {
    Step::not(letter_or_digit())
}

/// As [`end`], for tokens that an underscore or a dash goes on with.
fn end_of_identifier() -> Step {
    Step::not(letter_or_digit() | Class::chars("_-"))
}

fn text(text: &str) -> Step {
    Step::text(text)
}

fn words(words: &[&str]) -> Step {
    Step::words(Trie::from_words(words))
}

fn pattern<const N: usize>(kind: &'static str, steps: [Step; N]) -> Box<dyn Matcher> {
    Box::new(Pattern::new(kind, steps))
}

/// Four numbers from 0 to 255 joined by full stops.
fn ipv4_parts() -> [Step; 7] {
    let part = || Step::number(3, 255);
    [
        part(),
        text("."),
        part(),
        text("."),
        part(),
        text("."),
        part(),
    ]
}

/// A number of degrees up to `most`, with a full stop and at least
/// `fraction` digits after it, and a minus sign before it or not.
fn degrees(most: u64, fraction: usize) -> [Step; 4] {
    [
        Step::optional([text("-")]),
        Step::number(3, most),
        text("."),
        Step::between(fraction, 8, digit()),
    ]
}

/// A number as it is written before a unit: digits, with full stops and
/// commas between them.
fn amount() -> [Step; 2] {
    [
        Step::run(digit()),
        Step::repeat(3, [Step::one(Class::chars(".,")), Step::run(digit())]),
    ]
}

// ── Matchers written by hand ─────────────────────────────────────────

/// An IPv6 address: eight groups of one to four hex digits joined by
/// colons, or fewer with one `::` standing for the groups left out. The
/// last two groups may be written as an IPv4 address: `::ffff:192.0.2.1`.
struct Ipv6 {
    /// The four numbers of an IPv4 address: [`ipv4_parts`].
    ipv4: Pattern,
}

impl Ipv6 {
    fn new() -> Self {
        Self {
            ipv4: Pattern::new(IPV6, ipv4_parts()),
        }
    }

    /// Moves past an IPv4 address, if one stands at the cursor.
    fn ipv4(&self, cur: &mut Cursor<'_>) -> bool {
        let mut ahead = cur.clone();
        let found = self.ipv4.matches(&mut ahead);
        if found {
            cur.set_pos(ahead.pos());
        }
        found
    }
}

impl Matcher for Ipv6 {
    fn kind(&self) -> &'static str {
        IPV6
    }

    fn can_start(&self, first: char) -> bool {
        first.is_ascii_hexdigit() || first == ':'
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        let mut shortened = cur.advance_str("::");
        let mut groups = 0;
        while groups < 8 {
            // An IPv4 address is the last two groups.
            if groups <= 6 && self.ipv4(cur) {
                groups += 2;
                break;
            }
            let digits = cur.advance_while(|c| c.is_ascii_hexdigit());
            if digits == 0 {
                break;
            }
            if digits > 4 {
                return false;
            }
            groups += 1;
            if !shortened && cur.advance_str("::") {
                shortened = true;
            } else if !(cur.followed_by(|c| c == ':', |c| c.is_ascii_hexdigit())
                && cur.advance().is_some())
            {
                break;
            }
        }
        let whole = if shortened {
            (1..=7).contains(&groups)
        } else {
            groups == 8
        };
        whole
            && !cur
                .peek()
                .is_some_and(|c| is_letter_or_digit(c) || c == ':')
            && !cur.followed_by(|c| c == '.', |c| c.is_ascii_digit())
    }
}

/// A telephone number, in one of three forms, with 7 to 15 digits.
///
/// International, after a `+`: `+1 (555) 010-9999`, `+44 20 7946 0958`.
/// With an area code in brackets: `(541) 893-1940`. Or digits alone in
/// groups joined by single dashes or full stops, where a leading zero or
/// two such marks set it apart from a number, and a group of four digits
/// or more from an IP address: `03.3122.2995`, `555-010-9999`. After a
/// leading zero a space joins groups too.
struct PhoneNumber;

impl PhoneNumber {
    /// Digits in groups: after the first, each follows one to `most` of
    /// the marks `joins` accepts. Returns how many digits were read, in how
    /// many groups, and how many the longest group has.
    fn groups(
        cur: &mut Cursor<'_>,
        joins: impl Fn(char) -> bool,
        most: usize,
    ) -> (usize, usize, usize) {
        let mut digits = cur.advance_while(|c| c.is_ascii_digit());
        let mut groups = usize::from(digits > 0);
        let mut longest = digits;
        // A number has few groups: the bound keeps a column of numbers
        // from being read whole at each of them.
        while groups < 7 {
            let mut ahead = cur.clone();
            let mut marks = 0;
            while marks < most && ahead.advance_if(&joins) {
                marks += 1;
            }
            let more = ahead.advance_while(|c| c.is_ascii_digit());
            if (marks == 0 && groups > 0) || more == 0 {
                break;
            }
            cur.set_pos(ahead.pos());
            digits += more;
            groups += 1;
            longest = longest.max(more);
        }
        (digits, groups, longest)
    }
}

impl Matcher for PhoneNumber {
    fn kind(&self) -> &'static str {
        PHONE_NUMBER
    }

    fn can_start(&self, first: char) -> bool {
        first.is_ascii_digit() || first == '+' || first == '('
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        let mark = |c: char| matches!(c, ' ' | '-' | '.');
        let digits = if cur.advance_if(|c| c == '+') {
            let (digits, _, _) = Self::groups(cur, |c| mark(c) || matches!(c, '(' | ')'), 2);
            (7..=15).contains(&digits).then_some(digits)
        } else if cur.advance_if(|c| c == '(') {
            let area = cur.advance_while(|c| c.is_ascii_digit());
            if !(2..=5).contains(&area) || !cur.advance_if(|c| c == ')') {
                return false;
            }
            let (rest, _, _) = Self::groups(cur, mark, 1);
            (7..=15).contains(&(area + rest)).then_some(area + rest)
        } else {
            let trunk = cur.peek() == Some('0');
            let start = cur.pos();
            let (digits, groups, longest) =
                Self::groups(cur, |c| matches!(c, '-' | '.') || (trunk && c == ' '), 1);
            let marks = cur.since(start).matches(['-', '.']).count();
            let set_apart = groups >= 2 && longest >= 4 && (trunk || marks >= 2);
            ((9..=15).contains(&digits) && set_apart).then_some(digits)
        };
        // Not the start of a longer run, of a time or of a date.
        digits.is_some()
            && !cur
                .peek()
                .is_some_and(|c| is_letter_or_digit(c) || matches!(c, ':' | '/'))
            && !cur.followed_by(|c| matches!(c, '-' | '.'), |c| c.is_ascii_digit())
    }
}

/// The schemes of a [`URI`], each with the mark that ends it.
#[rustfmt::skip]
const SCHEMES: &[&str] = &[
    "ftp://", "ftps://", "sftp://", "ssh://", "git://", "file://", "s3://", "gs://", "ws://",
    "wss://", "ldap://", "ldaps://", "redis://", "rediss://", "amqp://", "amqps://", "mysql://",
    "postgres://", "postgresql://", "mongodb://", "mongodb+srv://", "smb://", "nfs://", "irc://",
    "ircs://", "rtsp://", "mailto:", "tel:", "sms:",
];

/// An address with one of [`SCHEMES`]: the scheme, and after it what a
/// [`LIKE_URL`](crate::matchers::LIKE_URL) has after its prefix, which
/// ends as that one does.
struct Uri(Trie);

impl Matcher for Uri {
    fn kind(&self) -> &'static str {
        URI
    }

    fn can_start(&self, first: char) -> bool {
        self.0.can_start(first)
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        // A letter, a digit, a slash or a plus first: `tel: 5` and
        // `mailto:?` are a word and a mark.
        self.0.advance_longest(cur)
            && cur
                .peek()
                .is_some_and(|c| is_letter_or_digit(c) || matches!(c, '/' | '+'))
            && rest_of_address(cur)
    }
}

/// The characters of the data part of a bech32 address (BIP 173): letters
/// and digits without `1`, `b`, `i` and `o`.
const BECH32: &str = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";

/// An ISBN: the groups a [`Pattern`] reads, and the number of digits the
/// form has, which depends on its first group.
struct Isbn(Pattern);

impl Matcher for Isbn {
    fn kind(&self) -> &'static str {
        ISBN
    }

    fn can_start(&self, first: char) -> bool {
        self.0.can_start(first)
    }

    fn matches(&self, cur: &mut Cursor<'_>) -> bool {
        let start = cur.pos();
        if !self.0.matches(cur) {
            return false;
        }
        let token = cur.since(start);
        let characters = token.chars().filter(|c| c.is_ascii_digit() || *c == 'X');
        // Ten in four groups; thirteen in five, or in one.
        let wanted = match token.split(['-', ' ']).count() {
            4 => 10,
            _ => 13,
        };
        characters.count() == wanted
    }
}

// ── The catalog ──────────────────────────────────────────────────────

/// Kind of IPv4 addresses with a prefix length: `10.0.0.0/8`.
pub const CIDR: &str = "CIDR";
/// Kind of IPv4 addresses: `192.168.1.20`.
pub const IPV4: &str = "IPV4";
/// Kind of IPv6 addresses: `2001:db8::1`.
pub const IPV6: &str = "IPV6";
/// Kind of MAC addresses: `1d:d8:97:09:54:39`.
pub const MAC_ADDRESS: &str = "MAC_ADDRESS";
/// Kind of addresses with a scheme other than `http`: `ssh://host/path`.
pub const URI: &str = "URI";
/// Kind of AWS access key IDs.
pub const AWS_ACCESS_KEY_ID: &str = "AWS_ACCESS_KEY_ID";
/// Kind of GitHub tokens.
pub const GITHUB_TOKEN: &str = "GITHUB_TOKEN";
/// Kind of Slack tokens.
pub const SLACK_TOKEN: &str = "SLACK_TOKEN";
/// Kind of Stripe API keys.
pub const STRIPE_KEY: &str = "STRIPE_KEY";
/// Kind of Google API keys.
pub const GOOGLE_API_KEY: &str = "GOOGLE_API_KEY";
/// Kind of npm access tokens.
pub const NPM_TOKEN: &str = "NPM_TOKEN";
/// Kind of JSON Web Tokens.
pub const JWT: &str = "JWT";
/// Kind of the first line of a PEM block: `-----BEGIN PRIVATE KEY-----`.
pub const PEM_HEADER: &str = "PEM_HEADER";
/// Kind of ISBNs written in groups: `978-3-16-148410-0`.
pub const ISBN: &str = "ISBN";
/// Kind of IMEI numbers written with dashes.
pub const IMEI: &str = "IMEI";
/// Kind of payment card numbers.
pub const CARD_NUMBER: &str = "CARD_NUMBER";
/// Kind of international bank account numbers.
pub const IBAN: &str = "IBAN";
/// Kind of Bitcoin addresses.
pub const BITCOIN_ADDRESS: &str = "BITCOIN_ADDRESS";
/// Kind of Ethereum addresses.
pub const ETHEREUM_ADDRESS: &str = "ETHEREUM_ADDRESS";
/// Kind of stock symbols written with a dollar sign: `$AAPL`.
pub const CASHTAG: &str = "CASHTAG";
/// Kind of United States social security numbers.
pub const US_SSN: &str = "US_SSN";
/// Kind of telephone numbers.
pub const PHONE_NUMBER: &str = "PHONE_NUMBER";
/// Kind of pairs of a latitude and a longitude.
pub const COORDINATES: &str = "COORDINATES";
/// Kind of UUIDs.
pub const UUID: &str = "UUID";
/// Kind of digests written in hex: MD5, SHA-1, SHA-2.
pub const HASH: &str = "HASH";
/// Kind of numbers written in hex: `0xFF`.
pub const HEX_NUMBER: &str = "HEX_NUMBER";
/// Kind of version numbers after a `v`: `v1.2.3-beta`.
pub const VERSION: &str = "VERSION";
/// Kind of numbers with an exponent: `6.02e23`.
pub const SCIENTIFIC_NUMBER: &str = "SCIENTIFIC_NUMBER";
/// Kind of colours written in hex: `#ff8800`.
pub const HEX_COLOR: &str = "HEX_COLOR";
/// Kind of file paths that start at the home or the current directory.
pub const FILE_PATH: &str = "FILE_PATH";
/// Kind of Windows file paths: `C:\Users\dana`.
pub const WINDOWS_PATH: &str = "WINDOWS_PATH";
/// Kind of offsets from UTC: `UTC+02:00`.
pub const UTC_OFFSET: &str = "UTC_OFFSET";
/// Kind of ordinal numbers: `22nd`.
pub const ORDINAL: &str = "ORDINAL";
/// Kind of fractions: `3/4`.
pub const FRACTION: &str = "FRACTION";
/// Kind of lengths of time: `90 minutes`.
pub const DURATION: &str = "DURATION";
/// Kind of amounts with a unit: `5 km`.
pub const MEASUREMENT: &str = "MEASUREMENT";
/// Kind of abbreviations written with full stops: `e.g.`.
pub const ABBREVIATION: &str = "ABBREVIATION";
/// Kind of emoticons: `:-)`.
pub const EMOTICON: &str = "EMOTICON";

/// The matchers of the catalog and what each is known to do, in the order
/// they are to be tried.
pub fn entries() -> &'static [Entry] {
    ENTRIES
}

const ENTRIES: &[Entry] = &[
    // Secrets come first: each starts with a text nothing else starts with.
    Entry {
        kind: AWS_ACCESS_KEY_ID,
        group: Group::Secrets,
        summary: "`AKIA` or `ASIA` and sixteen characters of base32: capital letters and the digits 2 to 7",
        source: "AWS IAM User Guide, IAM identifiers: `AKIA` is an access key and `ASIA` a temporary one. AWS does not publish the alphabet; base32 is that of its examples and of the gitleaks rule",
        matches: &[
            ("AKIAIOSFODNN7EXAMPLE", "AKIAIOSFODNN7EXAMPLE"),
            ("ASIAIOSFODNN7EXAMPLE.", "ASIAIOSFODNN7EXAMPLE"),
        ],
        rejects: &[
            "AKIAIOSFODNN7EXAMPL",
            "AKIAIOSFODNN7EXAMPLEX",
            "akiaiosfodnn7example",
            "AKIA0000000000000000",
            "AIDAIOSFODNN7EXAMPLE",
        ],
        build: || {
            pattern(
                AWS_ACCESS_KEY_ID,
                [
                    Step::either([[text("AKIA")], [text("ASIA")]]),
                    Step::exactly(16, upper() | Class::range('2', '7')),
                    end(),
                ],
            )
        },
    },
    Entry {
        kind: GITHUB_TOKEN,
        group: Group::Secrets,
        summary: "`ghp_`, `gho_`, `ghu_`, `ghs_` or `ghr_` and 36 or more letters and digits, or `github_pat_` and 82 or more with underscores",
        source: "GitHub Docs, About authentication to GitHub, for the prefixes; GitHub Blog, Behind GitHub's new authentication token formats (2021), for the 36 characters. The 82 after `github_pat_` are from the gitleaks rule, not from GitHub",
        matches: &[
            (
                "ghp_abcdefghijklmnopqrstuvwxyzABCDEFGHIJ",
                "ghp_abcdefghijklmnopqrstuvwxyzABCDEFGHIJ",
            ),
            (
                "github_pat_11ABCDEFG0abcdefghijkl_0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVW ",
                "github_pat_11ABCDEFG0abcdefghijkl_0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVW",
            ),
        ],
        rejects: &[
            "ghp_short",
            "ghx_abcdefghijklmnopqrstuvwxyzABCDEFGHIJ",
            "github_pat_short",
            "github_pat_11ABCDEFG0abcdefghijkl_mnopqrstuvwxyz0123456789",
        ],
        build: || {
            pattern(
                GITHUB_TOKEN,
                [
                    Step::either([
                        vec![
                            words(&["ghp_", "gho_", "ghu_", "ghs_", "ghr_"]),
                            Step::between(36, 255, alnum()),
                        ],
                        vec![
                            text("github_pat_"),
                            Step::between(82, 255, alnum() | Class::chars("_")),
                        ],
                    ]),
                    end_of_identifier(),
                ],
            )
        },
    },
    Entry {
        kind: SLACK_TOKEN,
        group: Group::Secrets,
        summary: "`xoxb-`, `xoxp-`, `xapp-`, `xwfp-`, an older `xox` prefix or a rotating `xoxe` one, and ten or more letters, digits and dashes",
        source: "Slack developer docs, Tokens: `xoxb-`, `xoxp-`, `xapp-`, `xwfp-`, `xoxe.xapp-`. The older and rotating prefixes are from the gitleaks rules. Slack publishes no length",
        matches: &[
            (
                "xoxb-123456789012-abcdefABCDEF,",
                "xoxb-123456789012-abcdefABCDEF",
            ),
            ("xapp-1-A0123456789-0123456789-abcdef", "xapp-1-A0123456789-0123456789-abcdef"),
            ("xoxe.xoxp-1-ABCDEFGHIJKL", "xoxe.xoxp-1-ABCDEFGHIJKL"),
        ],
        rejects: &["xoxb-short", "xoxz-123456789012-abcdef"],
        build: || {
            pattern(
                SLACK_TOKEN,
                [
                    words(&[
                        "xoxb-", "xoxp-", "xoxa-", "xoxr-", "xoxs-", "xoxo-", "xoxe-", "xapp-",
                        "xwfp-", "xoxe.xoxb-", "xoxe.xoxp-", "xoxe.xapp-",
                    ]),
                    Step::between(10, 250, alnum() | Class::chars("-")),
                    end_of_identifier(),
                ],
            )
        },
    },
    Entry {
        kind: STRIPE_KEY,
        group: Group::Secrets,
        summary: "`sk_`, `pk_` or `rk_`, then `live_` or `test_`, and sixteen or more letters and digits",
        source: "Stripe Docs, API keys: secret, publishable and restricted keys in test and live mode. Stripe publishes no length",
        matches: &[("sk_live_abcdefghijklmnop1234", "sk_live_abcdefghijklmnop1234")],
        rejects: &["sk_live_short", "sk_prod_abcdefghijklmnop1234"],
        build: || {
            pattern(
                STRIPE_KEY,
                [
                    words(&[
                        "sk_live_", "sk_test_", "pk_live_", "pk_test_", "rk_live_", "rk_test_",
                    ]),
                    Step::between(16, 247, alnum()),
                    end_of_identifier(),
                ],
            )
        },
    },
    Entry {
        kind: GOOGLE_API_KEY,
        group: Group::Secrets,
        summary: "`AIza` and 35 letters, digits, dashes and underscores",
        source: "Not published by Google: the gitleaks rule `gcp-api-key`",
        matches: &[(
            "AIzaSyA-abcdefghijklmnopqrstuvwxyz01234",
            "AIzaSyA-abcdefghijklmnopqrstuvwxyz01234",
        )],
        rejects: &["AIzaSyA-short", "AIzaSyA-abcdefghijklmnopqrstuvwxyz012345"],
        build: || {
            pattern(
                GOOGLE_API_KEY,
                [
                    text("AIza"),
                    Step::exactly(35, alnum() | Class::chars("_-")),
                    end_of_identifier(),
                ],
            )
        },
    },
    Entry {
        kind: NPM_TOKEN,
        group: Group::Secrets,
        summary: "`npm_` and 36 letters and digits",
        source: "GitHub Changelog, npm has a new access token format (2021): the prefix, and the format of GitHub's tokens",
        matches: &[(
            "npm_abcdefghijklmnopqrstuvwxyz0123456789",
            "npm_abcdefghijklmnopqrstuvwxyz0123456789",
        )],
        rejects: &["npm_install", "npm_abcdefghijklmnopqrstuvwxyz01234567890"],
        build: || {
            pattern(
                NPM_TOKEN,
                [text("npm_"), Step::exactly(36, alnum()), end_of_identifier()],
            )
        },
    },
    Entry {
        kind: JWT,
        group: Group::Secrets,
        summary: "Three parts of base64url letters joined by full stops, the first two starting with `eyJ`; the third, the signature, may be empty",
        source: "RFC 7519 and RFC 7515: three base64url parts; a JSON object in base64url starts with `eyJ`; an unsecured JWT has an empty signature (RFC 7519, section 6)",
        matches: &[
            (
                "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl-_x and",
                "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl-_x",
            ),
            ("eyJhbGciOiJub25lIn0.eyJzdWIiOiIxIn0. x", "eyJhbGciOiJub25lIn0.eyJzdWIiOiIxIn0."),
        ],
        rejects: &["eyJhbGciOiJIUzI1NiJ9", "eyJhbGciOiJIUzI1NiJ9.payload.signature"],
        build: || {
            let base64url = || alnum() | Class::chars("_-");
            pattern(
                JWT,
                [
                    text("eyJ"),
                    Step::run(base64url()),
                    text(".eyJ"),
                    Step::run(base64url()),
                    text("."),
                    Step::between(0, usize::MAX, base64url()),
                    end_of_identifier(),
                ],
            )
        },
    },
    Entry {
        kind: PEM_HEADER,
        group: Group::Secrets,
        summary: "`-----BEGIN`, a label of up to eight words in capitals and digits with one space between them, `-----`",
        source: "RFC 7468, section 3: a label does not start or end with a space, and holds no two together. The standard allows any printable character but a hyphen in it, and a hyphen between words; the labels in use are words in capitals and digits",
        matches: &[
            ("-----BEGIN PRIVATE KEY-----", "-----BEGIN PRIVATE KEY-----"),
            (
                "-----BEGIN RSA PRIVATE KEY-----\nMII",
                "-----BEGIN RSA PRIVATE KEY-----",
            ),
            ("-----BEGIN X509 CRL-----", "-----BEGIN X509 CRL-----"),
        ],
        rejects: &[
            "-----BEGIN private key-----",
            "----BEGIN PRIVATE KEY-----",
            "-----BEGIN -",
            "-----BEGIN  KEY-----",
            "-----BEGIN KEY -----",
            "-----BEGIN RSA  KEY-----",
        ],
        build: || {
            pattern(
                PEM_HEADER,
                [
                    text("-----BEGIN "),
                    Step::between(1, 32, upper() | digit()),
                    Step::repeat(7, [text(" "), Step::between(1, 32, upper() | digit())]),
                    text("-----"),
                ],
            )
        },
    },
    // Network.
    Entry {
        kind: CIDR,
        group: Group::Network,
        summary: "An IPv4 address, a slash and a prefix length up to 32",
        source: "RFC 4632, section 3.1: a prefix length from 0 to 32",
        matches: &[("10.0.0.0/8", "10.0.0.0/8"), ("192.168.1.0/24.", "192.168.1.0/24")],
        rejects: &["10.0.0.0/33", "10.0.0.0", "10.0.0.0/8a", "10.0.0/8"],
        build: || {
            let [a, b, c, d, e, f, g] = ipv4_parts();
            pattern(CIDR, [a, b, c, d, e, f, g, text("/"), Step::number(2, 32), end()])
        },
    },
    Entry {
        kind: IPV4,
        group: Group::Network,
        summary: "Four numbers from 0 to 255 joined by full stops",
        source: "RFC 3986, section 3.2.2, `IPv4address`",
        matches: &[
            ("192.168.1.20", "192.168.1.20"),
            ("8.8.8.8, and", "8.8.8.8"),
            ("127.0.0.1.", "127.0.0.1"),
        ],
        rejects: &["256.1.1.1", "1.2.3", "1.2.3.4.5", "1.2.3.4a", "17.02.2009"],
        build: || {
            let [a, b, c, d, e, f, g] = ipv4_parts();
            let longer = Step::not_before([text("."), Step::one(digit())]);
            pattern(IPV4, [a, b, c, d, e, f, g, longer, end()])
        },
    },
    Entry {
        kind: IPV6,
        group: Group::Network,
        summary: "Eight groups of hex digits joined by colons, or fewer around one `::`; the last two may be an IPv4 address",
        source: "RFC 4291, section 2.2. A zone after `%` (RFC 4007) is not read",
        matches: &[
            (
                "cc87:f687:8bdb:8a6c:ffac:0910:1bde:3a63",
                "cc87:f687:8bdb:8a6c:ffac:0910:1bde:3a63",
            ),
            ("2001:db8::1.", "2001:db8::1"),
            ("::1", "::1"),
            ("fe80::", "fe80::"),
            ("::ffff:192.168.1.20,", "::ffff:192.168.1.20"),
            ("64:ff9b::192.0.2.33", "64:ff9b::192.0.2.33"),
            ("0:0:0:0:0:ffff:10.0.0.1", "0:0:0:0:0:ffff:10.0.0.1"),
        ],
        rejects: &[
            "1d:d8:97:09:54:39",
            "10:30",
            "::",
            "1:2:3:4:5:6:7:8:9",
            "12345::1",
            "2001:db8::1::2",
            "std::vector",
            "::ffff:192.168.1.256",
            "::ffff:1.2.3.4.5",
            "1:2:3:4:5:6:7:1.2.3.4",
        ],
        build: || Box::new(Ipv6::new()),
    },
    Entry {
        kind: MAC_ADDRESS,
        group: Group::Network,
        summary: "Six pairs of hex digits joined by the same colon or dash, or three groups of four joined by full stops with a letter among them",
        source: "IEEE 802 as summarised in Wikipedia, MAC address: hyphens or colons between pairs, or the three groups of four that Cisco writes. Twelve hex digits with no separator are not taken",
        matches: &[
            ("1d:d8:97:09:54:39", "1d:d8:97:09:54:39"),
            ("1D-D8-97-09-54-39 ", "1D-D8-97-09-54-39"),
            ("0123.4567.89ab.", "0123.4567.89ab"),
        ],
        rejects: &[
            "1d:d8-97:09:54:39",
            "1d:d8:97:09:54",
            "1d:d8:97:09:54:3g",
            "1d:d8:97:09:54:39a",
            "0123.4567.8901",
            "0123.4567.89abc",
            "0123.4567",
        ],
        build: || {
            let pair = || Step::exactly(2, hex());
            let again = Step::same_separator;
            let four = |class: fn() -> Class| Step::exactly(4, class());
            let dotted = |class| [four(class), text("."), four(class), text("."), four(class)];
            let mut in_fours = vec![
                // Twelve digits are more often a number than an address.
                Step::not_before(dotted(digit)),
            ];
            in_fours.extend(dotted(hex));
            pattern(
                MAC_ADDRESS,
                [
                    Step::either([
                        vec![
                            pair(),
                            Step::separator(Class::chars(":-")),
                            pair(),
                            again(),
                            pair(),
                            again(),
                            pair(),
                            again(),
                            pair(),
                            again(),
                            pair(),
                        ],
                        in_fours,
                    ]),
                    end(),
                ],
            )
        },
    },
    Entry {
        kind: URI,
        group: Group::Network,
        summary: "A scheme other than `http` and what follows: `ssh://`, `ftp://`, `s3://`, `postgres://`, `mailto:`, `tel:` and others; it ends as a `LIKE_URL` does",
        source: "RFC 3986 for the characters; the IANA registry of URI schemes for the schemes. `ftps`, `s3`, `gs`, `amqp`, `amqps`, `mysql`, `postgres`, `postgresql` and `mongodb+srv` are not registered and are taken because they are in use",
        matches: &[
            ("ssh://git@host.example/repo.git.", "ssh://git@host.example/repo.git"),
            ("mailto:dana@acme.com, or", "mailto:dana@acme.com"),
            ("s3://bucket/key?x=1", "s3://bucket/key?x=1"),
            ("postgres://user:pw@db:5432/app)", "postgres://user:pw@db:5432/app"),
            ("file:///etc/hosts", "file:///etc/hosts"),
            ("s3://bucket/a_(b)) and", "s3://bucket/a_(b)"),
            ("tel:+15550109999.", "tel:+15550109999"),
        ],
        rejects: &["ssh://", "mailto:", "http://example.com", "ssh:/x", "tel: 5", "mailto:?", "ssh://)."],
        build: || Box::new(Uri(Trie::from_words(SCHEMES))),
    },
    // Technical, and the address that starts as a hex number does.
    Entry {
        kind: UUID,
        group: Group::Technical,
        summary: "Hex digits in groups of 8, 4, 4, 4 and 12 joined by dashes",
        source: "RFC 9562, section 4",
        matches: &[(
            "123e4567-e89b-12d3-a456-426614174000.",
            "123e4567-e89b-12d3-a456-426614174000",
        )],
        rejects: &[
            "123e4567-e89b-12d3-a456-42661417400",
            "123e4567-e89b-12d3-a456-4266141740000",
            "123e4567e89b12d3a456426614174000x",
        ],
        build: || {
            let of = |count| Step::exactly(count, hex());
            pattern(
                UUID,
                [of(8), text("-"), of(4), text("-"), of(4), text("-"), of(4), text("-"), of(12), end()],
            )
        },
    },
    Entry {
        kind: ETHEREUM_ADDRESS,
        group: Group::Finance,
        summary: "`0x` and forty hex digits",
        source: "ERC-55: `0x` and 40 hex digits in any case. The checksum in the case of the letters is not checked",
        matches: &[(
            "0xe1dcd20433acf4060193a3df869fc93a5d656d7f",
            "0xe1dcd20433acf4060193a3df869fc93a5d656d7f",
        )],
        rejects: &[
            "0xe1dcd20433acf4060193a3df869fc93a5d656d7",
            "0xe1dcd20433acf4060193a3df869fc93a5d656d7f0",
        ],
        build: || pattern(ETHEREUM_ADDRESS, [text("0x"), Step::exactly(40, hex()), end()]),
    },
    Entry {
        kind: HASH,
        group: Group::Technical,
        summary: "32, 40, 56, 64, 96 or 128 hex digits: an MD5, SHA-1 or SHA-2 digest",
        source: "The digest lengths of MD5 (RFC 1321) and of SHA-1 and SHA-2 (FIPS 180-4), in hex",
        matches: &[
            ("d41d8cd98f00b204e9800998ecf8427e", "d41d8cd98f00b204e9800998ecf8427e"),
            (
                "da39a3ee5e6b4b0d3255bfef95601890afd80709 file",
                "da39a3ee5e6b4b0d3255bfef95601890afd80709",
            ),
            (
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
        ],
        rejects: &[
            "d41d8cd98f00b204e9800998ecf8427",
            "d41d8cd98f00b204e9800998ecf8427e0",
            "d41d8cd98f00b204e9800998ecf8427g",
        ],
        build: || {
            let of = |count| vec![Step::exactly(count, hex()), end()];
            pattern(
                HASH,
                [Step::either([of(128), of(96), of(64), of(56), of(40), of(32)])],
            )
        },
    },
    Entry {
        kind: HEX_NUMBER,
        group: Group::Technical,
        summary: "`0x` and hex digits",
        source: "The `0x` literal of C and of the languages after it",
        matches: &[("0xFF", "0xFF"), ("0x1f4a,", "0x1f4a"), ("0XDEADBEEF", "0XDEADBEEF")],
        rejects: &["0x", "0xZZ", "0x1g", "x1F"],
        build: || {
            pattern(
                HEX_NUMBER,
                [
                    Step::either([[text("0x")], [text("0X")]]),
                    Step::run(hex()),
                    end(),
                ],
            )
        },
    },
    Entry {
        kind: VERSION,
        group: Group::Technical,
        summary: "`v`, numbers joined by full stops, and a pre-release or build after a dash or a plus",
        source: "Semantic Versioning 2.0.0, items 9 and 10, with the `v` that tags carry. Two to four numbers are taken, where SemVer has three",
        matches: &[
            ("v1.2.3", "v1.2.3"),
            ("v2.0.", "v2.0"),
            ("V10.4.1-beta.2+build5 is", "V10.4.1-beta.2+build5"),
            ("v1.2.3.4", "v1.2.3.4"),
            ("v1.0.0-x-y.7-", "v1.0.0-x-y.7"),
        ],
        rejects: &["v1", "v1.", "version", "v1.2x", "1.2.3"],
        build: || {
            let part = || [text("."), Step::run(digit())];
            let [dot, number] = part();
            pattern(
                VERSION,
                [
                    Step::one(Class::chars("vV")),
                    Step::run(digit()),
                    dot,
                    number,
                    Step::repeat(2, part()),
                    // A letter or a digit first: the marks left out at
                    // the end then never reach back to the first one.
                    Step::repeat(
                        2,
                        [
                            Step::one(Class::chars("-+")),
                            Step::one(alnum()),
                            Step::between(0, 31, alnum() | Class::chars(".-")),
                        ],
                    ),
                    Step::trim_end(Class::chars(".-")),
                    end(),
                ],
            )
        },
    },
    Entry {
        kind: SCIENTIFIC_NUMBER,
        group: Group::Technical,
        summary: "A number, `e` or `E`, and an exponent",
        source: "The E notation of programming languages and of IEEE 754 decimal strings",
        matches: &[("6.02e23", "6.02e23"), ("1E-9,", "1E-9"), ("3e+8 m", "3e+8")],
        rejects: &["6.02e", "e23", "1e2x", "3e8a", "12"],
        build: || {
            pattern(
                SCIENTIFIC_NUMBER,
                [
                    Step::run(digit()),
                    Step::optional([text("."), Step::run(digit())]),
                    Step::one(Class::chars("eE")),
                    Step::optional([Step::one(Class::chars("+-"))]),
                    Step::run(digit()),
                    end(),
                ],
            )
        },
    },
    Entry {
        kind: HEX_COLOR,
        group: Group::Technical,
        summary: "`#` and 3, 4, 6 or 8 hex digits; without a letter among them, 6 only. A hashtag of those letters is one too: `#decade`",
        source: "CSS Color Module Level 4, the hex notations. Three, four or eight digits with no letter are left out, a departure: they are more often the number of an issue or of an account",
        matches: &[
            ("#fff", "#fff"),
            ("#FF8800;", "#FF8800"),
            ("#ff880080", "#ff880080"),
            ("#000000", "#000000"),
        ],
        rejects: &["#ff", "#fffff", "#ggg", "#fffg", "#tag", "#516", "#1234", "#53901976"],
        build: || {
            let of = |count| vec![Step::exactly(count, hex()), end_of_identifier()];
            pattern(
                HEX_COLOR,
                [
                    text("#"),
                    // Digits alone are the number of an issue or of an
                    // account, unless there are six of them.
                    Step::not_before([Step::between(3, 4, digit()), end_of_identifier()]),
                    Step::not_before([Step::exactly(8, digit()), end_of_identifier()]),
                    Step::either([of(8), of(6), of(4), of(3)]),
                ],
            )
        },
    },
    Entry {
        kind: FILE_PATH,
        group: Group::Technical,
        summary: "A path that starts with `~/` or `../`, or with `./` and holds a second slash or a full stop",
        source: "POSIX path notation. An absolute path is not taken: `/` starts too much else. Nor is `./configure`: after `./` a second slash or a full stop is asked for, since `Mr./Ms` is not a path",
        matches: &[
            ("~/Projects/tolkien/README.md.", "~/Projects/tolkien/README.md"),
            ("./src/main.rs:12", "./src/main.rs"),
            ("../docs/a-b_c/x.txt, and", "../docs/a-b_c/x.txt"),
            ("~/.ssh/config", "~/.ssh/config"),
            ("~/ab", "~/ab"),
            ("./a.out", "./a.out"),
            ("./.env,", "./.env"),
        ],
        rejects: &[
            "~/", "./ x", ".../x", "/usr/bin", "~user", "./././", "./..", "./-", "./C", "./C. 6608",
            "./Mr", "./Mr./Ms", "./configure", "~/a",
        ],
        build: || {
            let name = || letter_or_digit() | Class::chars("_~+@");
            let rest = || Step::between(0, usize::MAX, name() | Class::chars("/.-"));
            pattern(
                FILE_PATH,
                [
                    Step::either([
                        // A hidden name starts with one full stop. What
                        // comes after it is not a mark that is left out at
                        // the end, so leaving marks out never undoes the
                        // path.
                        vec![
                            words(&["~/", "../"]),
                            Step::optional([text(".")]),
                            Step::exactly(2, name()),
                            rest(),
                        ],
                        // `./` also stands between two abbreviations, as
                        // in `W./C.` and `Mr./Ms`: a second slash or a
                        // full stop with a name after it tells a path.
                        vec![
                            text("./"),
                            Step::either([
                                vec![text("."), Step::one(name())],
                                vec![
                                    Step::run(name() | Class::chars("-")),
                                    Step::one(Class::chars("/.")),
                                    Step::one(name()),
                                ],
                            ]),
                            rest(),
                        ],
                    ]),
                    Step::trim_end(Class::chars(".-/")),
                ],
            )
        },
    },
    Entry {
        kind: WINDOWS_PATH,
        group: Group::Technical,
        summary: "A drive letter, `:\\` and a path of names without spaces",
        source: "Microsoft Learn, Naming Files, Paths, and Namespaces. A name may hold spaces and more marks than are taken here",
        matches: &[
            ("C:\\Users\\dana\\notes.txt.", "C:\\Users\\dana\\notes.txt"),
            ("d:\\x and", "d:\\x"),
        ],
        rejects: &["C:\\", "C:/Users", "CC:\\x", "C:\\ x", "C:\\..", "C:\\\\x"],
        build: || {
            let name = || letter_or_digit() | Class::chars("_~+@$");
            pattern(
                WINDOWS_PATH,
                [
                    Step::one(upper() | lower()),
                    text(":\\"),
                    Step::one(name()),
                    Step::between(0, usize::MAX, name() | Class::chars("\\.-")),
                    Step::trim_end(Class::chars(".-\\")),
                ],
            )
        },
    },
    Entry {
        kind: UTC_OFFSET,
        group: Group::Technical,
        summary: "`UTC` or `GMT`, then `+` and hours up to 14 or `-` and hours up to 12, and `:00`, `:30` or `:45` or not",
        source: "The offsets in use, from UTC-12:00 to UTC+14:00 on the hour or at 30 or 45 minutes (Wikipedia, List of UTC offsets)",
        matches: &[
            ("UTC+2", "UTC+2"),
            ("GMT-05:00.", "GMT-05:00"),
            ("UTC+14 and", "UTC+14"),
            ("UTC+05:45", "UTC+05:45"),
            ("UTC+2:15", "UTC+2"),
        ],
        rejects: &["UTC", "UTC+15", "UTC-13", "UTC+2x", "utc+2", "EST+2"],
        build: || {
            pattern(
                UTC_OFFSET,
                [
                    words(&["UTC", "GMT"]),
                    Step::either([
                        [text("+"), Step::number(2, 14)],
                        [text("-"), Step::number(2, 12)],
                    ]),
                    Step::optional([text(":"), words(&["00", "30", "45"])]),
                    end(),
                ],
            )
        },
    },
    // Numbers in groups, the most specific first: an ISBN and an IMEI have
    // the digits of a card number, and an ISBN and a social security
    // number those of a telephone number.
    Entry {
        kind: ISBN,
        group: Group::Technical,
        summary: "The number of a book: 13 digits that start with `978` or `979`, together or in five groups, or 10 in four groups, of which the last may be an `X`",
        source: "The ISBN Users' Manual as summarised in Wikipedia, ISBN: 13 digits in five parts after `978` or `979`, or 10 in four parts with a check character that may be `X`",
        matches: &[
            ("978-3-931911-18-8", "978-3-931911-18-8"),
            ("979 10 90636 07 1.", "979 10 90636 07 1"),
            ("3-922296-21-1", "3-922296-21-1"),
            ("0-8044-2957-X,", "0-8044-2957-X"),
            ("9786164510432", "9786164510432"),
        ],
        rejects: &[
            "978-3-931911-18",
            "978-3-931911-18-88",
            "977-3-931911-18-8",
            "3-922296-21",
            "3-92229-21-1",
            "555-010-9999",
            "978-3 931911-18-8",
            "97861645104321",
            "9776164510432",
        ],
        build: || {
            let again = Step::same_separator;
            let groups = Pattern::new(
                ISBN,
                [Step::either([
                    vec![
                        Step::either([[text("978")], [text("979")]]),
                        Step::separator(Class::chars("- ")),
                        Step::between(1, 5, digit()),
                        again(),
                        Step::between(1, 7, digit()),
                        again(),
                        Step::between(1, 6, digit()),
                        again(),
                        Step::one(digit()),
                        end(),
                        Step::not_before([Step::between(0, 9, digit()), text("X")]),
                    ],
                    vec![
                        Step::either([[text("978")], [text("979")]]),
                        Step::exactly(10, digit()),
                        end(),
                    ],
                    vec![
                        Step::between(1, 5, digit()),
                        Step::separator(Class::chars("- ")),
                        Step::between(1, 7, digit()),
                        again(),
                        Step::between(1, 6, digit()),
                        again(),
                        Step::one(digit() | Class::chars("X")),
                        end(),
                    ],
                ])],
            );
            Box::new(Isbn(groups))
        },
    },
    Entry {
        kind: IMEI,
        group: Group::Personal,
        summary: "Digits in groups of 2, 6, 6 and 1 joined by dashes, or with a last group of 2 for the number with a software version",
        source: "3GPP TS 23.003 as summarised in Wikipedia, International Mobile Equipment Identity: `AA-BBBBBB-CCCCCC-D`, and `-EE` for an IMEISV",
        matches: &[
            ("39-650908-828872-7", "39-650908-828872-7"),
            ("39-650908-828872-77.", "39-650908-828872-77"),
        ],
        rejects: &["39-650908-828872", "39-650908-828872-777", "39-65090-828872-7"],
        build: || {
            let of = |count| Step::exactly(count, digit());
            let last = Step::between(1, 2, digit());
            pattern(
                IMEI,
                [of(2), text("-"), of(6), text("-"), of(6), text("-"), last, end()],
            )
        },
    },
    Entry {
        kind: US_SSN,
        group: Group::Personal,
        summary: "Digits in groups of 3, 2 and 4 joined by the same dash or space, without the numbers that are never assigned: a first group of 000, 666 or 900 to 999, a group of zeros",
        source: "Social Security Administration, as summarised in Wikipedia, Social Security number: `AAA-GG-SSSS` and the numbers never assigned",
        matches: &[("543-21-6046", "543-21-6046"), ("838 44 5162.", "838 44 5162")],
        rejects: &[
            "543-21-604",
            "543-21-60466",
            "543-21 6046",
            "5432-1-6046",
            "543-21-6046-1",
            "000-21-6046",
            "666-21-6046",
            "900-21-6046",
            "543-00-6046",
            "543-21-0000",
        ],
        build: || {
            let of = |count| Step::exactly(count, digit());
            pattern(
                US_SSN,
                [
                    Step::not_before([words(&["000", "666", "9"])]),
                    of(3),
                    Step::separator(Class::chars("- ")),
                    Step::not_before([text("00")]),
                    of(2),
                    Step::same_separator(),
                    Step::not_before([text("0000")]),
                    of(4),
                    Step::not_before([text("-"), Step::one(digit())]),
                    end(),
                ],
            )
        },
    },
    Entry {
        kind: CARD_NUMBER,
        group: Group::Finance,
        summary: "13 to 19 digits, in up to five groups joined by single spaces or dashes, the first of four digits or more",
        source: "ISO/IEC 7812 as summarised in Wikipedia, Payment card number: 8 to 19 digits. 13 to 19 are taken, the lengths the card networks issue",
        matches: &[
            ("4111111111111111", "4111111111111111"),
            ("4111 1111 1111 1111.", "4111 1111 1111 1111"),
            ("3782-822463-10005 x", "3782-822463-10005"),
        ],
        rejects: &[
            "4111 1111 1111",
            "4111111111111111x",
            "41111111111111111111",
            "4111  1111 1111 1111",
            "978-3-931911-18-8",
            "1 22 333 4444 55555",
        ],
        build: || {
            Box::new(
                Pattern::new(
                    CARD_NUMBER,
                    [
                        Step::between(4, 19, digit()),
                        Step::repeat(4, [Step::one(Class::chars(" -")), Step::run(digit())]),
                        end(),
                    ],
                )
                .with_count(digit(), 13, 19),
            )
        },
    },
    Entry {
        kind: IBAN,
        group: Group::Finance,
        summary: "Two capital letters, two digits, and 11 to 30 capital letters and digits, together or in groups of up to four",
        source: "ISO 13616 as summarised in Wikipedia, International Bank Account Number: up to 34 characters, 15 at the least, printed in groups of four",
        matches: &[
            ("GB29NWBK60161331926819", "GB29NWBK60161331926819"),
            ("GB29 NWBK 6016 1331 9268 19.", "GB29 NWBK 6016 1331 9268 19"),
            ("KZ17877780479W8988YS,", "KZ17877780479W8988YS"),
        ],
        rejects: &["GB29NWBK6016", "gb29nwbk60161331926819", "G129NWBK60161331926819", "GB29 NWBK 6016"],
        build: || {
            let body = || upper() | digit();
            Box::new(
                Pattern::new(
                    IBAN,
                    [
                        Step::exactly(2, upper()),
                        Step::exactly(2, digit()),
                        Step::either([
                            vec![Step::between(11, 30, body())],
                            vec![Step::repeat(
                                8,
                                [text(" "), Step::between(1, 4, body())],
                            )],
                        ]),
                        end(),
                    ],
                )
                .with_count(body(), 15, 34),
            )
        },
    },
    Entry {
        kind: BITCOIN_ADDRESS,
        group: Group::Finance,
        summary: "`1` or `3` and 25 to 34 base58 characters, or `bc1` and 11 to 71 characters of the bech32 alphabet, in one case",
        source: "Bitcoin Wiki, Invoice address, for the 26 to 35 base58 characters; BIP 173 and BIP 350 for bech32",
        matches: &[
            ("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"),
            ("3dSL29y3ps7zv5AkQc5FSgCNS84S.", "3dSL29y3ps7zv5AkQc5FSgCNS84S"),
            (
                "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
                "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            ),
            (
                "BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4.",
                "BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4",
            ),
        ],
        rejects: &[
            "1A1zP1eP5QGefi2DMPTf",
            "2A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
            "1A1zP1eP5QGefi2DMPTfTL5SLmv7Divf0l",
            "bc1short",
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdb",
            "bc1QAR0SRRR7XFKVY5L643LYDNW9RE59GTZZWF5MDQ",
        ],
        build: || {
            let base58 =
                Class::chars("123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz");
            pattern(
                BITCOIN_ADDRESS,
                [
                    Step::either([
                        vec![Step::one(Class::chars("13")), Step::between(25, 34, base58)],
                        vec![
                            text("bc1"),
                            Step::between(11, 71, Class::chars(BECH32)),
                        ],
                        vec![
                            text("BC1"),
                            Step::between(11, 71, Class::chars(&BECH32.to_uppercase())),
                        ],
                    ]),
                    end(),
                ],
            )
        },
    },
    Entry {
        kind: CASHTAG,
        group: Group::Finance,
        summary: "`$` and one to six capital letters, then a full stop or an underscore and one or two more, or not",
        source: "twitter-text, the cashtag rule. Twitter takes any case; capitals alone are taken here, since `$path` is more often a variable",
        matches: &[("$AAPL", "$AAPL"), ("$T,", "$T"), ("$BRK.A", "$BRK.A"), ("$GOOGL.", "$GOOGL")],
        rejects: &["$5", "$aapl", "$TOOLONG", "$AAPL1", "$BRK.ABC"],
        build: || {
            pattern(
                CASHTAG,
                [
                    text("$"),
                    Step::between(1, 6, upper()),
                    Step::optional([Step::one(Class::chars("._")), Step::between(1, 2, upper())]),
                    end_of_identifier(),
                ],
            )
        },
    },
    Entry {
        kind: COORDINATES,
        group: Group::Personal,
        summary: "A latitude up to 90 and a longitude up to 180 in decimal degrees with a comma between them: in square brackets, or with four or more decimals each",
        source: "ISO 6709, decimal degrees. The brackets and the count of decimals are what sets a pair apart from two numbers",
        matches: &[
            ("[39.6601,-36.8189]", "[39.6601,-36.8189]"),
            ("[43.6,75.94]", "[43.6,75.94]"),
            ("40.7128, -74.0060.", "40.7128, -74.0060"),
        ],
        rejects: &[
            "3.14, 2.72",
            "[39.6601,-36.8189",
            "40.7128,-74.00",
            "1234.5678, 1.2345",
            "91.0000, 10.0000",
            "10.0000, 181.0000",
        ],
        build: || {
            let pair = |fraction| {
                let [a, b, c, d] = degrees(90, fraction);
                let [e, f, g, h] = degrees(180, fraction);
                vec![a, b, c, d, text(","), Step::optional([text(" ")]), e, f, g, h]
            };
            let mut bracketed = vec![text("[")];
            bracketed.extend(pair(1));
            bracketed.push(text("]"));
            let mut bare = pair(4);
            bare.push(Step::not(digit()));
            pattern(COORDINATES, [Step::either([bracketed, bare])])
        },
    },
    Entry {
        kind: PHONE_NUMBER,
        group: Group::Personal,
        summary: "7 to 15 digits after a `+`, after an area code in brackets, or in groups joined by dashes or full stops",
        source: "ITU-T E.164 for the 15 digits at the most. The grouping is usage, not a standard",
        matches: &[
            ("+1 (555) 010-9999", "+1 (555) 010-9999"),
            ("+44 20 7946 0958.", "+44 20 7946 0958"),
            ("+543.242-863.5663", "+543.242-863.5663"),
            ("(541) 893-1940", "(541) 893-1940"),
            ("(541)-8931940", "(541)-8931940"),
            ("555-010-9999", "555-010-9999"),
            ("03.3122.2995", "03.3122.2995"),
            ("0178 67860239 x", "0178 67860239"),
        ],
        rejects: &[
            "+1 555",
            "(541)",
            "17.02.2009",
            "2009-02-17",
            "1,000.50",
            "12345.6789",
            "01.02.2009 10:30",
            "+1234567890123456",
            "192.168.1.20",
            "(2009) and",
        ],
        build: || Box::new(PhoneNumber),
    },
    // Text.
    Entry {
        kind: ORDINAL,
        group: Group::Text,
        summary: "Digits and `st`, `nd`, `rd` or `th`, unless a month name follows",
        source: "English usage",
        matches: &[("1st", "1st"), ("22nd place", "22nd"), ("103RD.", "103RD")],
        rejects: &["1", "st", "1sts", "17th Feb 2009", "3rd, May 2009", "12345th"],
        build: || {
            pattern(
                ORDINAL,
                [
                    Step::between(1, 4, digit()),
                    Step::words(Trie::from_words_in_common_cases(["st", "nd", "rd", "th"])),
                    end(),
                    // A date takes a day with its ending.
                    Step::not_before([
                        Step::between(0, 3, Class::chars(" ,.-_/")),
                        Step::words(Trie::from_words_in_common_cases(MONTH_NAMES)),
                    ]),
                ],
            )
        },
    },
    Entry {
        kind: FRACTION,
        group: Group::Text,
        summary: "Two numbers of up to three digits, neither starting with a zero, with a slash between them",
        source: "English usage",
        matches: &[("3/4", "3/4"), ("1/20.", "1/20"), ("24/7 support", "24/7")],
        rejects: &["17/02/2009", "1/", "/2", "1/2a", "1234/5", "682/02", "01/2"],
        build: || {
            pattern(
                FRACTION,
                [
                    Step::one(Class::range('1', '9')),
                    Step::between(0, 2, digit()),
                    text("/"),
                    Step::one(Class::range('1', '9')),
                    Step::between(0, 2, digit()),
                    end(),
                    Step::not_before([text("/")]),
                ],
            )
        },
    },
    Entry {
        kind: DURATION,
        group: Group::Text,
        summary: "A number and a unit of time, with a space between them or not: `90 minutes`, `5ms`, `2 hrs`",
        source: "English usage, and the SI symbols `s`, `ms`, `min` and `h`",
        matches: &[
            ("90 minutes", "90 minutes"),
            ("5ms", "5ms"),
            ("2.5 hrs.", "2.5 hrs"),
            ("85 years old", "85 years"),
            ("1 Day", "1 Day"),
            ("30s", "30s"),
            ("2h.", "2h"),
        ],
        rejects: &["5 m", "5 May", "5 mins2", "minutes", "5  minutes", "2008 s", "22 h"],
        build: || {
            let mut units = Trie::from_words_in_common_cases([
                "second", "seconds", "minute", "minutes", "hour", "hours", "day", "days", "week",
                "weeks", "month", "months", "year", "years",
            ]);
            for unit in ["ms", "sec", "secs", "min", "mins", "hr", "hrs", "yr", "yrs"] {
                units.insert(unit);
            }
            let [a, b] = amount();
            pattern(
                DURATION,
                [
                    a,
                    b,
                    // A unit of one letter stands against its number:
                    // after a space it is too often something else.
                    Step::either([
                        vec![Step::optional([text(" ")]), Step::words(units)],
                        vec![words(&["s", "h"])],
                    ]),
                    end(),
                ],
            )
        },
    },
    Entry {
        kind: MEASUREMENT,
        group: Group::Text,
        summary: "A number and a unit of length, mass, volume, speed, temperature, data, power or frequency",
        source: "The SI Brochure for the metric symbols; English usage for the names and the other units",
        matches: &[
            ("5 km", "5 km"),
            ("162cm", "162cm"),
            ("1,000.5 kg.", "1,000.5 kg"),
            ("100 km/h", "100 km/h"),
            ("37.5 °C", "37.5 °C"),
            ("16 GB of", "16 GB"),
            ("111 centimeters", "111 centimeters"),
        ],
        rejects: &["5", "km", "5 kms", "5 kmx", "5 in", "5 USD"],
        build: || {
            let mut units = Trie::from_words_in_common_cases([
                "millimeter", "millimeters", "millimetre", "millimetres", "centimeter",
                "centimeters", "centimetre", "centimetres", "meter", "meters", "metre", "metres",
                "kilometer", "kilometers", "kilometre", "kilometres", "inch", "inches", "foot",
                "feet", "yard", "yards", "mile", "miles", "gram", "grams", "kilogram",
                "kilograms", "pound", "pounds", "ounce", "ounces", "liter", "liters", "litre",
                "litres", "milliliter", "milliliters", "millilitre", "millilitres", "byte",
                "bytes", "watt", "watts", "hertz",
            ]);
            for unit in [
                "mm", "cm", "m", "km", "ft", "yd", "mi", "mg", "g", "kg", "lb", "lbs", "oz", "ml",
                "mL", "L", "km/h", "kph", "mph", "m/s", "°C", "°F", "KB", "kB", "MB", "GB", "TB",
                "PB", "kW", "MW", "GW", "kWh", "MWh", "Hz", "kHz", "MHz", "GHz", "px", "dB",
            ] {
                units.insert(unit);
            }
            let [a, b] = amount();
            pattern(
                MEASUREMENT,
                [a, b, Step::optional([text(" ")]), Step::words(units), end()],
            )
        },
    },
    Entry {
        kind: ABBREVIATION,
        group: Group::Text,
        summary: "Two to six single letters, each with a full stop after it",
        source: "English usage",
        matches: &[("e.g.", "e.g."), ("U.S.A. and", "U.S.A."), ("i.e., that", "i.e.")],
        rejects: &["e.g", "eg.", "a.", "e.g.x", "a.b@c.de", "x.y.z@example.com"],
        build: || {
            let letter = || [Step::one(Class::letter()), text(".")];
            let [a, b] = letter();
            let [c, d] = letter();
            pattern(
                ABBREVIATION,
                [a, b, c, d, Step::repeat(4, letter()), end()],
            )
        },
    },
    Entry {
        kind: EMOTICON,
        group: Group::Text,
        summary: "A face or a heart made of marks: `:-)`, `;)`, `:D`, `<3`, `^_^`",
        source: "Wikipedia, List of emoticons, the Western ones",
        matches: &[(":-)", ":-)"), (":) ok", ":)"), (":D", ":D"), ("<3", "<3"), ("^_^.", "^_^")],
        // Marks that markdown, code and ratios are written with are left
        // out: `:*`, `:|`, `=(`, `:3`.
        rejects: &[":", ":Data", ":p5", "<", "^", ":*", ":|", "=(", ":3"],
        build: || {
            pattern(
                EMOTICON,
                [
                    words(&[
                        ":-)", ":)", ":-(", ":(", ";-)", ";)", ":-D", ":D", ":-P", ":P", ":-p",
                        ":p", ":'(", ":-O", ":-o", "<3", "</3", "^_^", "-_-", ">:(",
                    ]),
                    end(),
                ],
            )
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Lexer;

    #[test]
    fn every_matcher_does_what_its_entry_says() {
        for entry in entries() {
            let matcher = entry.matcher();
            assert_eq!(matcher.kind(), entry.kind);
            assert!(!entry.matches.is_empty(), "{} has no example", entry.kind);
            assert!(!entry.rejects.is_empty(), "{} refuses nothing", entry.kind);
            for (text, token) in entry.matches {
                assert_eq!(
                    matcher.match_prefix(text),
                    Some(*token),
                    "{}: {text:?}",
                    entry.kind
                );
            }
            for text in entry.rejects {
                assert_eq!(matcher.match_prefix(text), None, "{}: {text:?}", entry.kind);
            }
        }
    }

    /// In a lexer, in the order of the catalog, every example is a token
    /// of its kind: no matcher before it takes its text.
    #[test]
    fn every_example_is_a_token_of_its_kind_with_the_whole_catalog() {
        let lexer = Lexer::with(all()).unwrap();
        for entry in entries() {
            for (text, token) in entry.matches {
                let first = lexer.tokens(text).next().expect("a token");
                assert_eq!((first.kind, first.text), (entry.kind, *token), "{text:?}");
            }
        }
    }

    #[test]
    fn kinds_are_distinct_and_found_by_name_and_by_group() {
        let kinds: Vec<_> = entries().iter().map(|entry| entry.kind).collect();
        for (index, kind) in kinds.iter().enumerate() {
            assert!(!kinds[..index].contains(kind), "{kind} twice");
            assert_eq!(named(kind).map(|matcher| matcher.kind()), Some(*kind));
        }
        assert!(named("WORD").is_none());
        let grouped: usize = Group::ALL.iter().map(|&g| group(g).len()).sum();
        assert_eq!(grouped, kinds.len());
        assert_eq!(all().len(), kinds.len());
        for of in Group::ALL {
            assert!(!group(of).is_empty(), "{} is empty", of.name());
        }
    }

    /// The table of the README has a row for every matcher, with its
    /// group, its summary and its first example, in the order of the
    /// groups.
    #[test]
    fn the_readme_lists_the_catalog() {
        let readme = include_str!("../../README.md");
        let mut rows = Vec::new();
        for of in Group::ALL {
            for entry in entries().iter().filter(|entry| entry.group == of) {
                let example = entry.matches[0].1.replace('|', "\\|");
                rows.push(format!(
                    "| `{}` | {} | {} | `{example}` |",
                    entry.kind,
                    of.name(),
                    entry.summary
                ));
            }
        }
        assert!(
            readme.contains(&rows.join("\n")),
            "the catalog table of the README is not:\n{}",
            rows.join("\n")
        );
    }

    /// `docs/catalog.md` has a row for every matcher with its source, in
    /// the order of the catalog.
    #[test]
    fn the_docs_give_the_source_of_every_matcher() {
        let docs = include_str!("../../docs/catalog.md");
        let rows: Vec<_> = entries()
            .iter()
            .map(|entry| format!("| `{}` | {} |", entry.kind, entry.source))
            .collect();
        assert!(
            docs.contains(&rows.join("\n")),
            "the table of docs/catalog.md is not:\n{}",
            rows.join("\n")
        );
    }

    /// What the built-in matchers find is left to them.
    #[test]
    fn the_catalog_leaves_dates_times_and_amounts_alone() {
        let lexer = Lexer::with(all()).unwrap();
        let plain = Lexer::default();
        for text in [
            "Feb 17, 2009",
            "17th Feb 2009",
            "17.02.2009",
            "02/17/2009",
            "2022-01-06T08:42:27.549090+02:00",
            "2022-01-06 08:42",
            "10:30:15 PM",
            "4 PM",
            "$1,250.50",
            "5 USD",
            "dana@acme.com",
            "https://acme.com/inv?id=7",
            "#paid",
            "@bob",
            "1,000,000",
            "3.14",
            "100%",
            "don't",
            "well-known",
        ] {
            let with: Vec<_> = lexer.tokens(text).map(|t| (t.kind, t.text)).collect();
            let without: Vec<_> = plain.tokens(text).map(|t| (t.kind, t.text)).collect();
            assert_eq!(with, without, "{text:?}");
        }
    }

    /// A matcher that reads far and then fails is asked again at the next
    /// token. Each text is a run some matcher of the catalog reads into.
    #[test]
    fn long_runs_are_lexed_in_linear_time_with_the_whole_catalog() {
        const UNITS: &[&str] = &[
            "1 ", "12-", "1111 ", "1.", "10.0.", "1d:", "1:", "::", "ab:", "a.", "a.b.", "1/",
            "+1 ", "(1) ", "0 1 ", "01.", "1e", "0x", "#f", "v1.", "./", "~/a", "AB12 ", "GB29",
            "eyJ.", "-----", "5 k", "1st ", ":-", "$A", "13", "3a", "bc1", "ssh://", "C:\\", "1,",
            "UTC+", "d4",
        ];
        let lexer = Lexer::with(all()).unwrap();
        for unit in UNITS {
            let text = unit.repeat(20_000 / unit.len());
            let started = std::time::Instant::now();
            let end = lexer.tokens(&text).map(|token| token.end).last();
            assert_eq!(end, Some(text.len()));
            let elapsed = started.elapsed();
            assert!(elapsed.as_secs() < 2, "a run of {unit:?} took {elapsed:?}");
        }
    }
}
