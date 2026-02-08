# Eval Fixtures

Eval fixtures are JSON files used by `commit-wizard-eval` to measure type/scope accuracy.

Each `.json` file can contain either a single fixture object or an array of fixtures.

Schema:

```json
{
  "id": "string",
  "description": "optional string",
  "files": [
    {
      "path": "relative/path/to/file.rs",
      "diff": "+added line\n-removed line\n"
    }
  ],
  "expected": {
    "type": "feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert",
    "scope": "optional scope string",
    "subject": "optional subject string"
  }
}
```

Notes:
- `diff` should be a unified-style snippet with `+` and `-` markers.
- `expected` fields are optional; metrics are calculated only when provided.
