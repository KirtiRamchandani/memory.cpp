# Automatic repo memory watch

`memory watch` coordinates lightweight local observation. It does not upload anything and it writes candidates first.

```bash
memory watch once --dry-run
memory watch status
memory watch pause
memory watch resume
```

What it watches:

- Git branch and commit changes
- README and docs changes
- dependency files
- test files
- TODO/FIXME movement
- terminal command memory when enabled

What just happened: `watch once --dry-run` reports candidate memories it would create, but stores nothing.