# Troubleshooting install

## `memory` is not found

Add the install folder to `PATH`.

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Windows PowerShell:

```powershell
$env:Path = "$env:USERPROFILE\.memory.cpp\bin;$env:Path"
```

## Release binary is unavailable

Use the fallback path:

```bash
cargo install --path crates/memory-cli --force
```

## Corporate proxy

Download the release asset manually or use Cargo with your normal proxy configuration.

## First check

```bash
memory welcome
memory setup --developer --yes
memory doctor
```