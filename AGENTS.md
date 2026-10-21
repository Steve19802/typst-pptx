# Notes for coding agents

Working notes for agents on this repository. The plan is
`touying-pptx-exporter.md`; measurements and decisions are in
`docs/phase0-findings.md` (all phases, despite the name).

## Environment

- The agent runs in a **volatile container**: its home directory (including
  agent memory) and `/tmp` are wiped on restart. Only the project folder
  persists (a bind mount of the owner's folder). Keep anything that matters
  in the project: notes here or in `docs/`, scripts in `tools/`, results in
  `out/milestone/`. `out/scratch/` is for throwaway files (`make clean`
  deletes it).
- The agent container has no Docker and no Rust toolchain. Commands run in
  the devcontainer over SSH:

  ```sh
  ssh -F .devcontainer/ssh/agent/config typst-pptx-dev 'cd /workspaces/typst-pptx && make check'
  ```

  The key and SSH config live in `.devcontainer/ssh/agent/` (gitignored),
  never in `~/.ssh`. The repository is `/workspaces/typst-pptx` inside the
  devcontainer; the agent sees it under its own path.
- The devcontainer is rebuilt from `.devcontainer/Dockerfile`: its `/tmp`,
  `~/.cargo/bin` and other image paths are not persistent either. Its named
  volumes are (`target/`, cargo registry, `~/.cache`, NuGet). Tools that
  must survive go into the project (e.g. `make zigbuild` installs zig and
  cargo-zigbuild under `tools/release/`, gitignored).
- In shell chains, a `grep` that matches nothing exits 1 and stops `&&`;
  run checks whose output may be empty on their own.

## Workflow

- `make check` (fmt, clippy, tests), `make validate` (Open XML SDK,
  python-pptx, LibreOffice visual diff and text positions over the fixture
  decks; it uses the release binary, so `cargo build --release` first if
  running `tools/validate/run.sh` directly).
- Snapshots: `INSTA_UPDATE=always cargo nextest run --workspace`, then
  review the diff.
- Windows milestones: `make milestone PHASE=phaseN` builds the zip with
  `docs/milestones/phaseN-checklist.md`; the owner exports PDFs from the
  **PowerPoint desktop app** (`<name>-windows.pdf`) and copies the folder
  back; `make milestone-report DIR=out/milestone/...` compares them.
  `tools/validate/analysis/` has the scripts used to fit the text models.
  Record results in `docs/phase0-findings.md`.
- Commit per finished feature with the findings updated; the owner pushes.
- Releases: tag `vX.Y.Z` matching the workspace version (README "Releases");
  run the Release workflow manually first as a dry run.

## Division of work

- **tino** (the consumer) is integrated by a separate agent working in
  tino's repository. Don't edit tino. This repository owns the CLI contract
  in `docs/tino-integration.md`: requests for flags, messages or
  `report.json` fields come through the owner; update the doc and the CLI
  together, keep changes additive within 0.x, release.
