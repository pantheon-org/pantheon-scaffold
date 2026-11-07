import { ContractError } from "./contract";

/** Flags that restrict what the consumer's pantheon.pkl may load or read (task 2.3, Trust). */
export const ALLOWED_MODULES = "pkl:,file:,package:,projectpackage:";
// package: lets a package module read its own assets, https: lets pkl download the package (spike S.1).
// There is no file: here, so a pantheon.pkl cannot read arbitrary files.
export const ALLOWED_RESOURCES = "package:,projectpackage:,https:";

export async function evalContractJson(configPath: string, cwd: string): Promise<unknown> {
  const proc = Bun.spawn(
    ["pkl", "eval", "-f", "json", "--allowed-modules", ALLOWED_MODULES, "--allowed-resources", ALLOWED_RESOURCES, configPath],
    { cwd, stdout: "pipe", stderr: "pipe" },
  );
  const [out, err, code] = await Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text(), proc.exited]);
  if (code !== 0) throw new ContractError(`pkl eval failed (exit ${code}):\n${err.trim()}`);
  try {
    return JSON.parse(out);
  } catch {
    throw new ContractError("pkl eval did not print JSON");
  }
}
