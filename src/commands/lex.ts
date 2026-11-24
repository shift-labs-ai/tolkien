import { readFileSync, writeFileSync } from "node:fs";
import { catalog, Lexer, type Matcher, type Token } from "../index.js";
import { EXIT_ERROR, EXIT_USER_ERROR } from "../utils/exit-codes.js";
import type { OutputOptions } from "../utils/output.js";
import { dim, error, jsonOutput, output, success } from "../utils/output.js";

export interface LexOptions extends OutputOptions {
  /** Write the tokens to this file. */
  output?: string;
  /** The text to lex, in place of a source. */
  text?: string;
  /** Add every matcher of the catalog. */
  catalog?: boolean;
  /** Add these groups of the catalog. */
  groups: string[];
  /** Keep only tokens of these kinds. */
  kinds: string[];
  /** Leave out white space and line breaks. */
  skipSpace?: boolean;
}

const SPACE = new Set(["WHITE_SPACE", "LINE_TERMINATION"]);

/** Says why the command cannot go on and exits with `status`. */
function fail(options: OutputOptions, message: string, status: number): never {
  output(options, {
    json: () => ({ success: false, error: message }),
    human: () => error(message),
  });
  process.exit(status);
}

/** A mistake of the user's. */
function refuse(options: OutputOptions, message: string): never {
  fail(options, message, EXIT_USER_ERROR);
}

async function readStdin(): Promise<string> {
  const chunks: Uint8Array[] = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }
  return Buffer.concat(chunks).toString("utf-8");
}

/** The text to lex and what to call it. */
async function read(
  source: string | undefined,
  options: LexOptions,
): Promise<{ label: string; text: string }> {
  if (options.text !== undefined) {
    if (source !== undefined) {
      refuse(options, "Give a source or --text, not both.");
    }
    return { label: "text", text: options.text };
  }
  if (source === undefined || source === "-") {
    if (process.stdin.isTTY) {
      refuse(
        options,
        "No input on stdin. Pipe a file: cat notes.txt | tolkien -",
      );
    }
    return { label: "stdin", text: await readStdin() };
  }
  try {
    return { label: source, text: readFileSync(source, "utf-8") };
  } catch (err) {
    const missing = (err as NodeJS.ErrnoException).code === "ENOENT";
    const message = missing
      ? `File not found: ${source}`
      : (err as Error).message;
    fail(options, message, EXIT_ERROR);
  }
}

/** The matchers the options add to the built-in ones. */
function added(options: LexOptions): Matcher[] {
  try {
    // A group is checked also where the whole catalog is asked for.
    const groups = catalog.group(...options.groups);
    return options.catalog ? catalog.all() : groups;
  } catch (err) {
    refuse(options, `${(err as Error).message}. Run 'tolkien kinds'.`);
  }
}

/** Why `kind` is not one of the lexer's. */
function unknownKind(kind: string): string {
  const entry = catalog.entries().find((entry) => entry.kind === kind);
  return entry
    ? `${kind} is a kind of the catalog. Add -g ${entry.group}, or -c for all of it.`
    : `No kind ${kind}. Run 'tolkien kinds'.`;
}

/** A token as a line: kind, start, end and text, with tabs between. */
const line = (token: Token) =>
  `${token.kind}\t${token.start}\t${token.end}\t${JSON.stringify(token.text)}`;

/** The tokens as lines for a person: the same fields, in columns. */
export function columns(tokens: readonly Token[]): string[] {
  const width = tokens.reduce(
    (most, { kind }) => Math.max(most, kind.length),
    0,
  );
  return tokens.map(
    (token) =>
      `${token.kind.padEnd(width)}  ${dim(`${token.start}..${token.end}`.padEnd(12))} ${JSON.stringify(token.text)}`,
  );
}

/** Writes `lines` to stdout, waiting where the reader is slow. */
async function print(lines: string[]): Promise<void> {
  const CHUNK = 4096;
  for (let from = 0; from < lines.length; from += CHUNK) {
    const chunk = `${lines.slice(from, from + CHUNK).join("\n")}\n`;
    if (!process.stdout.write(chunk)) {
      await new Promise((resume) => process.stdout.once("drain", resume));
    }
  }
}

export async function lex(
  source: string | undefined,
  options: LexOptions,
): Promise<void> {
  // The reader closed the pipe, as `head` does.
  process.stdout.on("error", (err: NodeJS.ErrnoException) => {
    if (err.code === "EPIPE") process.exit(0);
    throw err;
  });

  const { label, text } = await read(source, options);
  const lexer = new Lexer(added(options));
  const unknown = options.kinds.find((kind) => !lexer.kinds.includes(kind));
  if (unknown !== undefined) {
    refuse(options, unknownKind(unknown));
  }
  const wanted = options.kinds.length > 0 ? new Set(options.kinds) : undefined;

  const all = lexer.tokens(text);
  const tokens: Token[] = [];
  for (let index = 0; index < all.length; index++) {
    const kind = all.kind(index);
    if (options.skipSpace && SPACE.has(kind)) continue;
    if (wanted && !wanted.has(kind)) continue;
    tokens.push(all.at(index) as Token);
  }

  if (options.output) {
    const body = options.json
      ? `${JSON.stringify(tokens, null, 2)}\n`
      : tokens.map((token) => `${line(token)}\n`).join("");
    writeFileSync(options.output, body);
    output(options, {
      json: () => ({
        success: true,
        source: label,
        output: options.output,
        count: tokens.length,
      }),
      human: () => {
        success(`Lexed → ${options.output}`);
        console.log(dim(`  ${tokens.length} tokens`));
      },
    });
  } else if (options.json) {
    jsonOutput({ success: true, source: label, count: tokens.length, tokens });
  } else if (options.quiet || !process.stdout.isTTY) {
    // Into a pipe or a file the lines are the ones a program reads.
    await print(tokens.map(line));
  } else {
    await print(columns(tokens));
  }
}
