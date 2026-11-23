/**
 * The lexer and its tokens, over whichever engine runs them: the native
 * addon in Node, WebAssembly in a browser.
 */

import type { Matcher } from "./describe.js";

/** A piece of the text and what it is. */
export interface Token {
  /** The kind of the matcher that found the token, or `OTHER`. */
  kind: string;
  /** Index of the token's first character in the text. */
  start: number;
  /** Index just past its last character. */
  end: number;
  /** The token's text: `text.slice(start, end)`. */
  text: string;
}

/** What an engine gives for a text. */
export interface TokenArrays {
  /** For each token, the place of its kind in the engine's kinds. */
  kinds: Uint16Array;
  /** For each token, its end. A token starts where the one before ends. */
  ends: Uint32Array;
}

/** A lexer in the engine. */
export interface EngineLexer {
  kinds(): string[];
  tokens(text: string): TokenArrays;
}

/**
 * The tokens of one text. They cover it: every character belongs to
 * exactly one token.
 *
 * The tokens are kept as two arrays of numbers, and a `Token` object is
 * made when one is asked for. To go through a long text without making an
 * object for every token, use `kind`, `start`, `end` and `text` with an
 * index.
 */
export class Tokens implements Iterable<Token> {
  readonly #text: string;
  readonly #kindNames: readonly string[];
  readonly #kinds: Uint16Array;
  readonly #ends: Uint32Array;

  constructor(text: string, kindNames: readonly string[], arrays: TokenArrays) {
    this.#text = text;
    this.#kindNames = kindNames;
    this.#kinds = arrays.kinds;
    this.#ends = arrays.ends;
  }

  /** How many tokens there are. */
  get length(): number {
    return this.#kinds.length;
  }

  /** The kind of token `index`. */
  kind(index: number): string {
    return this.#kindNames[this.#kinds[index]];
  }

  /** Where token `index` starts in the text. */
  start(index: number): number {
    return index === 0 ? 0 : this.#ends[index - 1];
  }

  /** Where token `index` ends in the text. */
  end(index: number): number {
    return this.#ends[index];
  }

  /** The text of token `index`. */
  text(index: number): string {
    return this.#text.slice(this.start(index), this.#ends[index]);
  }

  /** Token `index`, or `undefined` outside the tokens. */
  at(index: number): Token | undefined {
    if (!Number.isInteger(index) || index < 0 || index >= this.length) {
      return undefined;
    }
    return {
      kind: this.kind(index),
      start: this.start(index),
      end: this.end(index),
      text: this.text(index),
    };
  }

  /** The tokens of the given kinds, in the order of the text. */
  ofKind(...kinds: string[]): Token[] {
    const wanted = new Set(
      kinds.map((kind) => this.#kindNames.indexOf(kind)).filter((i) => i >= 0),
    );
    const found: Token[] = [];
    for (let index = 0; index < this.length; index++) {
      if (wanted.has(this.#kinds[index])) {
        found.push(this.at(index) as Token);
      }
    }
    return found;
  }

  *[Symbol.iterator](): Iterator<Token> {
    for (let index = 0; index < this.length; index++) {
      yield this.at(index) as Token;
    }
  }
}

/** The `Lexer` class over an engine that builds lexers. */
export function lexerOver(create: (matchers: string) => EngineLexer) {
  /**
   * Splits text into tokens: words, numbers, dates, times, email
   * addresses, URLs, amounts of money, and the kinds of the matchers it is
   * given, which are tried before the built-in ones in the order given.
   *
   * Build a lexer once and use it for every text.
   */
  return class Lexer {
    readonly #engine: EngineLexer;

    /** The kinds a token can have, in the order their matchers are tried. */
    readonly kinds: readonly string[];

    constructor(matchers: readonly Matcher[] = []) {
      this.#engine = create(JSON.stringify(matchers));
      this.kinds = Object.freeze(this.#engine.kinds());
    }

    /** The tokens of `text`, from start to end. */
    tokens(text: string): Tokens {
      return new Tokens(text, this.kinds, this.#engine.tokens(text));
    }
  };
}
