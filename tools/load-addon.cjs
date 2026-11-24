// Loads the addon built in this directory and lexes a text with it: what
// the release workflow runs on each machine before an addon is published.
const addon = require("node:fs")
  .readdirSync(".")
  .find((file) => file.endsWith(".node"));
if (!addon) throw new Error("no addon was built here");
const { Lexer } = require(`${process.cwd()}/${addon}`);
const lexer = new Lexer("[]");
const { kinds, ends } = lexer.tokens("it loads");
if (lexer.kinds()[kinds[0]] !== "WORD" || ends.join() !== "2,3,8") {
  throw new Error(`unexpected tokens: ${ends}`);
}
console.log(addon, "loads and lexes on", process.platform, process.arch);
