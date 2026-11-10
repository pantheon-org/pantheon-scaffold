// Prints drifted files with the repo groups per version, and the full bypass actors. Usage: bun show2.ts
const d = await Bun.file("baseline.json").json();
console.log("DRIFTED");
for (const f of d.files.filter((x: any) => x.class === "drifted")) console.log(`  ${f.path}: ${f.versions.map((v: string[]) => `[${v.join("+")}]`).join(" vs ")}`);
console.log("BYPASS");
for (const [r, rs] of Object.entries(d.rulesets) as [string, any[]][]) for (const x of rs) console.log(`  ${r}: ${x.name}: ${JSON.stringify(x.bypass_actors)}`);
