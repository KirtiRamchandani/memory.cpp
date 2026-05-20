# Automatic repo memory

Goal: automatic repo memory.

`ash
memory watch once --dry-run
memory git watch --once --dry-run
memory inbox stats
`

Expected output: a short status, report, or generated file path with an exact next command.

What happened: Watch proposes candidates without silently approving them.

Privacy note: data stays local unless you copy or attach it yourself.

Next step: run memory doctor if setup feels off.