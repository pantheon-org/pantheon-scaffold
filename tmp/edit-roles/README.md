# edit-roles (07-11-2026)

One-off helper written in TypeScript on bun. It replaced six exact text fragments in two plans so that Thomas Roche is recorded as holding all three roles (package maintainer, pin-update owner, gate A owner).

Run on 07-11-2026 as `bun edit-roles.ts <pkl plan> <shared plan>`. It fails if a fragment does not match exactly once, so it cannot be rerun on the already edited plans.
