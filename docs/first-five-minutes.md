# First five minutes

Goal: make your repo remember what happened, why it changed, and what to do next.

## 1. Set up local memory

```bash
memory setup --developer
```

Output:

```text
Welcome to memory.cpp
Database: .memory.cpp/memory.db
Next: memory dev morning
```

What just happened? memory.cpp created .memory.cpp/, a local SQLite database, starter config, and safe ignore rules.

## 2. Ask what it knows

```bash
memory what
memory where
```

## 3. Get your daily recap

```bash
memory dev morning
```

## 4. Create context for an assistant

```bash
memory dev context --for codex
```

## 5. Generate a map

```bash
memory show-map
```

Safe default: this writes a local HTML file. It does not upload your repo.
