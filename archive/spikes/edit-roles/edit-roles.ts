// One-off helper: replaces exact lines in the two plans and reports what changed.
import { readFileSync, writeFileSync } from "node:fs";

function replaceOnce(path: string, from: string, to: string): void {
  const text = readFileSync(path, "utf8");
  const count = text.split(from).length - 1;
  if (count !== 1) throw new Error(`${path}: expected 1 match, found ${count} for: ${from.slice(0, 60)}`);
  writeFileSync(path, text.replace(from, () => to));
  process.stdout.write(`ok ${path}: ${from.slice(0, 50)}\n`);
}

const [pkl, shared] = process.argv.slice(2);

replaceOnce(
  pkl,
  "Roles (decision 38): Thomas Roche holds package maintainer and pin-update owner, provisionally. The gate A owner is a different person whose name has not yet been supplied (placeholder, to be recorded before the spikes start). The timebox",
  "Roles (decision 38): Thomas Roche holds all three: package maintainer, pin-update owner and gate A owner (stated by the user on 07-11-2026). The timebox",
);
replaceOnce(pkl, "| The gate A owner's name and the start date are recorded |", "| The start date is recorded |");
replaceOnce(
  pkl,
  "38. Roles: Thomas Roche holds package maintainer and pin-update owner, and a different person holds gate A. Rejected: one person for all three (the builder would also decide whether to build), three named people (not yet agreed). The gate A owner's name was not supplied and is a placeholder. Revisit trigger: no name is recorded by the day the spikes would start, which blocks the spikes.",
  "38. Roles: Thomas Roche holds all three roles, package maintainer, pin-update owner and gate A owner, as the user stated on 07-11-2026. The interview answer on 07-11-2026 had been to separate gate A from the other two roles, and the user then said they are the gate A owner, so the decider and the builder are the same person. The user has accepted that. The risk is a builder scoring their own route, and a single point of failure for the package and its pins. Rejected: a different person for gate A, three named people (not yet agreed). Revisit trigger: a second person becomes available before the spikes start, who then co-signs the gate A scoring or takes the role.",
);
replaceOnce(pkl, "- Who is the gate A owner? The name was not supplied, and P.2 is not complete without it.\n", "");
replaceOnce(shared, "- Who is the gate A owner? The name was not supplied (task P.2 in the pkl plan).\n", "");
replaceOnce(shared, "The plan owner (named under P.2 of the pkl plan) decides", "The gate A owner (Thomas Roche, recorded under P.2 of the pkl plan) decides");
