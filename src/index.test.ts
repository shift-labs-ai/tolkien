import { describe, expect, test } from "bun:test";
import { matchers, texts } from "../test/fixtures.js";
import {
  between,
  catalog,
  chars,
  dash,
  digit,
  either,
  exactly,
  fullStop,
  Lexer,
  letter,
  matcher,
  not,
  notBefore,
  number,
  one,
  optional,
  range,
  repeat,
  run,
  type Step,
  sameSeparator,
  separator,
  type Token,
  text,
  trimEnd,
  words,
} from "./index.js";

/** The steps the engine knows: it lists them when it refuses one. */
function stepsOfTheEngine(): string[] {
  const unknown = { step: "jump" } as unknown as Step;
  try {
    new Lexer([matcher("A", [unknown])]);
  } catch (error) {
    const [, known = ""] = String(error).split("expected one of");
    return [...known.matchAll(/`(\w+)`/g)].map(([, step]) => step).sort();
  }
  return [];
}

const kindsAndTexts = (tokens: Iterable<Token>) =>
  [...tokens]
    .filter((token) => token.kind !== "WHITE_SPACE")
    .map((token) => [token.kind, token.text]);

describe("Lexer", () => {
  test("names the parts of a sentence", () => {
    const tokens = new Lexer().tokens(
      "Dana paid $1,250.50 on Feb 17, 2009 at 8:05 PM",
    );
    expect(kindsAndTexts(tokens)).toEqual([
      ["WORD", "Dana"],
      ["WORD", "paid"],
      ["CURRENCY", "$1,250.50"],
      ["WORD", "on"],
      ["DATE", "Feb 17, 2009"],
      ["WORD", "at"],
      ["TIME", "8:05 PM"],
    ]);
  });

  test("lists its kinds in the order they are tried", () => {
    const kinds = new Lexer().kinds;
    expect(kinds[0]).toBe("DATE_ISO_8601");
    expect(kinds).toContain("WORD");
    expect(kinds.at(-1)).toBe("OTHER");
    expect(new Lexer(matchers).kinds.slice(0, 4)).toEqual([
      "CARD_NUMBER",
      "IPV4",
      "MAC_ADDRESS",
      "KEYWORD",
    ]);
  });

  test("tokens cover the text, with offsets into the string", () => {
    const lexer = new Lexer(matchers);
    for (const input of texts) {
      const tokens = lexer.tokens(input);
      let position = 0;
      for (let index = 0; index < tokens.length; index++) {
        expect(tokens.start(index)).toBe(position);
        expect(tokens.end(index)).toBeGreaterThan(position);
        expect(tokens.text(index)).toBe(
          input.slice(tokens.start(index), tokens.end(index)),
        );
        position = tokens.end(index);
      }
      expect(position).toBe(input.length);
      expect([...tokens].map((token) => token.text).join("")).toBe(input);
    }
  });

  test("an offset counts what a string index counts", () => {
    const tokens = [...new Lexer().tokens("é 👍 x")];
    expect(tokens.map((token) => [token.kind, token.start, token.end])).toEqual(
      [
        ["WORD", 0, 1],
        ["WHITE_SPACE", 1, 2],
        ["SYMBOL", 2, 4],
        ["WHITE_SPACE", 4, 5],
        ["WORD", 5, 6],
      ],
    );
  });

  test("matchers of your own are tried before the built-in ones", () => {
    const tokens = new Lexer(matchers).tokens(texts[2]);
    expect(kindsAndTexts(tokens)).toEqual([
      ["WORD", "Card"],
      ["CARD_NUMBER", "4111 1111-1111 1111"],
      ["WORD", "from"],
      ["IPV4", "192.168.1.20"],
      ["SYMBOL", "("],
      ["MAC_ADDRESS", "1d:d8:97:09:54:39"],
      ["SYMBOL", ")"],
      ["SYMBOL", "."],
      ["KEYWORD", "Secret"],
      ["SYMBOL", ":"],
      ["DATE", "17.02.2009"],
      ["SYMBOL", ","],
      ["ALPHA_NUMERIC_SEQUENCE", "v1"],
      ["LIKE_NUMBER", ".2"],
    ]);
    expect(tokens.ofKind("IPV4", "MAC_ADDRESS", "NO_SUCH_KIND")).toEqual([
      { kind: "IPV4", start: 30, end: 42, text: "192.168.1.20" },
      { kind: "MAC_ADDRESS", start: 44, end: 61, text: "1d:d8:97:09:54:39" },
    ]);
  });

  test("a token is read by its index", () => {
    const tokens = new Lexer().tokens("a b");
    expect(tokens.length).toBe(3);
    expect(tokens.at(2)).toEqual({ kind: "WORD", start: 2, end: 3, text: "b" });
    expect(tokens.at(3)).toBeUndefined();
    expect(tokens.at(-1)).toBeUndefined();
    expect(tokens.at(0.5)).toBeUndefined();
    expect(new Lexer().tokens("").length).toBe(0);
  });

  test("a long text gives the tokens of its parts", () => {
    // Long enough to be lexed on several threads.
    const sentence = "Paid $5 on Feb 17, 2009 to a@b.cd for 3 items. ";
    const lexer = new Lexer();
    const once = [...lexer.tokens(sentence)];
    const tokens = lexer.tokens(sentence.repeat(20_000));
    expect(tokens.length).toBe(once.length * 20_000);
    const last = tokens.length - once.length;
    for (let index = 0; index < once.length; index++) {
      expect(tokens.kind(last + index)).toBe(once[index].kind);
      expect(tokens.text(last + index)).toBe(once[index].text);
    }
  });

  test("the catalog adds matchers by name", () => {
    const entries = catalog.entries();
    expect(entries.length).toBeGreaterThan(30);
    expect(entries[0]).toEqual({
      kind: "AWS_ACCESS_KEY_ID",
      group: "secrets",
      summary: expect.any(String),
    });
    expect(catalog.all().length).toBe(entries.length);
    expect(catalog.of("MAC_ADDRESS", "IPV4")).toEqual([
      { catalog: "IPV4" },
      { catalog: "MAC_ADDRESS" },
    ]);
    expect(catalog.group("network").length).toBeGreaterThan(3);
    expect(() => catalog.of("NOPE")).toThrow('the catalog has no "NOPE"');
    expect(() => catalog.group("nope")).toThrow('the catalog has no "nope"');

    const lexer = new Lexer(catalog.all());
    const tokens = lexer.tokens(
      "Ping 192.168.1.20 (1d:d8:97:09:54:39), call +1 (555) 010-9999 by 5 PM",
    );
    expect(kindsAndTexts(tokens)).toEqual([
      ["WORD", "Ping"],
      ["IPV4", "192.168.1.20"],
      ["SYMBOL", "("],
      ["MAC_ADDRESS", "1d:d8:97:09:54:39"],
      ["SYMBOL", ")"],
      ["SYMBOL", ","],
      ["WORD", "call"],
      ["PHONE_NUMBER", "+1 (555) 010-9999"],
      ["WORD", "by"],
      ["TIME", "5 PM"],
    ]);
    // A matcher of your own stands before those of the catalog, or after.
    const mixed = new Lexer([
      matcher("LOCALHOST", [text("127.0.0.1")]),
      ...catalog.group("network"),
    ]);
    expect(kindsAndTexts(mixed.tokens("127.0.0.1 8.8.8.8"))).toEqual([
      ["LOCALHOST", "127.0.0.1"],
      ["IPV4", "8.8.8.8"],
    ]);
  });

  // The builders give every step the engine knows, and no other: this
  // fails when a step is added to one side only.
  test("every step has a builder the engine takes", () => {
    const everything = matcher(
      "EVERYTHING",
      [
        text("id:"),
        words(["a", "b"], { commonCases: true }),
        number(3, 255),
        separator(chars(":-")),
        exactly(2, letter),
        sameSeparator(),
        optional([text("?")]),
        either([[text("x")], []]),
        repeat(2, [one(dash), run(range("0", "9"))]),
        between(0, 9, fullStop),
        trimEnd(fullStop),
        notBefore([text("!")]),
        not([letter, digit]),
      ],
      { count: { of: [letter, digit], min: 1, max: 40 } },
    );
    const known = stepsOfTheEngine();
    expect(known).not.toEqual([]);
    expect(
      [...new Set(everything.steps.map((step) => step.step))].sort(),
    ).toEqual(known);
    const lexer = new Lexer([everything]);
    expect(lexer.tokens("id:A12-ab-x-7.. ").at(0)).toEqual({
      kind: "EVERYTHING",
      start: 0,
      end: 13,
      text: "id:A12-ab-x-7",
    });
    expect(lexer.tokens("id:A12-ab-x!").at(0)?.kind).toBe("WORD");
  });

  test("a wrong matcher is refused with a reason", () => {
    expect(() => new Lexer([matcher("WORD", [text("x")])])).toThrow(
      "two matchers produce the kind WORD",
    );
    expect(() => new Lexer([matcher("A", [one({ named: "vowel" })])])).toThrow(
      'no set of characters is named "vowel"',
    );
    expect(
      () => new Lexer([matcher("A", [run({ range: ["ab", "c"] })])]),
    ).toThrow("not a matcher");
    expect(
      () => new Lexer([matcher("A", [not([])]), matcher("A", [text("x")])]),
    ).toThrow("two matchers produce the kind A");
  });

  test("lexing many texts in a row does not keep their tokens", () => {
    // About 2 MB, and 3.6 MB of token arrays for each call. Arrays that
    // are freed only when the event loop turns add up to 360 MB here.
    const log = "Server 10.0.0.7 answered on 2024-01-15, ok.\n".repeat(50_000);
    const lexer = new Lexer();
    const megabytes = () => process.memoryUsage().rss / 1e6;
    lexer.tokens(log);
    const before = megabytes();
    for (let call = 0; call < 100; call++) lexer.tokens(log);
    expect(megabytes() - before).toBeLessThan(150);
  }, 30_000);
});
