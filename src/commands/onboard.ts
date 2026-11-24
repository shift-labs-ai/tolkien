import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { OutputOptions } from "../utils/output.js";
import { output, success } from "../utils/output.js";

const INSTRUCTIONS = `
<tolkien>
Use \`tolkien\` to split text into typed tokens: words, numbers, dates, times, email addresses, URLs, amounts of money, and with the catalog IP addresses, secrets, card and phone numbers and more.

<commands>
- \`tolkien <file>\` - tokens of a file, one per line: kind, start, end, text
- \`tolkien -t "some text"\` - tokens of a text
- \`cat file | tolkien -\` - tokens of stdin
- \`tolkien <file> --catalog -k IPV4,CARD_NUMBER\` - only tokens of these kinds
- \`tolkien kinds\` - list every kind and the groups of the catalog
</commands>

<rules>
- Use \`--json\` to get structured output for parsing
- Use \`-g secrets\` (or network, finance, personal, technical, text) to add one group of the catalog, \`-c\` for all of it
- A kind is a shape, not a proof: check a CARD_NUMBER or an IBAN with its checksum
- Start and end are indexes into the text as a JavaScript string
</rules>
</tolkien>
`.trim();

const MARKER = "<tolkien>";

export async function onboard(options: OutputOptions): Promise<void> {
  const cwd = process.cwd();
  const claudeMd = join(cwd, "CLAUDE.md");
  const agentsMd = join(cwd, "AGENTS.md");
  const targetFile =
    !existsSync(claudeMd) && existsSync(agentsMd) ? agentsMd : claudeMd;

  const existingContent = existsSync(targetFile)
    ? readFileSync(targetFile, "utf-8")
    : "";

  if (existingContent.includes(MARKER)) {
    output(options, {
      json: () => ({
        success: true,
        file: targetFile,
        message: "already_onboarded",
      }),
      human: () => success(`Already onboarded (${targetFile})`),
    });
    return;
  }

  writeFileSync(
    targetFile,
    existingContent
      ? `${existingContent.trimEnd()}\n\n${INSTRUCTIONS}\n`
      : `${INSTRUCTIONS}\n`,
  );

  output(options, {
    json: () => ({ success: true, file: targetFile }),
    human: () => success(`Added tolkien instructions to ${targetFile}`),
  });
}
