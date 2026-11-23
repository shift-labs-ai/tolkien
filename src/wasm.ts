/**
 * tolkien for browsers: the same lexer, compiled to WebAssembly.
 *
 * Call the default export once before building a `Lexer`: it loads the
 * engine. A text is lexed on the calling thread, so lex a long one in a
 * Web Worker to keep a page responsive.
 */

import initEngine, { type InitInput, Lexer as WasmLexer } from "./engine.js";
import { type EngineLexer, lexerOver } from "./lexer.js";

export * from "./describe.js";
export { type Token, Tokens } from "./lexer.js";

/**
 * Loads the engine. With no argument it is fetched from beside this
 * module; a bundler or a server can pass its URL, a `Response` or its
 * bytes.
 */
export default async function init(
  engine?: InitInput | Promise<InitInput>,
): Promise<void> {
  await initEngine(
    engine === undefined ? undefined : { module_or_path: engine },
  );
}

function create(matchers: string): EngineLexer {
  const lexer = new WasmLexer(matchers);
  return {
    kinds: () => lexer.kinds(),
    tokens(text) {
      const arrays = lexer.tokens(text);
      try {
        return { kinds: arrays.kinds, ends: arrays.ends };
      } finally {
        arrays.free();
      }
    },
  };
}

export const Lexer = lexerOver(create);
export type Lexer = InstanceType<typeof Lexer>;
