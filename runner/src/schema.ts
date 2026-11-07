// A small JSON Schema validator for the subset contract.schema.json uses: type, const, enum, required, properties,
// additionalProperties (boolean), items, pattern, minLength and minimum. No dependencies, by design (task 2.3).
export type Schema = {
  type?: string;
  const?: unknown;
  enum?: unknown[];
  required?: string[];
  properties?: Record<string, Schema>;
  additionalProperties?: boolean;
  items?: Schema;
  pattern?: string;
  minLength?: number;
  minimum?: number;
};

const typeOf = (v: unknown): string =>
  v === null ? "null" : Array.isArray(v) ? "array" : Number.isInteger(v) ? "integer" : typeof v;

export function validate(value: unknown, schema: Schema, path = "#"): string[] {
  const errors: string[] = [];
  if (schema.const !== undefined && value !== schema.const) errors.push(`${path}: must be ${JSON.stringify(schema.const)}`);
  if (schema.enum && !schema.enum.includes(value)) errors.push(`${path}: must be one of ${JSON.stringify(schema.enum)}`);
  if (schema.type) {
    const actual = typeOf(value);
    const ok = actual === schema.type || (schema.type === "number" && actual === "integer");
    if (!ok) {
      errors.push(`${path}: must be ${schema.type}, got ${actual}`);
      return errors;
    }
  }
  if (typeof value === "string") {
    if (schema.minLength !== undefined && value.length < schema.minLength) errors.push(`${path}: shorter than ${schema.minLength}`);
    if (schema.pattern && !new RegExp(schema.pattern).test(value)) errors.push(`${path}: does not match ${schema.pattern}`);
  }
  if (typeof value === "number" && schema.minimum !== undefined && value < schema.minimum) errors.push(`${path}: below ${schema.minimum}`);
  if (Array.isArray(value) && schema.items) value.forEach((item, i) => errors.push(...validate(item, schema.items!, `${path}/${i}`)));
  if (value && typeof value === "object" && !Array.isArray(value)) {
    const obj = value as Record<string, unknown>;
    for (const key of schema.required ?? []) if (!(key in obj)) errors.push(`${path}: missing ${key}`);
    for (const [key, sub] of Object.entries(schema.properties ?? {})) if (key in obj) errors.push(...validate(obj[key], sub, `${path}/${key}`));
    if (schema.additionalProperties === false)
      for (const key of Object.keys(obj)) if (!(schema.properties && key in schema.properties)) errors.push(`${path}: unexpected ${key}`);
  }
  return errors;
}
