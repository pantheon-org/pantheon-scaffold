import type { RepoFinding } from "./live";

export const ISSUE_TITLE = "Governance drift: live rulesets differ from governance/desired";
export const ISSUE_LABEL = "governance-drift";

/** The open issues that carry the drift label. Replaceable, which is how the tests run without GitHub. */
export type Issues = {
  list(): Promise<{ number: number; title: string; body: string }[]>;
  create(title: string, body: string): Promise<number>;
  update(number: number, body: string): Promise<void>;
  comment(number: number, text: string): Promise<void>;
  close(number: number): Promise<void>;
};

export type Outcome = "none" | "created" | "updated" | "unchanged" | "closed";

/**
 * The issue for a set of findings. The body holds only the differences and the notes, no dates and no run links, so the same drift gives the
 * same body and a weekly run does not keep editing an issue that has not changed.
 */
export function renderIssue(findings: RepoFinding[], assignee: string): { title: string; body: string } {
  const drifted = findings.filter((f) => f.problems.length);
  const sections = drifted.map((f) => `### ${f.repo}\n\n${f.problems.map((p) => `- ${p}`).join("\n")}`);
  const notes = [...new Set(findings.flatMap((f) => f.notes))];
  const body = [
    `The live rulesets of ${drifted.length} of ${findings.length} governed repositories differ from \`governance/desired/\`. @${assignee} owns this.`,
    ...sections,
    notes.length ? `Notes:\n\n${notes.map((n) => `- ${n}`).join("\n")}` : "",
    "Compare and look at the live state with `bun runner/scripts/governance.ts check` (an admin token also compares the bypass actors). If the live state is right, change `governance/desired/` in a pull request. If the desired state is right, the owner applies it, one repository at a time (`governance/README.md`). The merge settings are not compared by the scheduled run: it cannot read them.",
    "This issue is opened, updated and closed by the scheduled workflow `governance-drift`. It closes itself when the drift is gone.",
  ].filter(Boolean);
  return { title: ISSUE_TITLE, body: body.join("\n\n") + "\n" };
}

/**
 * Brings the drift issue in line with the findings. Only an open issue with exactly the title above is ever touched. Drift and no issue:
 * one is opened. Drift and an issue whose body differs: the body is replaced and a comment says so. No drift and an issue: it gets a
 * comment and is closed. Anything else changes nothing. With `dryRun` nothing is written.
 */
export async function reconcileIssue(issues: Issues, findings: RepoFinding[], assignee: string, runUrl: string | undefined, dryRun: boolean): Promise<{ outcome: Outcome; number?: number; issue?: { title: string; body: string } }> {
  const open = (await issues.list()).filter((i) => i.title === ISSUE_TITLE);
  if (open.length > 1) throw new Error(`${open.length} open issues carry the title ${JSON.stringify(ISSUE_TITLE)}: the workflow will not guess which one to update`);
  const existing = open[0];
  const drifted = findings.some((f) => f.problems.length);
  const where = runUrl ? ` (${runUrl})` : "";
  if (!drifted) {
    if (!existing) return { outcome: "none" };
    if (!dryRun) {
      await issues.comment(existing.number, `No drift in the latest run${where}. Closing.`);
      await issues.close(existing.number);
    }
    return { outcome: "closed", number: existing.number };
  }
  const issue = renderIssue(findings, assignee);
  if (!existing) {
    const number = dryRun ? undefined : await issues.create(issue.title, issue.body);
    return { outcome: "created", number, issue };
  }
  if (existing.body === issue.body) return { outcome: "unchanged", number: existing.number, issue };
  if (!dryRun) {
    await issues.update(existing.number, issue.body);
    await issues.comment(existing.number, `The drift changed in the latest run${where}. The description is up to date.`);
  }
  return { outcome: "updated", number: existing.number, issue };
}
