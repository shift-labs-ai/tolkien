/**
 * Matchers of your own, described as data.
 *
 * A matcher is a kind and a list of steps, each one thing a lexer does at
 * a place in the text: take a character of a class, take a run of them,
 * take a fixed text, refuse what comes next. The description is handed to
 * the Rust engine once, and the matcher runs there: nothing here is called
 * while a text is lexed.
 */

/** A part of a class of characters. */
export type ClassPart =
  | { named: string }
  | { chars: string }
  | { range: [first: string, last: string] };

/** A set of characters: a part, or a list of classes taken together. */
export type Class = ClassPart | readonly Class[];

/** One thing a matcher does at its place in the text. */
export type Step =
  | { step: "run"; class: ClassPart[]; min: number; max?: number }
  | { step: "text"; text: string }
  | { step: "words"; words: string[]; commonCases: boolean }
  | { step: "number"; maxDigits: number; maxValue: number }
  | { step: "separator"; class: ClassPart[] }
  | { step: "sameSeparator" }
  | { step: "not"; class: ClassPart[] }
  | { step: "notBefore"; steps: Step[] }
  | { step: "optional"; steps: Step[] }
  | { step: "repeat"; max: number; steps: Step[] }
  | { step: "either"; choices: Step[][] }
  | { step: "trimEnd"; class: ClassPart[] };

/** A matcher described by its steps: what `matcher` returns. */
export interface PatternMatcher {
  kind: string;
  steps: Step[];
  count?: { of: ClassPart[]; min: number; max: number };
}

/** A matcher of the catalog, named by its kind. */
export interface CatalogMatcher {
  catalog: string;
}

/** A matcher: what a `Lexer` takes. */
export type Matcher = PatternMatcher | CatalogMatcher;

function parts(cls: Class): ClassPart[] {
  return Array.isArray(cls) ? cls.flatMap(parts) : [cls as ClassPart];
}

/** Each character of `chars`. */
export const chars = (chars: string): Class => ({ chars });

/** The characters from `first` to `last`, both included. */
export const range = (first: string, last: string): Class => ({
  range: [first, last],
});

/** Letters of any script. */
export const letter: Class = { named: "letter" };
/** Decimal digits of any script. The ASCII ones alone are `range("0", "9")`. */
export const digit: Class = { named: "digit" };
/** Any number character: digits, Roman numerals, fractions. */
export const numeric: Class = { named: "number" };
/** White space, line breaks included. */
export const whitespace: Class = { named: "whitespace" };
/** Characters that force a line break. */
export const lineBreak: Class = { named: "line_break" };
/** Hyphens, dashes and minus signs. */
export const dash: Class = { named: "dash" };
/** Underscores. */
export const underscore: Class = { named: "underscore" };
/** Apostrophes that can stand inside a word, the typographic one included. */
export const apostrophe: Class = { named: "apostrophe" };
/** Full stops of any script or width. */
export const fullStop: Class = { named: "full_stop" };
/** Commas of any script or width. */
export const comma: Class = { named: "comma" };
/** Colons of any script or width. */
export const colon: Class = { named: "colon" };
/** Slashes and backslashes. */
export const slash: Class = { named: "slash" };
/** Currency symbols, such as `$` and the euro sign. */
export const currencySymbol: Class = { named: "currency_symbol" };

/** One character of `cls`. */
export const one = (cls: Class): Step => exactly(1, cls);

/** One character of `cls` or more, as many as there are. */
export const run = (cls: Class): Step => ({
  step: "run",
  class: parts(cls),
  min: 1,
});

/**
 * Exactly `count` characters of `cls`. More of them may follow: say `not`
 * after it to refuse that.
 */
export const exactly = (count: number, cls: Class): Step =>
  between(count, count, cls);

/** From `min` to `max` characters of `cls`, as many as there are. */
export const between = (min: number, max: number, cls: Class): Step => ({
  step: "run",
  class: parts(cls),
  min,
  max,
});

/** The text `text`, as it is written. */
export const text = (text: string): Step => ({ step: "text", text });

/**
 * The longest of `words` that stands at this place. With `commonCases`, a
 * word is also taken in lower case, in upper case and with a capital.
 */
export const words = (
  words: readonly string[],
  options: { commonCases?: boolean } = {},
): Step => ({
  step: "words",
  words: [...words],
  commonCases: options.commonCases ?? false,
});

/**
 * A number of one to `maxDigits` ASCII digits that is at most `maxValue`,
 * such as a part of an IP address.
 */
export const number = (maxDigits: number, maxValue: number): Step => ({
  step: "number",
  maxDigits,
  maxValue,
});

/** One character of `cls`, which `sameSeparator` takes again. */
export const separator = (cls: Class): Step => ({
  step: "separator",
  class: parts(cls),
});

/** The character the last `separator` took. */
export const sameSeparator = (): Step => ({ step: "sameSeparator" });

/**
 * Succeeds, without moving, when the next character is not in `cls` or the
 * text has ended. At the end of a matcher it says the token is not the
 * start of something longer.
 */
export const not = (cls: Class): Step => ({ step: "not", class: parts(cls) });

/** Succeeds, without moving, when `steps` do not succeed here. */
export const notBefore = (steps: readonly Step[]): Step => ({
  step: "notBefore",
  steps: [...steps],
});

/** `steps`, if they succeed; nothing otherwise. */
export const optional = (steps: readonly Step[]): Step => ({
  step: "optional",
  steps: [...steps],
});

/**
 * `steps` again and again, at most `max` times, as long as they succeed.
 * The bound keeps lexing fast: a matcher that reads on without one and
 * then fails reads the same text again at the next token.
 */
export const repeat = (max: number, steps: readonly Step[]): Step => ({
  step: "repeat",
  max,
  steps: [...steps],
});

/** The first of `choices` whose steps succeed. */
export const either = (choices: readonly (readonly Step[])[]): Step => ({
  step: "either",
  choices: choices.map((steps) => [...steps]),
});

/** Steps back over the characters of `cls` at the end of the token. */
export const trimEnd = (cls: Class): Step => ({
  step: "trimEnd",
  class: parts(cls),
});

/**
 * A matcher of the kind `kind`: it matches where `steps` succeed in turn,
 * and takes what they passed. With `count`, the token must hold from `min`
 * to `max` characters of the class `of`, as a card number holds 13 to 19
 * digits however they are grouped.
 */
export function matcher(
  kind: string,
  steps: readonly Step[],
  options: { count?: { of: Class; min: number; max: number } } = {},
): PatternMatcher {
  const { count } = options;
  return {
    kind,
    steps: [...steps],
    ...(count && {
      count: { of: parts(count.of), min: count.min, max: count.max },
    }),
  };
}
