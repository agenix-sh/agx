# AGX Examples

These examples demonstrate the new PLAN workflow. Each `PLAN add` call can read STDIN when piped so the planner can inspect representative data.

## 1. Remove duplicate lines

```bash
agx PLAN new
cat test.txt | agx PLAN add "remove duplicates"
agx PLAN preview
```

Preview output:

```json
{
  "status": "ok",
  "plan": {
    "plan": [
      {"cmd": "sort"},
      {"cmd": "uniq"}
    ]
  }
}
```

## 2. Dedupe CSV rows by first three columns

```bash
agx PLAN new
cat data.csv | agx PLAN add "dedupe rows based on the first three CSV columns"
agx PLAN preview
```

Typical normalized plan:

```json
{
  "plan": [
    {"cmd": "cut", "args": ["-d", ",", "-f1-3"]},
    {"cmd": "sort"},
    {"cmd": "uniq"}
  ]
}
```

## 3. Filter JSON using jq

```bash
agx PLAN new
cat data.json | agx PLAN add "extract id and name fields from each JSON object"
agx PLAN preview
```

Possible plan snippet:

```json
{
  "plan": [
    {"cmd": "jq", "args": [".[] | {id, name}"]}
  ]
}
```
