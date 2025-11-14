# AGX Examples

This document lists example AGX invocations and the kinds of plans they are expected to produce.

## 1. Remove duplicate lines (text)

```bash
cat test.txt | agx "remove duplicates" > out.txt
```

Typical plan:

```json
{"plan":[{"cmd":"sort"},{"cmd":"uniq"}]}
```

## 2. Dedupe CSV rows by first three columns

```bash
cat data.csv | agx "dedupe rows based on the first three CSV columns" > out.csv
```

Typical plan (once argument support is used):

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
cat data.json | agx "extract id and name fields from each JSON object" > out.json
```

Typical plan:

```json
{
  "plan": [
    {"cmd": "jq", "args": [".[] | {id, name}"]}
  ]
}
```

