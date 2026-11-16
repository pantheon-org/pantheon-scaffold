import { expect, test } from "bun:test";
import { pklProjectVersion } from "../src/release";

// The tests of the rest of the release tooling are the Rust tests of `scaffold-release` (docs/ts-tooling-test-mapping.md).
test("pklProjectVersion reads the one version line of the package block", () => {
  expect(pklProjectVersion(`package {\n  name = "x"\n  version = "1.2.3"\n}\n`)).toBe("1.2.3");
  expect(() => pklProjectVersion(`package {}\n`)).toThrow("no version");
  expect(() => pklProjectVersion(`version = "1.0.0"\nversion = "2.0.0"\n`)).toThrow("more than one");
});
