/**
 * tolkien for Node: a lexer for natural text, run by a native Rust addon.
 */

import { createRequire } from "node:module";
import { type EngineLexer, lexerOver } from "./lexer.js";

export * from "./describe.js";
export { type Token, Tokens } from "./lexer.js";

interface NativeBinding {
  Lexer: new (matchers: string) => EngineLexer;
}

const require = createRequire(import.meta.url);
const native = require("../native.cjs") as NativeBinding;

export const Lexer = lexerOver((matchers) => new native.Lexer(matchers));
export type Lexer = InstanceType<typeof Lexer>;
