#!/usr/bin/env node

import { createRequire } from "node:module";
import { Command } from "commander";

// Command modules are imported inside each action: they load the native
// engine, and --help and --version should not wait for it.

const require = createRequire(import.meta.url);
const { version } = require("../package.json");

const collect = (value: string, all: string[]) => [
  ...all,
  ...value.split(",").filter(Boolean),
];

const program = new Command();

program
  .name("tolkien")
  .description("Split text into typed tokens.")
  .version(`tolkien ${version}`, "-V, --version")
  .option("--json", "Output as JSON")
  .option("-q, --quiet", "Tab-separated lines only, no decoration")
  .option("-o, --output <file>", "Write to file instead of stdout")
  .option("-t, --text <text>", "Lex this text instead of a file")
  .option("-c, --catalog", "Add every matcher of the catalog")
  .option("-g, --group <names>", "Add groups of the catalog", collect, [])
  .option("-k, --kind <kinds>", "Print only tokens of these kinds", collect, [])
  .option("-s, --skip-space", "Leave out white space and line breaks")
  .addHelpText(
    "after",
    `
Examples:
  $ tolkien notes.txt                    Tokens of a file
  $ tolkien -t "Mail dana@acme.com"      Tokens of a text
  $ cat notes.txt | tolkien -            Read from stdin
  $ tolkien notes.txt -c -k IPV4,IBAN    Find kinds of the catalog
  $ tolkien notes.txt -g secrets --json  Secrets, as JSON

A token is printed as its kind, start, end and text. Start and end are
indexes into the text as a JavaScript string.

Docs: https://github.com/shift-labs-ai/tolkien`,
  );

/** The options of `lex`, from wherever on the command line they stand. */
function lexOptions(globals: Record<string, unknown>) {
  return {
    json: globals.json as boolean | undefined,
    quiet: globals.quiet as boolean | undefined,
    output: globals.output as string | undefined,
    text: globals.text as string | undefined,
    catalog: globals.catalog as boolean | undefined,
    groups: globals.group as string[],
    kinds: globals.kind as string[],
    skipSpace: globals.skipSpace as boolean | undefined,
  };
}

program
  .command("lex", { isDefault: true })
  .description("Print the tokens of a file, of stdin or of a text")
  .argument("[source]", "File path, or - for stdin")
  .action(async (source, _opts, cmd) => {
    const { lex } = await import("./commands/lex.js");
    await lex(source, lexOptions(cmd.optsWithGlobals()));
  });

program
  .command("kinds")
  .description("List the kinds of tokens: built-in and of the catalog")
  .action(async (_opts, cmd) => {
    const globals = cmd.optsWithGlobals();
    const { kinds } = await import("./commands/kinds.js");
    await kinds({ json: globals.json, quiet: globals.quiet });
  });

program
  .command("onboard")
  .description("Add tolkien instructions to CLAUDE.md or AGENTS.md")
  .action(async (_opts, cmd) => {
    const globals = cmd.optsWithGlobals();
    const { onboard } = await import("./commands/onboard.js");
    await onboard({ json: globals.json, quiet: globals.quiet });
  });

// No arguments and nothing piped in: show concise help.
if (process.argv.length <= 2 && process.stdin.isTTY) {
  console.log(`tolkien — split text into typed tokens

Usage:  tolkien <file> [options]

Examples:
  $ tolkien notes.txt
  $ tolkien -t "Mail dana@acme.com by 2026-11-23"
  $ cat notes.txt | tolkien - --catalog --json

Commands:
  tolkien kinds       List the kinds of tokens
  tolkien onboard     Add instructions to CLAUDE.md

Run tolkien --help for all options.
Docs: https://github.com/shift-labs-ai/tolkien`);
  process.exit(0);
}

program.parseAsync(process.argv).catch((err) => {
  console.error("Fatal error:", err.message);
  process.exit(1);
});
