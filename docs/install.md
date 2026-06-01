# Install

Here is what happens: you build the memory binary and put it on your PATH.

## Try this now

Unix:

```bash
./scripts/install.sh --dry-run
./scripts/install.sh
```

Windows PowerShell:

```powershell
./scripts/install.ps1 -DryRun
./scripts/install.ps1
```

## Output

```text
installed memory to ...
```

## What just happened?

The installer detects OS and architecture, tries the official GitHub release binary, verifies a checksum when one is available, and falls back to Cargo install from source. No account is created. Nothing is uploaded.

## First run

```bash
memory welcome
memory setup --developer --yes
memory today
memory dev context --for cursor
memory map --type evolution --output html
```

What just happened: setup created local config, `.memoryignore`, runtime folders, and a developer workspace.

## Without installing

```bash
cargo run -p memory-cli -- --help
```

## Demo scripts

After install, or from a checkout, run:

```bash
./scripts/demo.sh
```

PowerShell:

```powershell
./scripts/demo.ps1
```

The demo creates `.memory.cpp/demo-run/`, seeds sample memories, prints `memory dev morning`, generates Cursor context, and writes an HTML project map.

## Release artifacts

The release workflow builds Linux, macOS, and Windows archives and publishes `.sha256` checksum files. If you use a prebuilt binary, verify the checksum before putting `memory` on your PATH.

## Low-RAM / offline mode

```bash
memory setup --minimal --offline --yes
memory config set profile low-ram
memory embeddings set hash
```

This avoids heavy local model dependencies. The `fastembed`/`onnx` provider label currently maps to a lightweight local semantic backend; the repository does not bundle ONNX Runtime.

## Common mistakes

- If memory is not found, your PATH does not include the install directory.
- If Cargo is missing, install Rust first.
- If corporate proxy settings block downloads, use an offline Cargo cache or prebuilt binaries when available.
- To uninstall project data, run `memory privacy purge --yes` or remove `.memory.cpp/`.
