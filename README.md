# typst-pptx

Exports Typst/Touying 0.8 decks to PowerPoint files whose text, shapes,
tables, pictures, notes and click steps stay editable. See
`touying-pptx-exporter.md` for the design.

## Development environment

Everything (Rust, typst CLI, LibreOffice, .NET, Python/uv) runs in the
devcontainer, driven by the [devcontainer CLI](https://github.com/devcontainers/cli):

```sh
make devcontainer   # or: devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . make setup
devcontainer exec --workspace-folder . make check
```

Cargo caches and `target/` live in named Docker volumes (`typst-pptx-*`).
Files meant to be read from outside the container go to `out/`.

### Checks

| Command | What it does |
|---|---|
| `make check` | format, clippy, unit and snapshot tests (`cargo insta review` after intended changes) |
| `make validate` | exports the fixture decks, runs the Open XML SDK validator and python-pptx, and compares LibreOffice's rendering with Typst's PDF (output in `out/validate/`) |
| `make milestone PHASE=phase0` | zip for the Windows check in PowerPoint (`docs/milestones/`) |

Tests run offline: fonts come from `fixtures/fonts/`, packages from
`fixtures/packages/` (`--package-path`).

### SSH access for the coding agent

The container runs a non-root sshd on port 2222. It is not published on the
host: the agent reaches it by name over a shared Docker network.

1. Put the agent's public key in `.devcontainer/ssh/authorized_keys`.
   The sshd host key is generated in `.devcontainer/ssh/host/` on first start
   and kept across rebuilds. The agent keeps its own key pair, ssh config and
   known_hosts in `.devcontainer/ssh/agent/`, so they survive agent-container
   restarts. The whole `.devcontainer/ssh/` folder is gitignored.
2. Start the container on the agent's network, from the host:
   ```sh
   make devcontainer                  # network "typst-pptx", created if missing
   make devcontainer NETWORK=<name>   # another network
   ```
   The agent container must be attached to the same network
   (`docker network connect typst-pptx <agent-container>`). The target always
   recreates the container, so it also applies network or image changes.
3. From the agent:
   `ssh -F .devcontainer/ssh/agent/config typst-pptx-dev 'cd /workspaces/typst-pptx && make doctor'`.

sshd is started by `postStartCommand`, so after a host reboot run
`make devcontainer` again (a plain `docker start` doesn't start it).

## Licence

Apache-2.0, see `LICENSE` and `NOTICE`.
