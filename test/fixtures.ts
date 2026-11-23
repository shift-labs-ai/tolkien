// Matchers and texts that both packages are tested with.

import {
  chars,
  digit,
  exactly,
  letter,
  type Matcher,
  matcher,
  not,
  notBefore,
  number,
  one,
  range,
  repeat,
  run,
  sameSeparator,
  separator,
  text,
  words,
} from "../src/describe.js";

const asciiDigit = range("0", "9");
const hex = [asciiDigit, range("a", "f"), range("A", "F")];
const letterOrDigit = [letter, digit];
const part = number(3, 255);
const dot = text(".");

/** The kind of matcher a detector of sensitive data needs. */
export const matchers: Matcher[] = [
  matcher(
    "CARD_NUMBER",
    [
      run(asciiDigit),
      repeat(7, [one(chars(" -")), run(asciiDigit)]),
      not(letterOrDigit),
    ],
    { count: { of: asciiDigit, min: 13, max: 19 } },
  ),
  matcher("IPV4", [
    part,
    dot,
    part,
    dot,
    part,
    dot,
    part,
    notBefore([dot, one(asciiDigit)]),
    not(letterOrDigit),
  ]),
  matcher("MAC_ADDRESS", [
    exactly(2, hex),
    separator(chars(":-")),
    exactly(2, hex),
    sameSeparator(),
    exactly(2, hex),
    sameSeparator(),
    exactly(2, hex),
    sameSeparator(),
    exactly(2, hex),
    sameSeparator(),
    exactly(2, hex),
    not(letterOrDigit),
  ]),
  matcher("KEYWORD", [
    words(["confidential", "secret"], { commonCases: true }),
    not(letterOrDigit),
  ]),
];

export const texts = [
  "",
  "Dana (dana@acme.com) paid $1,250.50 on Feb 17, 2009 at 8:05 PM, see https://acme.com/inv?id=7",
  "Card 4111 1111-1111 1111 from 192.168.1.20 (1d:d8:97:09:54:39). Secret: 17.02.2009, v1.2",
  "naïve café 👍 日本語のテキスト。 👩\u200d💻 שלום 10.0.0.256 ٣٥",
  "one\r\ntwo\n\n\tthree  ",
];
