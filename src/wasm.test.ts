import { describe, expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { matchers, texts } from "../test/fixtures.js";
import { Lexer } from "./index.js";

// The browser build is the same engine compiled to wasm32, so its tokens
// must be the native addon's. Run `bun run build:wasm` first; without its
// output this suite is skipped.
const pkg = new URL("../npm/wasm/", import.meta.url);
const built = existsSync(new URL("engine_bg.wasm", pkg));

describe.skipIf(!built)("wasm package", () => {
  const load = async () => {
    const wasm = await import(new URL("wasm.js", pkg).href);
    await wasm.default(readFileSync(new URL("engine_bg.wasm", pkg)));
    return wasm as typeof import("./index.js");
  };

  test("gives the tokens of the native addon", async () => {
    const wasm = await load();
    for (const added of [[], matchers]) {
      const native = new Lexer(added);
      const browser = new wasm.Lexer(added);
      expect(browser.kinds).toEqual(native.kinds);
      for (const input of [...texts, texts.join("\n").repeat(2_000)]) {
        const expected = native.tokens(input);
        const tokens = browser.tokens(input);
        expect(tokens.length).toBe(expected.length);
        for (let index = 0; index < expected.length; index++) {
          if (
            tokens.kind(index) !== expected.kind(index) ||
            tokens.end(index) !== expected.end(index)
          ) {
            expect(tokens.at(index)).toEqual(expected.at(index));
          }
        }
      }
    }
  });

  test("exports the matcher builders and refuses a wrong matcher", async () => {
    const wasm = await load();
    const { matcher, text } = wasm;
    const lexer = new wasm.Lexer([matcher("HELLO", [text("hello")])]);
    expect(lexer.tokens("hello you").at(0)).toEqual({
      kind: "HELLO",
      start: 0,
      end: 5,
      text: "hello",
    });
    expect(() => new wasm.Lexer([matcher("WORD", [text("x")])])).toThrow(
      "two matchers produce the kind WORD",
    );
  });
});
