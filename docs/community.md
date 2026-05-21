# Community

`memory.cpp` should be easy to contribute to without becoming a giant platform.

## Good first contributions

- Improve command examples.
- Add a recipe that uses existing commands.
- Add tests for an existing command.
- Improve redaction fixtures.
- Improve map/context output clarity.
- Improve install troubleshooting.
- Add a small cross-platform smoke check.

## Project taste

Prefer:

- local-first behavior
- low dependency count
- small schemas
- exact commands in output
- honest maturity labels
- examples that run from a clean checkout

Avoid:

- hosted SaaS requirements
- enterprise/team sync in the core path
- plugin marketplaces
- mobile/AppSec/fuzzing packs before the developer core is excellent
- automatic secret storage
- large ML dependencies by default

## Templates

Use the issue and PR templates under `.github/`.

Adapter, integration, and recipe authors should include:

- goal
- commands
- expected output
- privacy note
- tests or smoke command