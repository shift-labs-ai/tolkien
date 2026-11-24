import { catalog, Lexer } from "../index.js";
import type { OutputOptions } from "../utils/output.js";
import { bold, dim, output } from "../utils/output.js";

export async function kinds(options: OutputOptions): Promise<void> {
  // The kinds of the default lexer, without the one for what no matcher takes.
  const builtIn = new Lexer().kinds.filter((kind) => kind !== "OTHER");
  const entries = catalog.entries();
  output(options, {
    json: () => ({ builtIn, catalog: entries }),
    quiet: () => {
      for (const kind of builtIn) console.log(kind);
      for (const entry of entries) console.log(entry.kind);
    },
    human: () => {
      console.log();
      console.log(bold("Built-in kinds"), dim("always on, in the order tried"));
      console.log();
      for (const kind of builtIn) console.log(`  ${kind}`);
      const groups = [...new Set(entries.map((entry) => entry.group))];
      const width = Math.max(...entries.map((entry) => entry.kind.length));
      for (const group of groups) {
        console.log();
        console.log(bold(`Catalog: ${group}`), dim(`--group ${group}`));
        console.log();
        for (const entry of entries.filter((e) => e.group === group)) {
          console.log(`  ${entry.kind.padEnd(width)}  ${dim(entry.summary)}`);
        }
      }
      console.log();
    },
  });
}
