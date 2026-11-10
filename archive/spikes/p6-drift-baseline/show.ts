// Prints a readable summary of baseline.json. Usage: bun show.ts
const d = await Bun.file("baseline.json").json();
console.log("TOGGLES");
for (const [r, t] of Object.entries(d.toggles)) console.log(" ", r.padEnd(24), JSON.stringify(t));
console.log("SETTINGS");
for (const [r, s] of Object.entries(d.settings)) console.log(" ", r.padEnd(34), JSON.stringify(s));
console.log("RULESETS");
for (const [r, s] of Object.entries(d.rulesets)) console.log(" ", r, JSON.stringify(s));
console.log("SINGLE-REPO", d.files.filter((f: any) => f.class === "single-repo").map((f: any) => `${f.path} in ${f.versions.flat().join(",")}`));
console.log("PER-REPO groups", [...new Set(d.files.filter((f: any) => f.class === "per-repo").map((f: any) => f.path.split("/")[0]))].join(", "));
const ident = d.files.filter((f: any) => f.class === "identical");
console.log(`IDENTICAL (${ident.length}):`);
console.log(ident.map((f: any) => `  ${f.path} [${f.repos}/4]`).join("\n"));
