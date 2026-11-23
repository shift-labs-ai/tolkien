# @shiftlabs/tolkien-wasm

The [tolkien](https://github.com/shift-labs-ai/tolkien) lexer compiled to
WebAssembly, for browsers. One pass over a string gives its tokens: words,
numbers, dates, times, email addresses, URLs, amounts of money, and the
kinds of matchers you describe yourself.

In Node, use [`@shiftlabs/tolkien`](https://www.npmjs.com/package/@shiftlabs/tolkien):
the same API over a native addon.

```ts
import init, { Lexer } from "@shiftlabs/tolkien-wasm";

await init(); // loads the engine, once

const lexer = new Lexer();
for (const token of lexer.tokens("Mail dana@acme.com by 2026-11-23")) {
  console.log(token.kind, token.start, token.end, token.text);
}
```

`init` fetches `engine_bg.wasm` from beside the module. Pass a URL, a
`Response` or the bytes when a bundler or a server puts it elsewhere:

```ts
import engine from "@shiftlabs/tolkien-wasm/engine_bg.wasm?url"; // Vite
await init(engine);
```

A text is lexed on the calling thread. Lex a long one in a Web Worker to
keep a page responsive.

Matchers of your own, and everything else, are described in the
[README](https://github.com/shift-labs-ai/tolkien#from-javascript).
