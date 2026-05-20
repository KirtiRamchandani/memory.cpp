# Upgrade memory.cpp

Goal: move to a newer release without changing your local memories.

```bash
./scripts/install.sh
memory doctor
memory config doctor
```

Windows:

```powershell
.\scripts\install.ps1
memory doctor
```

What happened: the installer tries the latest release binary first, then falls back to `cargo install` when needed. Your SQLite memory store stays in `.memory.cpp/memory.db`.

If a migration is needed later, `memory doctor` will show the exact next command.