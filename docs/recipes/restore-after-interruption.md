# Restore after interruption

Goal: restore after interruption.

`ash
memory dev resume
memory today
memory dev context --for generic
`

Expected output: a short status, report, or generated file path with an exact next command.

What happened: memory.cpp reconstructs recent files, commands, memories, and next steps.

Privacy note: data stays local unless you copy or attach it yourself.

Next step: run memory doctor if setup feels off.