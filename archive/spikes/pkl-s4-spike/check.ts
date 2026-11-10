// Parses pkl output with Bun.YAML and asserts the S.4 findings. Run from the spike folder.
import { $ } from "bun";

const render = async (file: string) => (await $`pkl eval ${file}`.text()).toString();
let failures = 0;
const check = (name: string, ok: boolean, detail = "") => {
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? "  " + detail : ""}`);
  if (!ok) failures++;
};

const dflt = await render("probe-omit-null.pkl");
const dfltDoc: any = Bun.YAML.parse(dflt);
check("omitNullProperties = true drops workflow_dispatch", !("workflow_dispatch" in dfltDoc.on));

const explicit = await render("probe-explicit-false.pkl");
const explicitDoc: any = Bun.YAML.parse(explicit);
check("omitNullProperties = false renders workflow_dispatch: null", explicitDoc.on.workflow_dispatch === null && "workflow_dispatch" in explicitDoc.on, JSON.stringify(explicitDoc.on));

const mapping = Bun.YAML.parse(await render("probe-mapping.pkl")) as any;
check("a Mapping null entry is dropped under the default renderer", !("workflow_dispatch" in mapping.on));

const other = Bun.YAML.parse(await render("probe-other-null.pkl")) as any;
check("omitNullProperties = false also emits unrelated null fields", other.jobs.build.name === null && "name" in other.jobs.build, JSON.stringify(other.jobs.build));

const cron = await render("probe-omit-null.pkl");
check("cron starting with * is quoted in the text", /cron: '\*\/15 \* \* \* \*'/.test(cron));
const cronDoc: any = Bun.YAML.parse(cron);
check("cron parses back as the same string", cronDoc.on.schedule[0].cron === "*/15 * * * *");

check("the on key is quoted in the text", /^'on':/m.test(cron));
check("the on key parses as the string 'on', not true", Object.keys(cronDoc).includes("on") && !Object.keys(cronDoc).includes("true"));

const probe = Bun.YAML.parse((await $`pkl eval -f yaml probe.pkl`.text()).toString()) as any;
check("unquoted cron 0 8 * * 1 parses as a string", probe.on.schedule[0].cron === "0 8 * * 1");

if (failures) { console.error(`${failures} failing`); process.exit(1); }
console.log("all passed");
