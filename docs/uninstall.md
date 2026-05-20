# Uninstall memory.cpp

Goal: remove the CLI and local memory files when you choose to.

## Remove the command

```bash
rm -f ~/.local/bin/memory
cargo uninstall memory-cli
```

On Windows, remove `memory.exe` from `%USERPROFILE%\.memory.cpp\bin` or from your Cargo bin folder.

## Delete local data

```bash
memory privacy purge --yes
```

What happened: memory.cpp deletes the local `.memory.cpp` folder for the current repo. Nothing is deleted from GitHub, cloud storage, or other repos.

## Undo editor attach

```bash
memory detach cursor --dry-run
memory detach cursor --yes
```

Privacy note: memory.cpp is local-first. Uninstalling the binary does not upload or sync anything.