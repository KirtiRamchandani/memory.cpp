# Terminal memory

Terminal memory is opt-in. It helps answer questions like "how did I run tests?"

## Enable

```bash
memory terminal enable
```

## Record a command

```bash
memory terminal record --command "cargo test" --exit-code 0
```

## Recall

```bash
memory terminal commands
memory terminal last-error
memory terminal search "run tests"
```

Safe default: commands are not recorded unless you enable or record them.
