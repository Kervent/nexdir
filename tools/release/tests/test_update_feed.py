"""Tests for update_feed.py with signatures made by the Tauri CLI and a throwaway test key
(fixtures in src-tauri/tests/fixtures/updater; the private test key is not in the repository).

Run: python -m unittest discover -s tools/release/tests
"""
import hashlib
import os
import sys
import unittest

HERE = os.path.dirname(__file__)
sys.path.insert(0, os.path.abspath(os.path.join(HERE, "..")))
import update_feed as uf  # noqa: E402

FIX = os.path.abspath(os.path.join(HERE, "..", "..", "..", "src-tauri", "tests", "fixtures", "updater"))
REPO = "Kervent/nexdir"


def fixture(name, mode="rb"):
    with open(os.path.join(FIX, name), mode) as f:
        return f.read()


PAYLOAD = fixture("payload.bin")
KEY = fixture("test-key.pub", "r").strip()
OTHER_KEY = fixture("other-key.pub", "r").strip()
SIG = {"0.6.0": fixture("payload-0.6.0.sig", "r").strip(), "0.7.0-beta.1": fixture("payload-0.7.0-beta.1.sig", "r").strip()}


def release(version, sig_version=None, prerelease=False, draft=False, data=PAYLOAD, sums=True, missing=()):
    base = f"https://github.com/{REPO}/releases/download/v{version}"
    files = {}
    for name in (f"Nexdir-{version}-Setup.exe", f"Nexdir-{version}.msi"):
        files[name] = data
        if sig_version:
            files[f"{name}.sig"] = SIG[sig_version].encode()
    if sums:
        files["SHA256SUMS.txt"] = "".join(f"{hashlib.sha256(PAYLOAD).hexdigest()}  {n}\n" for n in files if not n.endswith(".sig")).encode()
    for m in missing:
        files.pop(m.format(v=version), None)
    store = {f"{base}/{n}": d for n, d in files.items()}
    rel = {
        "tag_name": f"v{version}",
        "draft": draft,
        "prerelease": prerelease,
        "published_at": None if draft else "2026-11-20T12:00:00Z",
        "body": f"Notes for {version}",
        "html_url": f"https://github.com/{REPO}/releases/tag/v{version}",
        "assets": [{"name": n, "browser_download_url": f"{base}/{n}", "size": len(d), "state": "uploaded"} for n, d in files.items()],
    }
    return rel, store


def feeds(*releases, key=KEY):
    store = {}
    rels = []
    for rel, s in releases:
        rels.append(rel)
        store.update(s)
    return uf.build_feeds(rels, key, lambda url: store[url], REPO)


class Ed25519(unittest.TestCase):
    def test_rfc8032_test_vector_1(self):
        pk = bytes.fromhex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a")
        sig = bytes.fromhex("e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b")
        self.assertTrue(uf.ed25519_verify(pk, b"", sig))
        self.assertFalse(uf.ed25519_verify(pk, b"x", sig))
        self.assertFalse(uf.ed25519_verify(pk, b"", sig[:-1] + bytes([sig[-1] ^ 1])))


class Signatures(unittest.TestCase):
    def test_tauri_signature_verifies_and_names_the_version(self):
        trusted = uf.verify_signature(PAYLOAD, SIG["0.6.0"], KEY)
        self.assertEqual(uf.signed_version(trusted), "0.6.0")

    def test_modified_file_wrong_key_and_unversioned(self):
        with self.assertRaises(ValueError):
            uf.verify_signature(PAYLOAD[:-1] + b"!", SIG["0.6.0"], KEY)
        with self.assertRaises(ValueError):
            uf.verify_signature(PAYLOAD, SIG["0.6.0"], OTHER_KEY)
        trusted = uf.verify_signature(PAYLOAD, fixture("payload-unversioned.sig", "r").strip(), KEY)
        self.assertIsNone(uf.signed_version(trusted))


class Versions(unittest.TestCase):
    def test_semver_precedence(self):
        order = ["0.4.3", "0.5.0-alpha.1", "0.5.0-alpha.2", "0.5.0-alpha.10", "0.5.0-beta", "0.5.0-beta.2", "0.5.0-rc.1", "0.5.0", "1.0.0"]
        keys = [uf.version_key(uf.parse_version(v)) for v in order]
        self.assertEqual(keys, sorted(keys))
        for bad in ["v1.0.0", "1.0", "01.0.0", "1.0.0+b", "1.0.0-", ""]:
            self.assertIsNone(uf.parse_version(bad), bad)


class Feeds(unittest.TestCase):
    def test_only_verified_published_releases_are_listed(self):
        f = feeds(
            release("0.4.3", prerelease=True),  # before the updater: no signatures
            release("0.6.0", "0.6.0", prerelease=True),
            release("0.7.0-beta.1", "0.7.0-beta.1", draft=True, prerelease=True),  # draft
        )
        self.assertEqual(f["preview"]["version"], "0.6.0")
        self.assertEqual(f["stable"]["version"], None, "a GitHub pre-release never reaches stable")
        p = f["preview"]["platforms"]
        self.assertEqual(set(p), {"windows-x86_64-nsis", "windows-x86_64-msi"})
        self.assertEqual(p["windows-x86_64-nsis"]["url"], f"https://github.com/{REPO}/releases/download/v0.6.0/Nexdir-0.6.0-Setup.exe")
        self.assertEqual(p["windows-x86_64-msi"]["size"], len(PAYLOAD))
        self.assertEqual(f["preview"]["release_url"], f"https://github.com/{REPO}/releases/tag/v0.6.0")

    def test_preview_takes_the_newest_and_stable_only_finished_releases(self):
        f = feeds(release("0.6.0", "0.6.0"), release("0.7.0-beta.1", "0.7.0-beta.1", prerelease=True))
        self.assertEqual(f["preview"]["version"], "0.7.0-beta.1")
        self.assertTrue(f["preview"]["prerelease"])
        self.assertEqual(f["stable"]["version"], "0.6.0")
        self.assertFalse(f["stable"]["prerelease"])

    def test_signature_for_another_version_is_rejected(self):
        # The files of 0.6.0 published again as "0.8.0".
        f = feeds(release("0.8.0", "0.6.0"))
        self.assertIsNone(f["preview"]["version"])

    def test_tampered_missing_and_foreign_key(self):
        self.assertIsNone(feeds(release("0.6.0", "0.6.0", data=PAYLOAD + b"x", sums=False))["preview"]["version"])
        self.assertIsNone(feeds(release("0.6.0", "0.6.0", missing=["Nexdir-{v}.msi.sig"]))["preview"]["version"])
        self.assertIsNone(feeds(release("0.6.0", "0.6.0", missing=["Nexdir-{v}-Setup.exe"]))["preview"]["version"])
        self.assertIsNone(feeds(release("0.6.0", "0.6.0"), key=OTHER_KEY)["preview"]["version"])

    def test_checksum_mismatch_is_rejected(self):
        rel, store = release("0.6.0", "0.6.0")
        store[f"https://github.com/{REPO}/releases/download/v0.6.0/SHA256SUMS.txt"] = b"0" * 64 + b"  Nexdir-0.6.0-Setup.exe\n"
        self.assertIsNone(feeds((rel, store))["preview"]["version"])

    def test_empty_feeds_have_no_version(self):
        f = feeds()
        self.assertEqual(f["stable"], {"schema": 1, "channel": "stable", "version": None})
        self.assertEqual(f["preview"], {"schema": 1, "channel": "preview", "version": None})

    def test_downloads_only_from_github(self):
        with self.assertRaises(ValueError):
            uf.download("https://evil.example/Nexdir.exe")


if __name__ == "__main__":
    unittest.main()
