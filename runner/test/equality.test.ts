import { expect, test } from "bun:test";
import { compareWorkflows, findDuplicateKeys } from "../src/equality";

const a = `name: X
on:
  push:
    branches: [main]
  workflow_dispatch:
jobs:
  j:
    runs-on: ubuntu-latest
    steps:
      - run: |
          echo one
          echo two
`;

test("identical text is equal", () => expect(compareWorkflows(a, a).equal).toBe(true));
test("comments, key order, quoting and block lists do not matter", () => {
  const b = `# a comment
jobs:
  j:
    steps:
    - run: |
        echo one
        echo two
    runs-on: "ubuntu-latest"
'on':
  workflow_dispatch: {}
  push:
    branches:
    - main
name: X
`;
  expect(compareWorkflows(a, b)).toEqual({ equal: true, differences: [], duplicateKeys: [] });
});
test("null and {} are equal inside on:", () => {
  expect(compareWorkflows("on:\n  workflow_dispatch:\n", "on:\n  workflow_dispatch: {}\n").equal).toBe(true);
});
test("null and {} are NOT treated as equal outside on:", () => {
  expect(compareWorkflows("a:\n  b:\n", "a:\n  b: {}\n").equal).toBe(false);
});
test("a one character difference in a run script is a difference", () => {
  const r = compareWorkflows(a, a.replace("echo two", "echo twO"));
  expect(r.equal).toBe(false);
  expect(r.differences[0]).toContain("$.jobs.j.steps[0].run");
});
test("a trailing newline difference in a run script is a difference (byte for byte)", () => {
  const r = compareWorkflows("a:\n  - run: |\n      x\n", "a:\n  - run: |-\n      x\n");
  expect(r.equal).toBe(false);
});
test("an extra key is a difference", () => expect(compareWorkflows(a, a + "extra: 1\n").differences).toEqual(['$.extra: undefined vs 1']));
test("a duplicate key in the rendered file fails the comparison", () => {
  const r = compareWorkflows("a: 1\n", "a: 1\na: 1\n");
  expect(r.equal).toBe(false);
  expect(r.duplicateKeys).toEqual(["rendered line 2: a"]);
});

test("duplicate keys: same key in different list items is fine", () => {
  expect(findDuplicateKeys("steps:\n  - name: a\n    run: x\n  - name: b\n    run: y\n")).toEqual([]);
});
test("duplicate keys: same key in different mappings is fine", () => {
  expect(findDuplicateKeys("a:\n  name: x\nb:\n  name: y\n")).toEqual([]);
});
test("duplicate keys: a repeated key in one nested mapping is found", () => {
  expect(findDuplicateKeys("jobs:\n  j:\n    name: x\n    runs-on: a\n    name: y\n")).toEqual(["line 5: name"]);
});
test("duplicate keys: block scalar content is ignored", () => {
  expect(findDuplicateKeys("run: |\n  a: 1\n  a: 2\nother: 1\n")).toEqual([]);
});
test("duplicate keys: comments and the document marker are ignored", () => {
  expect(findDuplicateKeys("---\n# a: 1\na: 1\n# a: 2\n")).toEqual([]);
});
test("duplicate keys: quoted keys count", () => {
  expect(findDuplicateKeys("'on': 1\n'on': 2\n")).toEqual(["line 2: 'on'"]);
});
