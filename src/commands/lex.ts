import { readFileSync, writeFileSync } from "node:fs";
import { catalog, Lexer, type Matcher } from "../index.js";
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

interface Row {
  kind: string;
  start: number;
  end: number;
  text: string;
}

const SPACE = new Set(["WHITE_SPACE", "LINE_TERMINATION"]);

/** A user's mistake: say what it is and exit. */
function refuse(options: OutputOptions, message: string): never {
  output(options, {
    json: () => ({ success: false, error: message }),
    human: () => error(message),
  });
  process.exit(EXIT_USER_ERROR);
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
    output(options, {
      json: () => ({ success: false, error: message }),
      human: () => error(message),
    });
    process.exit(EXIT_ERROR);
  }
}

/** The matchers the options add to the built-in ones. */
function added(options: LexOptions): Matcher[] {
  if (options.catalog) return catalog.all();
  if (options.groups.length === 0) return [];
  try {
    return catalog.group(...options.groups);
  } catch (err) {
    refuse(options, `${(err as Error).message}. Run 'tolkien kinds'.`);
  }
}

/** A token as a line: kind, start, end and text, with tabs between. */
const line = (row: Row) =>
  `${row.kind}\t${row.start}\t${row.end}\t${JSON.stringify(row.text)}`;

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
    refuse(options, `No kind ${unknown}. Run 'tolkien kinds'.`);
  }
  const wanted = options.kinds.length > 0 ? new Set(options.kinds) : undefined;

  const tokens = lexer.tokens(text);
  const rows: Row[] = [];
  for (let index = 0; index < tokens.length; index++) {
    const kind = tokens.kind(index);
    if (options.skipSpace && SPACE.has(kind)) continue;
    if (wanted && !wanted.has(kind)) continue;
    rows.push(tokens.at(index) as Row);
  }

  if (options.output) {
    const body = options.json
      ? `${JSON.stringify(rows, null, 2)}\n`
      : rows.map((row) => `${line(row)}\n`).join("");
    writeFileSync(options.output, body);
    output(options, {
      json: () => ({
        success: true,
        source: label,
        output: options.output,
        count: rows.length,
      }),
      human: () => {
        success(`Lexed → ${options.output}`);
        console.log(dim(`  ${rows.length} tokens`));
      },
    });
    return;
  }

  if (options.json) {
    jsonOutput({
      success: true,
      source: label,
      count: rows.length,
      tokens: rows,
    });
  } else if (options.quiet || !process.stdout.isTTY) {
    // Into a pipe or a file the lines are the ones a program reads.
    await print(rows.map(line));
  } else {
    const width = Math.max(0, ...rows.map((row) => row.kind.length));
    await print(
      rows.map(
        (row) =>
          `${row.kind.padEnd(width)}  ${dim(`${row.start}..${row.end}`.padEnd(12))} ${JSON.stringify(row.text)}`,
      ),
    );
  }
}
