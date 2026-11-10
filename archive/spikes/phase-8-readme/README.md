# Checks of docs/adopting.md (Phase 8, wave C)

`check-links.ts` checks that every file the adoption guide names exists, that every relative link resolves, and that none of the names on a private list appears in it. The list is not in this repository: it is read from `.context/private-names.txt` (untracked) or the file in `PRIVATE_NAMES_FILE`. Without it the name check is skipped and says so. Written 09-11-2026. Rerun with `bun tmp/phase-8-readme/check-links.ts` after any edit of the guide.
