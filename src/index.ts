/**
 * tolkien for Node: a lexer for natural text, run by a native Rust addon.
 */

import { createRequire } from "node:module";
import { catalogOver, type EngineLexer, lexerOver } from "./lexer.js";

export * from "./describe.js";
export {
  type Catalog,
  type CatalogEntry,
  type Token,
  Tokens,
} from "./lexer.js";

interface NativeBinding {
  Lexer: new (matchers: string) => EngineLexer;
  catalog(): string;
}

const require = createRequire(import.meta.url);
const native = require("../native.cjs") as NativeBinding;

export const Lexer = lexerOver((matchers) => new native.Lexer(matchers));
export const catalog = catalogOver(native.catalog);
export type Lexer = InstanceType<typeof Lexer>;
