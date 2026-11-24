import chalk from "chalk";

export interface OutputOptions {
  json?: boolean;
  quiet?: boolean;
}

export const success = (msg: string) => console.log(chalk.green("✓"), msg);
export const error = (msg: string) => console.error(chalk.red("✗"), msg);

export const bold = (s: string) => chalk.bold(s);
export const dim = (s: string) => chalk.dim(s);

export function jsonOutput(data: object): void {
  console.log(JSON.stringify(data, null, 2));
}

/** Prints in the form the options ask for: JSON, bare, or for a person. */
export function output(
  options: OutputOptions,
  handlers: {
    json?: () => object;
    quiet?: () => void;
    human: () => void;
  },
): void {
  if (options.json && handlers.json) {
    jsonOutput(handlers.json());
  } else if (options.quiet && handlers.quiet) {
    handlers.quiet();
  } else {
    handlers.human();
  }
}
