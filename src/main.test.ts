import { describe, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const main = new URL("./main.ts", import.meta.url).pathname;

/** Runs the command with `args` in `cwd`, with `stdin` piped in. */
function tolkien(args: string[], stdin = "", cwd = process.cwd()) {
  const run = Bun.spawnSync([process.execPath, main, ...args], {
    stdin: Buffer.from(stdin),
    cwd,
  });
  return {
    code: run.exitCode,
    out: run.stdout.toString(),
    err: run.stderr.toString(),
  };
}

describe("the tolkien command", () => {
  test("prints a token a line: kind, start, end and text", () => {
    const { code, out } = tolkien(["-t", "Mail dana@acme.com", "-s"]);
    expect(code).toBe(0);
    expect(out).toBe(
      'WORD\t0\t4\t"Mail"\nLIKE_EMAIL\t5\t18\t"dana@acme.com"\n',
    );
  });

  test("reads a file, stdin with -, and stdin with no source", () => {
    const dir = mkdtempSync(join(tmpdir(), "tolkien-"));
    const file = join(dir, "notes.txt");
    writeFileSync(file, "a 1");
    const expected = 'WORD\t0\t1\t"a"\nLIKE_NUMBER\t2\t3\t"1"\n';
    expect(tolkien([file, "-s"]).out).toBe(expected);
    expect(tolkien(["lex", file, "-s"]).out).toBe(expected);
    expect(tolkien(["-", "-s"], "a 1").out).toBe(expected);
    expect(tolkien(["-s"], "a 1").out).toBe(expected);
  });

  test("adds the catalog or groups of it, and keeps the kinds asked for", () => {
    const text = "ssh 192.168.1.20, card 4111 1111 1111 1111";
    expect(tolkien(["-t", text, "-k", "LIKE_NUMBER", "-s"]).out).toContain(
      '"192.168.1.20"',
    );
    expect(tolkien(["-t", text, "-c", "-k", "IPV4,CARD_NUMBER"]).out).toBe(
      'IPV4\t4\t16\t"192.168.1.20"\nCARD_NUMBER\t23\t42\t"4111 1111 1111 1111"\n',
    );
    expect(tolkien(["-t", text, "-g", "network", "-k", "IPV4"]).out).toBe(
      'IPV4\t4\t16\t"192.168.1.20"\n',
    );
  });

  test("gives JSON with --json", () => {
    const { out } = tolkien(
      ["-", "-g", "network", "-k", "IPV4", "--json"],
      "at 8.8.8.8",
    );
    expect(JSON.parse(out)).toEqual({
      success: true,
      source: "stdin",
      count: 1,
      tokens: [{ kind: "IPV4", start: 3, end: 10, text: "8.8.8.8" }],
    });
  });

  test("writes to a file with --output", () => {
    const dir = mkdtempSync(join(tmpdir(), "tolkien-"));
    const file = join(dir, "tokens.tsv");
    const { out } = tolkien(["-t", "a b", "-s", "-o", file, "--json"]);
    expect(JSON.parse(out)).toEqual({
      success: true,
      source: "text",
      output: file,
      count: 2,
    });
    expect(JSON.parse(readFileSync(file, "utf-8"))).toHaveLength(2);
    tolkien(["-t", "a b", "-s", "-o", file]);
    expect(readFileSync(file, "utf-8")).toBe(
      'WORD\t0\t1\t"a"\nWORD\t2\t3\t"b"\n',
    );
  });

  test("refuses a mistake with a reason and a status", () => {
    const missing = tolkien(["no-such-file.txt"]);
    expect(missing.code).toBe(1);
    expect(missing.err).toContain("File not found: no-such-file.txt");
    for (const [args, reason] of [
      [["-t", "x", "-k", "NOPE"], "No kind NOPE"],
      [["-t", "x", "-g", "nope"], 'the catalog has no "nope"'],
      [["-t", "x", "file.txt"], "Give a source or --text, not both."],
    ] as const) {
      const { code, err } = tolkien([...args]);
      expect(code).toBe(2);
      expect(err).toContain(reason);
    }
    const asJson = tolkien(["-t", "x", "-k", "NOPE", "--json"]);
    expect(JSON.parse(asJson.out)).toEqual({
      success: false,
      error: "No kind NOPE. Run 'tolkien kinds'.",
    });
  });

  test("lists the kinds", () => {
    const listed = JSON.parse(tolkien(["kinds", "--json"]).out);
    expect(listed.builtIn[0]).toBe("DATE_ISO_8601");
    expect(listed.builtIn).not.toContain("OTHER");
    expect(listed.catalog.length).toBeGreaterThan(30);
    const names = tolkien(["kinds", "-q"]).out.trim().split("\n");
    expect(names).toHaveLength(listed.builtIn.length + listed.catalog.length);
    expect(tolkien(["kinds"]).out).toContain("Catalog: secrets");
  });

  test("adds its instructions to a project once", () => {
    const dir = mkdtempSync(join(tmpdir(), "tolkien-"));
    writeFileSync(join(dir, "AGENTS.md"), "# Project\n");
    const first = JSON.parse(tolkien(["onboard", "--json"], "", dir).out);
    expect(first.success).toBe(true);
    const written = readFileSync(first.file, "utf-8");
    expect(first.file.endsWith("AGENTS.md")).toBe(true);
    expect(written).toStartWith("# Project\n\n<tolkien>");
    const again = JSON.parse(tolkien(["onboard", "--json"], "", dir).out);
    expect(again.message).toBe("already_onboarded");
    expect(readFileSync(first.file, "utf-8")).toBe(written);
  });

  test("prints its version and its help", () => {
    expect(tolkien(["-V"]).out).toMatch(/^tolkien \d+\.\d+\.\d+/);
    expect(tolkien(["--help"]).out).toContain("Usage: tolkien");
  });
});
