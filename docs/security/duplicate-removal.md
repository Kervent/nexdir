# Removing duplicate copies

The duplicate finder (*Tools → Duplicate files*) only **reads** files. Removing copies is a
separate step that the user starts and confirms. Since 0.4.3 the rule *"at least one copy of
every group always remains"* is enforced by the backend, so a modified or direct IPC call cannot
bypass it. Code: `src-tauri/src/operations/dedupe.rs`, command `recycle_duplicates`.

## What the frontend sends

For every group with a selection: the group's size and full-content BLAKE3 hash from the scan,
the paths to **keep** (the unselected files) and the paths to **remove**. All paths go through
the normal path validation (`security::validate_path`).

## What the backend checks

Per group, before anything is removed:

| Check | Result if it fails |
|---|---|
| The hash is a valid BLAKE3 hash | whole group refused |
| At least one path is marked to keep | whole group refused ("every copy was selected") |
| No path is both kept and removed (case-insensitive) | whole group refused |
| One kept file can be opened with **read-only sharing**, is a regular file (not a link, junction or folder), has the scanned size and its full content hashes to the scanned hash | the next kept file is tried; if none qualifies, the whole group is refused |
| Each file to remove is on a drive with a Recycle Bin | that file is skipped |
| Each file to remove still exists | skipped ("already removed or moved") |
| Each file to remove is a regular file, not a link or junction | skipped |
| Each file to remove is not the kept file under another name (same volume and file index: hard link, 8.3 short name, other spelling) | skipped |
| Each file to remove still has the scanned size and its full content the scanned hash | skipped ("changed since the scan") |

Only files that pass are sent to the Recycle Bin, in one shell operation per group. While that
runs:

- the verified **kept** copy stays open without write, rename or delete sharing — no other
  program (and no other Nexdir request) can change, move or delete it;
- the files being removed stay open without write sharing, so their content cannot change
  between the hash check and the Recycle Bin call (rename/delete sharing is granted, because the
  Recycle Bin moves them);
- removal requests are serialised by a process-wide lock, so two requests that each keep "the
  other" copy cannot both succeed.

Afterwards each path is checked; files still present are reported as not removed. Nothing is
ever deleted permanently — Windows asks before deleting a file too large for the Recycle Bin.

## Scanner changes

Hard links (one file with several names) are no longer reported as duplicates: while hashing,
the scanner reads each file's identity from the handle it opened anyway and skips further names
of a file already in the same size group (so they are not read twice either). Links and
junctions were already never followed. Cost: see `docs/development/performance.md`.

## Interface

- The *Move to Recycle Bin* button stays disabled while every copy of a group is selected, and a
  warning names the affected groups (as before).
- The context menu inside the duplicates view no longer contains the generic *Move to Recycle
  Bin* entry, so removal there always goes through `recycle_duplicates`.
- Files that were skipped are listed in a report dialog with the reason.

## Limits

- The Recycle Bin call works on paths. Between the check and the call another program could
  rename a verified file (rename sharing must be granted so the Recycle Bin can move it) and put
  a different file at the same path; that file would then go to the Recycle Bin, from where it
  can be restored. The kept copy cannot be affected — it is locked throughout.
- Outside the duplicates view, *Move to Recycle Bin* remains an ordinary file operation: a user
  can still delete every copy of a file one by one there, as in Explorer.
- Verification re-reads the content of every file involved, so removing many large copies takes
  about as long as hashing them did during the scan.
- The tests for symbolic links skip themselves when the account cannot create symbolic links
  (no Developer Mode or administrator rights); junctions are tested instead.

## Tests

`cargo test --manifest-path src-tauri/Cargo.toml dedupe` — synthetic files in temporary folders
and a stand-in Recycle Bin (a folder): two and five copies, every copy selected, a path both kept
and removed, kept or removed copies changed (same and different size), already deleted files,
missing kept copy with fallback to a second kept copy, invalid groups (bad hash, wrong hash,
wrong size, empty request), hard link of the kept file, junction and symbolic link as kept or
removed entry, failing and silently failing Recycle Bin, kept copy locked against rename, delete
and write during the operation, 20 rounds of two opposite parallel requests, several independent
groups. `real_recycle_bin_with_handles_held` (ignored by default, run with `--ignored`) uses the
real Windows Recycle Bin with a synthetic file.
