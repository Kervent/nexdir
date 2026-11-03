# Update feeds

Nexdir needs no server of its own. Releases live on GitHub; two small JSON files tell Nexdir
which release is the newest per channel:

```text
https://raw.githubusercontent.com/Kervent/nexdir/update-feed/stable.json
https://raw.githubusercontent.com/Kervent/nexdir/update-feed/preview.json
```

GitHub's `/releases/latest` is not used: it ignores pre-releases, so it cannot serve a preview
channel, and it cannot tell alpha, beta and stable apart.

## Format (schema 1)

```json
{
  "schema": 1,
  "channel": "preview",
  "version": "0.5.0-alpha.2",
  "prerelease": true,
  "pub_date": "2026-11-20T12:00:00Z",
  "notes": "Release notes (Markdown)",
  "release_url": "https://github.com/Kervent/nexdir/releases/tag/v0.5.0-alpha.2",
  "platforms": {
    "windows-x86_64-nsis": { "url": ".../releases/download/v0.5.0-alpha.2/Nexdir-0.5.0-alpha.2-Setup.exe", "signature": "<contents of the .sig file>", "size": 2600000 },
    "windows-x86_64-msi":  { "url": ".../releases/download/v0.5.0-alpha.2/Nexdir-0.5.0-alpha.2.msi", "signature": "…", "size": 3500000 }
  }
}
```

A channel without a release has `"version": null` (the stable feed until 1.0).

## How the feeds stay in sync

`.github/workflows/update-feed.yml` runs on every release event (published, edited,
unpublished, deleted) and by hand. It runs `tools/release/update_feed.py` from `main`, which
lists the releases through the GitHub API and keeps a release only if

- it is published (never a draft) and its tag is a valid SemVer version,
- it has both installers and both `.sig` files,
- both signatures verify against the public key in `main`'s `tauri.conf.json` and name exactly
  that version, and the files match `SHA256SUMS.txt`.

The newest such release becomes `preview.json`; the newest that is neither a GitHub pre-release
nor a SemVer pre-release becomes `stable.json`. The files are committed to the `update-feed`
branch only when they change. Unpublishing or deleting a release removes it from the feed on
the next run (Nexdir never goes back to an older version, it just stops offering the withdrawn
one). Releases from before 0.5.0 have no signatures and are never listed.

## What Nexdir checks (it does not trust the feed)

The feed is served over HTTPS but is not signed itself, so Nexdir (`src-tauri/src/updater/feed.rs`)
accepts it only if the schema and channel match, the version is valid SemVer and strictly newer
than the running one (stable: no pre-release), the release page and every download address lie
inside this repository's release for exactly that version, and every signature is present and
well-formed. The installer is then verified by the official Tauri updater against the built-in
public key with `requireSignedVersion`. The worst a manipulated feed can do is announce an
update that then fails verification, or hide one.
