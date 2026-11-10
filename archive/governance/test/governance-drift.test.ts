import { expect, test } from "bun:test";
import { ISSUE_LABEL, ISSUE_TITLE, reconcileIssue, renderIssue, type Issues } from "../src/governance/drift";
import type { RepoFinding } from "../src/governance/live";

const DRIFT: RepoFinding[] = [
  { repo: "pantheon-adr", problems: ['ruleset "Protect main": enforcement: should be active, is disabled'], notes: ["the bypass actors were not compared: this token cannot see them (it is not an admin of the repository)"] },
  { repo: "pantheon-journal", problems: [], notes: ["the bypass actors were not compared: this token cannot see them (it is not an admin of the repository)"] },
];
const CLEAN: RepoFinding[] = DRIFT.map((f) => ({ ...f, problems: [] }));

type Fake = { issues: Issues; calls: string[]; open: { number: number; title: string; body: string }[] };
function fake(open: { number: number; title: string; body: string }[] = []): Fake {
  const f: Fake = { calls: [], open: [...open], issues: undefined as unknown as Issues };
  f.issues = {
    list: async () => f.open.map((i) => ({ ...i })),
    create: async (title, body) => {
      f.calls.push(`create ${title}`);
      f.open.push({ number: 7, title, body });
      return 7;
    },
    update: async (n, body) => void (f.calls.push(`update #${n}`), (f.open.find((i) => i.number === n)!.body = body)),
    comment: async (n, text) => void f.calls.push(`comment #${n}: ${text}`),
    close: async (n) => void (f.calls.push(`close #${n}`), (f.open = f.open.filter((i) => i.number !== n))),
  };
  return f;
}
const URL = "https://example.test/run/1";

test("the issue names the drift, the owner and the notes, and carries no date or run link", () => {
  const i = renderIssue(DRIFT, "thoroc");
  expect(i.title).toBe(ISSUE_TITLE);
  expect(i.body).toContain("1 of 2 governed repositories");
  expect(i.body).toContain("@thoroc owns this");
  expect(i.body).toContain("### pantheon-adr");
  expect(i.body).toContain('- ruleset "Protect main": enforcement: should be active, is disabled');
  expect(i.body).not.toContain("### pantheon-journal");
  expect(i.body.match(/the bypass actors were not compared/g)?.length).toBe(1);
  expect(i.body).not.toMatch(/\d{4}-\d{2}-\d{2}|https?:\/\//);
  expect(renderIssue(DRIFT, "thoroc").body).toBe(i.body);
  expect(ISSUE_LABEL).toBe("governance-drift");
});

test("drift and no issue: one is opened, once", async () => {
  const f = fake();
  const r = await reconcileIssue(f.issues, DRIFT, "thoroc", URL, false);
  expect(r).toMatchObject({ outcome: "created", number: 7 });
  expect(f.calls).toEqual([`create ${ISSUE_TITLE}`]);
  f.calls.length = 0;
  expect((await reconcileIssue(f.issues, DRIFT, "thoroc", URL, false)).outcome).toBe("unchanged");
  expect(f.calls).toEqual([]);
});

test("drift that changed updates the description and says so in a comment", async () => {
  const f = fake([{ number: 3, title: ISSUE_TITLE, body: "old\n" }]);
  const r = await reconcileIssue(f.issues, DRIFT, "thoroc", URL, false);
  expect(r).toMatchObject({ outcome: "updated", number: 3 });
  expect(f.calls).toEqual(["update #3", `comment #3: The drift changed in the latest run (${URL}). The description is up to date.`]);
});

test("no drift closes the issue with a comment, and with no issue does nothing", async () => {
  const f = fake([{ number: 3, title: ISSUE_TITLE, body: "x" }]);
  expect(await reconcileIssue(f.issues, CLEAN, "thoroc", URL, false)).toMatchObject({ outcome: "closed", number: 3 });
  expect(f.calls).toEqual([`comment #3: No drift in the latest run (${URL}). Closing.`, "close #3"]);
  f.calls.length = 0;
  expect((await reconcileIssue(f.issues, CLEAN, "thoroc", URL, false)).outcome).toBe("none");
  expect(f.calls).toEqual([]);
});

test("only an open issue with exactly the drift title is ever touched", async () => {
  const f = fake([
    { number: 1, title: "Governance drift", body: "someone else's" },
    { number: 2, title: `${ISSUE_TITLE} (copy)`, body: "x" },
    { number: 3, title: "Something else", body: "y" },
  ]);
  expect((await reconcileIssue(f.issues, CLEAN, "thoroc", URL, false)).outcome).toBe("none");
  expect(f.calls).toEqual([]);
  expect((await reconcileIssue(f.issues, DRIFT, "thoroc", URL, false)).outcome).toBe("created");
  expect(f.calls).toEqual([`create ${ISSUE_TITLE}`]);
  expect(f.open.filter((i) => i.number < 7).map((i) => i.body)).toEqual(["someone else's", "x", "y"]);
});

test("two open issues with the title are an error: it will not guess", async () => {
  const f = fake([
    { number: 1, title: ISSUE_TITLE, body: "a" },
    { number: 2, title: ISSUE_TITLE, body: "b" },
  ]);
  await expect(reconcileIssue(f.issues, DRIFT, "thoroc", URL, false)).rejects.toThrow("2 open issues carry the title");
  expect(f.calls).toEqual([]);
});

test("a dry run reports what it would do and writes nothing", async () => {
  const none = fake();
  expect(await reconcileIssue(none.issues, DRIFT, "thoroc", URL, true)).toMatchObject({ outcome: "created", number: undefined });
  const changed = fake([{ number: 3, title: ISSUE_TITLE, body: "old\n" }]);
  expect((await reconcileIssue(changed.issues, DRIFT, "thoroc", URL, true)).outcome).toBe("updated");
  const resolved = fake([{ number: 3, title: ISSUE_TITLE, body: "x" }]);
  expect((await reconcileIssue(resolved.issues, CLEAN, "thoroc", URL, true)).outcome).toBe("closed");
  expect([...none.calls, ...changed.calls, ...resolved.calls]).toEqual([]);
});

test("the comment works without a run link", async () => {
  const f = fake([{ number: 3, title: ISSUE_TITLE, body: "x" }]);
  await reconcileIssue(f.issues, CLEAN, "thoroc", undefined, false);
  expect(f.calls[0]).toBe("comment #3: No drift in the latest run. Closing.");
});
