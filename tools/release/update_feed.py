"""Builds Nexdir's update feeds (stable.json, preview.json) from the published GitHub releases.

Run by .github/workflows/update-feed.yml whenever a release is published, edited, unpublished or
deleted, so the feeds always match what is actually published. A release is only listed when

  * it is published (drafts are never visible to this script's API calls without a token that
    can see them, and are skipped explicitly as well),
  * its tag is a valid version (vMAJOR.MINOR.PATCH[-PRERELEASE]),
  * it has both installers (Nexdir-<v>-Setup.exe, Nexdir-<v>.msi) and their .sig files,
  * each installer's signature verifies against Nexdir's public update key (minisign / Ed25519)
    and names exactly this version in its signed trusted comment, and
  * the installers match SHA256SUMS.txt when the release has one.

Releases that fail a check are left out (with a warning); the newest valid one per channel is
used. The stable feed only considers releases that are not pre-releases on GitHub and have no
pre-release identifier. Releases from before the updater (0.4.3 and older) have no signatures
and are therefore never offered.

Usage:
    python tools/release/update_feed.py --repo Kervent/nexdir --out feed
        (GITHUB_TOKEN or GH_TOKEN in the environment raises the API rate limit)
    python tools/release/update_feed.py --verify FILE FILE.sig --pubkey KEY [--version V]
        (checks one signature locally, e.g. in the release workflow)
"""
import argparse
import base64
import hashlib
import json
import os
import re
import sys
import urllib.request

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
SCHEMA = 1
VERSION_RE = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*))?$")
PLATFORMS = {"windows-x86_64-nsis": "Nexdir-{v}-Setup.exe", "windows-x86_64-msi": "Nexdir-{v}.msi"}
MAX_DOWNLOAD = 200 * 1024 * 1024


# ---- Semantic versions --------------------------------------------------------------------

def parse_version(text):
    m = VERSION_RE.match(text or "")
    if not m:
        return None
    pre = m.group(4)
    return (int(m.group(1)), int(m.group(2)), int(m.group(3)), tuple(pre.split(".")) if pre else ())


def version_key(v):
    """Sort key following SemVer 2.0 precedence (a release ranks above its pre-releases)."""
    major, minor, patch, pre = v
    ids = tuple((0, int(p), "") if p.isdigit() else (1, 0, p) for p in pre)
    return (major, minor, patch, 0 if pre else 1, ids)


# ---- Ed25519 (RFC 8032, verification only) ------------------------------------------------

_P = 2**255 - 19
_L = 2**252 + 27742317777372353535851937790883648493
_D = -121665 * pow(121666, _P - 2, _P) % _P
_SQRT_M1 = pow(2, (_P - 1) // 4, _P)
_G = None


def _add(a, b):
    x1, y1, z1, t1 = a
    x2, y2, z2, t2 = b
    A = (y1 - x1) * (y2 - x2) % _P
    B = (y1 + x1) * (y2 + x2) % _P
    C = 2 * t1 * t2 * _D % _P
    D = 2 * z1 * z2 % _P
    E, F, G, H = B - A, D - C, D + C, B + A
    return (E * F % _P, G * H % _P, F * G % _P, E * H % _P)


def _mul(s, p):
    q = (0, 1, 1, 0)
    while s:
        if s & 1:
            q = _add(q, p)
        p = _add(p, p)
        s >>= 1
    return q


def _equal(a, b):
    return (a[0] * b[2] - b[0] * a[2]) % _P == 0 and (a[1] * b[2] - b[1] * a[2]) % _P == 0


def _recover_x(y, sign):
    if y >= _P:
        return None
    x2 = (y * y - 1) * pow(_D * y * y + 1, _P - 2, _P)
    if x2 == 0:
        return None if sign else 0
    x = pow(x2, (_P + 3) // 8, _P)
    if (x * x - x2) % _P:
        x = x * _SQRT_M1 % _P
    if (x * x - x2) % _P:
        return None
    if (x & 1) != sign:
        x = _P - x
    return x


def _decompress(s):
    if len(s) != 32:
        return None
    y = int.from_bytes(s, "little")
    sign = y >> 255
    y &= (1 << 255) - 1
    x = _recover_x(y, sign)
    return None if x is None else (x, y, 1, x * y % _P)


def _base():
    global _G
    if _G is None:
        gy = 4 * pow(5, _P - 2, _P) % _P
        gx = _recover_x(gy, 0)
        _G = (gx, gy, 1, gx * gy % _P)
    return _G


def ed25519_verify(public, message, signature):
    if len(public) != 32 or len(signature) != 64:
        return False
    a = _decompress(public)
    r = _decompress(signature[:32])
    if a is None or r is None:
        return False
    s = int.from_bytes(signature[32:], "little")
    if s >= _L:
        return False
    h = int.from_bytes(hashlib.sha512(signature[:32] + public + message).digest(), "little") % _L
    return _equal(_mul(s, _base()), _add(r, _mul(h, a)))


# ---- Minisign signatures as written by the Tauri CLI --------------------------------------

def _lines(b64_text):
    text = base64.b64decode(b64_text.strip(), validate=True).decode("utf-8")
    return [l for l in text.replace("\r\n", "\n").split("\n") if l]


def parse_pubkey(b64_text):
    lines = _lines(b64_text)
    raw = base64.b64decode(lines[1])
    if len(raw) != 42 or raw[:2] != b"Ed":
        raise ValueError("not a minisign Ed25519 public key")
    return raw[2:10], raw[10:]


def verify_signature(data, sig_b64, pubkey_b64):
    """Returns the signed trusted comment if `data` is signed by the key, else raises ValueError."""
    key_id, pk = parse_pubkey(pubkey_b64)
    lines = _lines(sig_b64)
    if len(lines) < 4 or not lines[2].startswith("trusted comment: "):
        raise ValueError("malformed signature file")
    blob = base64.b64decode(lines[1])
    if len(blob) != 74:
        raise ValueError("malformed signature")
    alg, sig_key_id, sig = blob[:2], blob[2:10], blob[10:]
    if sig_key_id != key_id:
        raise ValueError("signed with a different key")
    if alg == b"ED":
        message = hashlib.blake2b(data, digest_size=64).digest()
    elif alg == b"Ed":
        message = data
    else:
        raise ValueError("unknown signature algorithm")
    if not ed25519_verify(pk, message, sig):
        raise ValueError("signature does not match the file")
    trusted = lines[2][len("trusted comment: "):]
    if not ed25519_verify(pk, sig + trusted.encode("utf-8"), base64.b64decode(lines[3])):
        raise ValueError("trusted comment was modified")
    return trusted


def signed_version(trusted_comment):
    m = re.search(r"(?:^|\s)version:(\S+)", trusted_comment)
    return m.group(1) if m else None


# ---- Feeds --------------------------------------------------------------------------------

def warn(message):
    print(f"warning: {message}", file=sys.stderr)


def check_release(release, pubkey, fetch):
    """The feed entry for one GitHub release, or None (with a warning) when it is not usable."""
    tag = release.get("tag_name") or ""
    if release.get("draft"):
        return None
    if not release.get("published_at"):
        warn(f"{tag}: not published")
        return None
    version = tag[1:] if tag.startswith("v") else None
    parsed = parse_version(version)
    if not parsed:
        warn(f"{tag}: not a version tag")
        return None
    assets = {a["name"]: a for a in release.get("assets") or [] if a.get("state", "uploaded") == "uploaded"}
    sums = {}
    if "SHA256SUMS.txt" in assets:
        for line in fetch(assets["SHA256SUMS.txt"]["browser_download_url"]).decode("utf-8").splitlines():
            parts = line.split()
            if len(parts) == 2:
                sums[parts[1]] = parts[0].lower()
    platforms = {}
    for key, pattern in PLATFORMS.items():
        name = pattern.format(v=version)
        if name not in assets or f"{name}.sig" not in assets:
            warn(f"{tag}: {name} or its signature is missing - not offered as an update")
            return None
        data = fetch(assets[name]["browser_download_url"])
        sig = fetch(assets[f"{name}.sig"]["browser_download_url"]).decode("ascii").strip()
        try:
            trusted = verify_signature(data, sig, pubkey)
        except ValueError as e:
            warn(f"{tag}: {name}: {e}")
            return None
        if signed_version(trusted) != version:
            warn(f"{tag}: {name} is signed for version {signed_version(trusted)!r}, not {version}")
            return None
        if sums and sums.get(name) != hashlib.sha256(data).hexdigest():
            warn(f"{tag}: {name} does not match SHA256SUMS.txt")
            return None
        if len(data) != assets[name].get("size", len(data)):
            warn(f"{tag}: {name} size differs from the release asset")
            return None
        platforms[key] = {"url": assets[name]["browser_download_url"], "signature": sig, "size": len(data)}
    return {
        "version": version,
        "parsed": parsed,
        "prerelease": bool(release.get("prerelease")) or bool(parsed[3]),
        "pub_date": release["published_at"],
        "notes": (release.get("body") or "").strip()[:60_000],
        "release_url": release.get("html_url") or "",
        "platforms": platforms,
    }


def build_feeds(releases, pubkey, fetch, repo):
    valid = []
    for r in releases:
        entry = check_release(r, pubkey, fetch)
        if entry:
            expected = f"https://github.com/{repo}/releases/tag/v{entry['version']}"
            if entry["release_url"] != expected:
                warn(f"v{entry['version']}: unexpected release page {entry['release_url']}")
                continue
            valid.append(entry)
    valid.sort(key=lambda e: version_key(e["parsed"]), reverse=True)
    feeds = {}
    for channel in ("stable", "preview"):
        pick = next((e for e in valid if channel == "preview" or not e["prerelease"]), None)
        feed = {"schema": SCHEMA, "channel": channel, "version": None}
        if pick:
            feed.update({k: pick[k] for k in ("version", "prerelease", "pub_date", "notes", "release_url", "platforms")})
        feeds[channel] = feed
    return feeds


def github_releases(repo, token):
    out, page = [], 1
    while True:
        req = urllib.request.Request(f"https://api.github.com/repos/{repo}/releases?per_page=100&page={page}",
                                     headers={"Accept": "application/vnd.github+json", "User-Agent": "nexdir-update-feed",
                                              **({"Authorization": f"Bearer {token}"} if token else {})})
        with urllib.request.urlopen(req, timeout=60) as r:
            batch = json.load(r)
        out += batch
        if len(batch) < 100:
            return out
        page += 1


def download(url):
    if not url.startswith("https://github.com/"):
        raise ValueError(f"refusing to download {url}")
    req = urllib.request.Request(url, headers={"User-Agent": "nexdir-update-feed"})
    with urllib.request.urlopen(req, timeout=300) as r:
        data = r.read(MAX_DOWNLOAD + 1)
    if len(data) > MAX_DOWNLOAD:
        raise ValueError(f"{url} is too large")
    return data


def repo_pubkey():
    with open(os.path.join(ROOT, "src-tauri", "tauri.conf.json"), encoding="utf-8") as f:
        key = json.load(f)["plugins"]["updater"]["pubkey"].strip()
    if not key:
        sys.exit("update_feed: src-tauri/tauri.conf.json has no updater public key")
    return key


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo")
    ap.add_argument("--out")
    ap.add_argument("--verify", nargs=2, metavar=("FILE", "SIG"))
    ap.add_argument("--pubkey", help="public key (base64 text); default: the one in tauri.conf.json")
    ap.add_argument("--version", help="with --verify: the version the signature must name")
    args = ap.parse_args()
    pubkey = args.pubkey or repo_pubkey()

    if args.verify:
        with open(args.verify[0], "rb") as f:
            data = f.read()
        with open(args.verify[1], encoding="ascii") as f:
            sig = f.read().strip()
        try:
            trusted = verify_signature(data, sig, pubkey)
        except ValueError as e:
            sys.exit(f"update_feed: {os.path.basename(args.verify[0])}: {e}")
        if args.version and signed_version(trusted) != args.version:
            sys.exit(f"update_feed: {os.path.basename(args.verify[0])} is signed for {signed_version(trusted)!r}, expected {args.version}")
        print(f"OK {os.path.basename(args.verify[0])}: {trusted}")
        return

    if not args.repo or not args.out:
        ap.error("--repo and --out are required")
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    feeds = build_feeds(github_releases(args.repo, token), pubkey, download, args.repo)
    os.makedirs(args.out, exist_ok=True)
    for channel, feed in feeds.items():
        with open(os.path.join(args.out, f"{channel}.json"), "w", encoding="utf-8", newline="\n") as f:
            json.dump(feed, f, indent=2, ensure_ascii=False)
            f.write("\n")
        print(f"{channel}: {feed['version'] or 'no release'}")


if __name__ == "__main__":
    main()
