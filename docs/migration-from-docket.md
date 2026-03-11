# Migrating from docket to bmo

This guide walks through migrating an existing docket project to bmo. The migration
preserves issues, comments, labels, and relations. Issue IDs are remapped from
`DKT-N` to `BMO-N` using sequential numbering.

---

## Prerequisites

- `docket` CLI installed and your project initialized (`.docket/` directory present)
- `bmo` binary available on your `PATH` (see [installation](index.md))
- The target directory for bmo is empty or does not yet have a `.bmo/` directory

---

## Step 1: Export from docket

In your existing docket project directory, run:

```sh
docket export -f my-export.json
```

This writes a JSON file containing all issues, comments, labels, and relations.
Verify the file was created:

```sh
ls -lh my-export.json
```

---

## Step 2: Initialize bmo

Navigate to the directory where you want to run bmo (this can be the same directory
or a new one), then initialize a fresh bmo project:

```sh
cd my-project
bmo init
```

This creates a `.bmo/` directory containing the SQLite database.

---

## Step 3: Import the docket export

Run the import command with the `--from-docket` flag pointing to the file you exported
in Step 1:

```sh
bmo import --from-docket my-export.json
```

The `--from-docket` flag tells bmo to parse the docket export format (string IDs like
`DKT-1`) rather than a native bmo export bundle.

On success you will see output like:

```
Imported 12 issue(s) and 5 comment(s) (from docket format)
```

Any data that could not be imported (e.g. comments referencing issues that were not
in the export) will be reported as warnings appended to the output line.

---

## Step 4: Verify the migration

List all issues to confirm they were imported:

```sh
bmo issue list --all
```

The `--all` flag includes issues in every status (backlog, todo, in-progress, done).

Spot-check a specific issue:

```sh
bmo issue show BMO-1
```

---

## What gets migrated

| Data                | Migrated |
|---------------------|----------|
| Issues              | Yes      |
| Issue descriptions  | Yes      |
| Status              | Yes      |
| Priority            | Yes      |
| Kind                | Yes      |
| Assignee            | Yes      |
| Labels (on issues)  | Yes      |
| Comments            | Yes      |
| Relations           | Yes      |
| Labels (catalog)    | Yes      |
| File path refs      | Yes      |
| Activity log        | Yes      |

---

## What changes during migration

**Issue ID prefix** — docket uses `DKT-N` prefixes; bmo uses `BMO-N`. IDs are
reassigned sequentially starting at 1 in the order issues appear in the export file.
Example: `DKT-42` may become `BMO-1` if it is the first issue in the export.

**Assignee normalization** — docket stores an empty string `""` for unassigned issues;
bmo stores `null`. The importer handles this automatically.

**Priority** — docket supports a `critical` priority level. bmo maps this to `critical`
as well; if an unrecognized priority is encountered it falls back to `none`.

**Relation types** — bmo supports three relation types: `blocks`, `depends-on`, and
`relates-to`. Docket relation types are matched by name (case-insensitive). If an
unrecognized relation type is encountered it falls back to `relates-to`.

---

## Known limitations

- **Parent links may be dropped** — if a parent issue was not included in the export
  (e.g. you exported only a subset of issues), child issues will be imported without
  a parent reference. Re-link them manually with `bmo issue edit BMO-N --parent BMO-M`.

- **Activity log entries are best-effort** — docket's activity records are imported
  when present in the export, but the mapping is approximate. Docket's `field_changed`,
  `old_value`, `new_value`, and `changed_by` fields are translated to bmo's activity
  format. Entries referencing issues not in the export are silently skipped.

- **File attachments are path references only** — bmo stores file paths as strings.
  The paths from the docket export are preserved as-is, but no files are copied.
  Update paths if the project has moved.

- **Label color codes** — label colors are imported when present in the docket export.
  If a label already exists in bmo with a different color, the existing color is kept.
