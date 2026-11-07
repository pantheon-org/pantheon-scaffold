import { expect, test } from "bun:test";
import { validate, type Schema } from "../src/schema";

const s: Schema = {
  type: "object",
  required: ["a"],
  additionalProperties: false,
  properties: {
    a: { type: "integer", minimum: 1 },
    b: { enum: ["x", "y"] },
    c: { type: "string", pattern: "^[0-9a-f]{4}$", minLength: 4 },
    d: { type: "array", items: { type: "string" } },
    v: { const: 1 },
  },
};

test("accepts a valid value", () => expect(validate({ a: 1, b: "x", c: "ab12", d: ["z"], v: 1 }, s)).toEqual([]));
test("reports a missing required key", () => expect(validate({}, s)).toEqual(["#: missing a"]));
test("reports an unexpected key", () => expect(validate({ a: 1, z: 1 }, s)).toEqual(["#: unexpected z"]));
test("reports a wrong type with its path", () => expect(validate({ a: "1" }, s)).toEqual(["#/a: must be integer, got string"]));
test("a float is not an integer", () => expect(validate({ a: 1.5 }, s)).toEqual(["#/a: must be integer, got number"]));
test("reports a minimum", () => expect(validate({ a: 0 }, s)).toEqual(["#/a: below 1"]));
test("reports an enum miss", () => expect(validate({ a: 1, b: "q" }, s)[0]).toContain("#/b: must be one of"));
test("reports a pattern miss", () => expect(validate({ a: 1, c: "zzzz" }, s)[0]).toContain("#/c: does not match"));
test("reports an item error with an index", () => expect(validate({ a: 1, d: ["ok", 3] }, s)).toEqual(["#/d/1: must be string, got integer"]));
test("reports a const miss", () => expect(validate({ a: 1, v: 2 }, s)).toEqual(["#/v: must be 1"]));
