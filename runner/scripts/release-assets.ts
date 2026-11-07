// Steps of the release workflow that are code, not shell.
// usage: bun runner/scripts/release-assets.ts check-tag <tag>        prints the version if the tag matches pkl/PklProject
//        bun runner/scripts/release-assets.ts version                       prints the version in pkl/PklProject (a dry run has no tag)
//        bun runner/scripts/release-assets.ts checksums <dir> <version>   writes <dir>/checksums.txt after cross-checking the package files
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { assembleChecksums, checkTag, pklProjectVersion } from "../src/release";

const [cmd, a, b] = Bun.argv.slice(2);
const usage = (): never => {
  console.error("usage: release-assets.ts check-tag <tag> | version | checksums <dir> <version>");
  process.exit(2);
};

const projectText = (): string => readFileSync(join(import.meta.dir, "../../pkl/PklProject"), "utf8");

try {
  if (cmd === "version") {
    console.log(pklProjectVersion(projectText()));
  } else if (cmd === "check-tag" && a) {
    console.log(checkTag(a, projectText()));
  } else if (cmd === "checksums" && a && b) {
    const text = assembleChecksums(a, b);
    writeFileSync(join(a, "checksums.txt"), text);
    console.log(text.trimEnd());
  } else usage();
} catch (e) {
  console.error((e as Error).message);
  process.exit(1);
}
