import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

// The rendered release-automerge.yml of this repository (presets/release-automerge.pkl): manual by default, a schedule only when the
// repository variable is true. `check scaffold.pkl` keeps the file equal to the render, so this reads the rendered file.
const text = readFileSync(join(import.meta.dir, "../../.github/workflows/release-automerge.yml"), "utf8");
const wf = Bun.YAML.parse(text) as { on: Record<string, unknown>; jobs: Record<string, { if?: string }> };

test("it can be run by hand and still has the schedule, which is switched by a repository variable", () => {
  expect(Object.keys(wf.on).sort()).toEqual(["schedule", "workflow_dispatch"]);
  expect(wf.jobs.automerge.if).toBe("github.event_name != 'schedule' || vars.RELEASE_AUTOMERGE_SCHEDULE == 'true'");
});

test("a run by hand is never gated by the variable", () => {
  const gate = (event: string, variable: string | undefined): boolean => event !== "schedule" || variable === "true";
  expect(gate("workflow_dispatch", undefined)).toBe(true);
  expect(gate("schedule", undefined)).toBe(false);
  expect(gate("schedule", "false")).toBe(false);
  expect(gate("schedule", "true")).toBe(true);
});
