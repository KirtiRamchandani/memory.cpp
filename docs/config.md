# Config

Project config lives beside the database:

```text
.memory.cpp/memory-config.json
```

Use setup profiles for now:

```bash
memory setup --minimal
memory setup --developer
memory setup --ai-coding
memory setup --private
memory setup --offline
```

Planned commands:

```bash
memory config show
memory config set profile developer
memory config doctor
```
