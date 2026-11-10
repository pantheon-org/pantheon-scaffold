// Read-only: for each pinned uses line with a comment in the real workflows, ask GitHub whether the SHA is the commit
// of the tag named in the comment. Non-tag comments are reported as unverifiable. Usage: bun verify-tags.ts [owner/repo ...]
import { $ } from "bun";
import { scan } from "./versions";

const repos = Bun.argv.slice(2);
if (repos.length === 0) repos.push("pantheon-org/pantheon-adr", "pantheon-org/pantheon-skill-auditor");
const TAG = /^v?\d+(\.\d+){0,2}([-.][0-9A-Za-z.]+)?$/;

const seen = new Map<string, { repo: string; sha: string; comment: string }>();
for (const repo of repos) {
  const names = (await $`env -u GH_TOKEN gh api repos/${repo}/contents/.github/workflows --jq '.[].name'`.text()).trim().split("\n");
  for (const n of names) {
    const text = Buffer.from((await $`env -u GH_TOKEN gh api repos/${repo}/contents/.github/workflows/${n} --jq .content`.text()).trim(), "base64").toString("utf8");
    for (const l of scan(text)) {
      const g = l.match?.groups;
      const comment = g?.tail.trim().replace(/^#\s*/, "");
      if (g && comment) {
        const [owner, name] = g.action.split("/");
        seen.set(`${owner}/${name}@${g.sha}#${comment}`, { repo: `${owner}/${name}`, sha: g.sha, comment });
      }
    }
  }
}

let bad = 0;
for (const { repo, sha, comment } of seen.values()) {
  if (!TAG.test(comment)) {
    console.log(`UNVERIFIABLE  ${repo}@${sha.slice(0, 12)}  comment "${comment}" is not a tag`);
    continue;
  }
  const r = await $`env -u GH_TOKEN gh api repos/${repo}/commits/${comment} --jq .sha`.quiet().nothrow();
  const actual = r.stdout.toString().trim();
  if (r.exitCode !== 0) {
    console.log(`NO TAG        ${repo}  tag ${comment} not found (${r.stderr.toString().trim().slice(0, 60)})`);
    bad++;
  } else if (actual === sha) {
    console.log(`OK            ${repo}  ${comment} -> ${sha.slice(0, 12)}`);
  } else {
    console.log(`MISMATCH      ${repo}  ${comment} is ${actual.slice(0, 12)} but the pin is ${sha.slice(0, 12)}`);
    bad++;
  }
}
console.log(`${seen.size} distinct pin+comment pairs checked, ${bad} problems`);
process.exit(bad ? 1 : 0);
