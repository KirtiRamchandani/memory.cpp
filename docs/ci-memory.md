# CI memory

CI memory is intentionally small: it remembers failures and previous fixes so developers can recover faster.

```bash
memory ci ingest ./ci.log
memory ci explain-failure
memory ci report --output .memory.cpp/reports/ci.md
```

Expected output:

```text
ingested 3 CI failure memory item(s)
CI failure explanation:
  - cargo fmt failed on newline style
```

What just happened: memory.cpp parsed error-looking CI lines, stored them as local bug memories, and can now recall similar failures.