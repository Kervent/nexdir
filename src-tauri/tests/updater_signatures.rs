//! Update signatures, checked by the official Tauri updater with test keys.
//!
//! Fixtures in tests/fixtures/updater: a small file signed with a throwaway test key (its
//! private half was never stored in the repository), once bound to version 0.6.0 and once
//! without a version, plus that key's public half and an unrelated public key.
//!
//! An integration test because Tauri's mock runtime needs the Windows manifest that build.rs
//! embeds into test binaries.

use nexdir_lib::updater::{build_update, updater_error};
use std::io::{Read, Write};
use std::net::TcpListener;

use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/updater/");

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}{name}")).unwrap()
}

fn fixture_text(name: &str) -> String {
    String::from_utf8(fixture(name)).unwrap().trim().to_string()
}

fn app(pubkey: &str) -> tauri::App<MockRuntime> {
    let mut context = mock_context(noop_assets());
    context.config_mut().plugins.0.insert(
        "updater".into(),
        serde_json::json!({ "pubkey": pubkey, "requireSignedVersion": true }),
    );
    mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .unwrap()
}

/// Manifest as `feed::updater_manifest` produces it. Unit tests run unbundled, so the
/// updater looks for the plain `windows-x86_64` entry.
fn manifest(version: &str, url: &str, signature: &str) -> serde_json::Value {
    serde_json::json!({
        "version": version,
        "notes": "test",
        "pub_date": "2026-11-20T12:00:00Z",
        "platforms": { "windows-x86_64": { "url": url, "signature": signature } }
    })
}

fn update(pubkey: &str, version: &str, sig: &str) -> tauri_plugin_updater::Update {
    let app = app(&fixture_text(pubkey));
    build_update(
        app.handle(),
        manifest(
            version,
            "https://github.com/Kervent/nexdir/releases/download/v0.6.0/x.exe",
            &fixture_text(sig),
        ),
    )
    .unwrap()
    .expect("0.6.0 is newer than the mock app")
}

fn code(r: Result<(), tauri_plugin_updater::Error>) -> &'static str {
    r.map_err(updater_error).unwrap_err().code
}

#[test]
fn valid_signature_is_accepted() {
    update("test-key.pub", "0.6.0", "payload-0.6.0.sig")
        .verify(&fixture("payload.bin"))
        .unwrap();
}

#[test]
fn modified_file_is_rejected() {
    let mut bytes = fixture("payload.bin");
    bytes[100] ^= 1;
    assert_eq!(
        code(update("test-key.pub", "0.6.0", "payload-0.6.0.sig").verify(&bytes)),
        "signature"
    );
    let truncated = &fixture("payload.bin")[..1000];
    assert_eq!(
        code(update("test-key.pub", "0.6.0", "payload-0.6.0.sig").verify(truncated)),
        "signature"
    );
}

#[test]
fn signature_from_another_key_is_rejected() {
    assert_eq!(
        code(update("other-key.pub", "0.6.0", "payload-0.6.0.sig").verify(&fixture("payload.bin"))),
        "signature"
    );
}

#[test]
fn signature_for_another_version_is_rejected() {
    // A tampered feed announcing 0.7.0 with the file and signature of 0.6.0.
    assert_eq!(
        code(update("test-key.pub", "0.7.0", "payload-0.6.0.sig").verify(&fixture("payload.bin"))),
        "signature"
    );
}

#[test]
fn signature_without_version_is_rejected() {
    assert_eq!(
        code(update("test-key.pub", "0.6.0", "payload-unversioned.sig").verify(&fixture("payload.bin"))),
        "signature"
    );
}

#[test]
fn missing_or_garbage_signature_is_rejected() {
    let app = app(&fixture_text("test-key.pub"));
    for sig in ["", "AAAA", "not base64 at all"] {
        let update = build_update(app.handle(), manifest("0.6.0", "https://github.com/x", sig))
            .unwrap()
            .unwrap();
        assert_eq!(code(update.verify(&fixture("payload.bin"))), "signature", "{sig:?}");
    }
}

#[test]
fn older_or_equal_release_is_not_an_update() {
    let app = app(&fixture_text("test-key.pub"));
    let current = app.package_info().version.to_string();
    let sig = fixture_text("payload-0.6.0.sig");
    assert!(build_update(app.handle(), manifest(&current, "https://github.com/x", &sig))
        .unwrap()
        .is_none());
    assert!(build_update(app.handle(), manifest("0.0.1", "https://github.com/x", &sig))
        .unwrap()
        .is_none());
}

/// Serves the payload over local HTTP, optionally cutting the connection after `limit` bytes.
fn serve_payload(body: Vec<u8>, limit: Option<usize>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body[..limit.unwrap_or(body.len())]);
        }
    });
    format!("http://{addr}/Nexdir-0.6.0-Setup.exe")
}

fn download(body: Vec<u8>, limit: Option<usize>) -> (Result<Vec<u8>, tauri_plugin_updater::Error>, u64, Option<u64>) {
    let app = app(&fixture_text("test-key.pub"));
    let url = serve_payload(body, limit);
    let update = build_update(app.handle(), manifest("0.6.0", &url, &fixture_text("payload-0.6.0.sig")))
        .unwrap()
        .unwrap();
    let (mut seen, mut total) = (0u64, None);
    let result = tauri::async_runtime::block_on(update.download(
        |n, t| {
            seen += n as u64;
            total = t;
        },
        || {},
    ));
    (result, seen, total)
}

#[test]
fn download_reports_progress_and_verifies() {
    let payload = fixture("payload.bin");
    let (result, seen, total) = download(payload.clone(), None);
    assert_eq!(result.unwrap(), payload);
    assert_eq!(seen, payload.len() as u64);
    assert_eq!(total, Some(payload.len() as u64));
}

#[test]
fn interrupted_download_fails() {
    let (result, seen, _) = download(fixture("payload.bin"), Some(500));
    assert!(seen <= 500);
    let code = result.map(|_| ()).map_err(updater_error).unwrap_err().code;
    assert!(code == "network" || code == "signature", "{code}");
}

#[test]
fn corrupted_download_is_never_returned() {
    let mut bytes = fixture("payload.bin");
    bytes[10] ^= 0xff;
    let (result, _, _) = download(bytes, None);
    assert_eq!(result.map(|_| ()).map_err(updater_error).unwrap_err().code, "signature");
}
