import { validate, type Schema } from "./schema";

export type FileClass = "owned" | "seed" | "external";
export type ContractFile = { path: string; class: FileClass; mode: "0644" | "0755"; content?: string; removeOnDetach?: boolean };
export type Pin = { action: string; sha: string; version: string };
export type Contract = {
  schemaVersion: 2;
  files: ContractFile[];
  pins: Pin[];
  escapeHatch: { uses: number; constructs: number };
  /** Where the package was loaded from, so the runner can read its version. Absent for a local package. */
  packageUri?: string;
  /** The oldest runner that can render this package. */
  requires?: { runner: string };
  /** Builtins owned by Security: the runner warns when one is disabled. */
  securityBuiltins?: { name: string; enabled: boolean }[];
};

export const SUPPORTED_SCHEMA_VERSION = 2;

export class ContractError extends Error {}

/** Checks the schema version first, so an unknown version gets a clear message, then the full schema. */
export function parseContract(json: unknown, schema: Schema): Contract {
  const version = (json as { schemaVersion?: unknown } | null)?.schemaVersion;
  if (version !== SUPPORTED_SCHEMA_VERSION)
    throw new ContractError(`unsupported contract schemaVersion ${JSON.stringify(version)}, this runner supports ${SUPPORTED_SCHEMA_VERSION}`);
  const errors = validate(json, schema);
  if (errors.length) throw new ContractError(`contract does not match the schema:\n  ${errors.join("\n  ")}`);
  const contract = json as Contract;
  for (const f of contract.files) {
    if (f.class === "external" && f.content !== undefined) throw new ContractError(`${f.path}: an external file must not carry content`);
    if (f.class !== "external" && f.content === undefined) throw new ContractError(`${f.path}: a ${f.class} file needs content`);
    if (f.removeOnDetach && f.class !== "owned") throw new ContractError(`${f.path}: removeOnDetach is only allowed on owned files`);
  }
  const paths = contract.files.map((f) => f.path);
  const dup = paths.find((p, i) => paths.indexOf(p) !== i);
  if (dup) throw new ContractError(`${dup}: listed twice`);
  return contract;
}
