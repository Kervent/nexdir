//! Network behaviour of the feed download against a local test server (plain HTTP on
//! 127.0.0.1 — release builds refuse anything but HTTPS, which is tested too).

use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;

/// Serves exactly one connection with `response` (raw HTTP), optionally waiting first.
fn serve_once(response: Vec<u8>, delay: Duration) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            std::thread::sleep(delay);
            let _ = stream.write_all(&response);
            let _ = stream.flush();
        }
    });
    format!("http://{addr}/preview.json")
}

fn http(status: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn fetch(url: &str) -> UpdateResult<Vec<u8>> {
    tauri::async_runtime::block_on(fetch_feed_with(url, true, Duration::from_secs(5), Duration::from_secs(3)))
}

#[test]
fn reads_a_feed() {
    let url = serve_once(http("200 OK", r#"{"schema":1}"#), Duration::ZERO);
    assert_eq!(fetch(&url).unwrap(), br#"{"schema":1}"#);
}

#[test]
fn maps_http_errors() {
    for (status, code) in [
        ("404 Not Found", "notFound"),
        ("429 Too Many Requests", "rateLimited"),
        ("403 Forbidden", "rateLimited"),
        ("500 Internal Server Error", "server"),
        ("503 Service Unavailable", "server"),
    ] {
        let url = serve_once(http(status, "{}"), Duration::ZERO);
        assert_eq!(fetch(&url).unwrap_err().code, code, "{status}");
    }
}

#[test]
fn does_not_follow_redirects() {
    let url = serve_once(
        b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/evil.json\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
        Duration::ZERO,
    );
    let err = fetch(&url).unwrap_err();
    assert_eq!(err.code, "server");
    assert!(err.message.contains("redirected"), "{}", err.message);
}

#[test]
fn rejects_oversized_answers() {
    let big = "x".repeat(feed::MAX_FEED_BYTES + 1);
    let url = serve_once(http("200 OK", &big), Duration::ZERO);
    assert_eq!(fetch(&url).unwrap_err().code, "malformed");
    // Without a Content-Length the limit applies while reading.
    let chunked = format!("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{big}");
    let url = serve_once(chunked.into_bytes(), Duration::ZERO);
    assert_eq!(fetch(&url).unwrap_err().code, "malformed");
}

#[test]
fn reports_a_dropped_connection() {
    let url = serve_once(
        b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nConnection: close\r\n\r\n{\"schema\"".to_vec(),
        Duration::ZERO,
    );
    assert_eq!(fetch(&url).unwrap_err().code, "network");
}

#[test]
fn reports_a_timeout() {
    let url = serve_once(http("200 OK", "{}"), Duration::from_secs(5));
    let started = Instant::now();
    assert_eq!(fetch(&url).unwrap_err().code, "timeout");
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[test]
fn reports_offline() {
    // Bind and close a port so nothing listens there.
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    assert_eq!(fetch(&format!("http://127.0.0.1:{port}/stable.json")).unwrap_err().code, "offline");
}

#[test]
fn release_builds_require_https() {
    let err = tauri::async_runtime::block_on(fetch_feed("http://127.0.0.1:1/stable.json", false)).unwrap_err();
    assert_eq!(err.code, "badUrl");
    assert_eq!(
        tauri::async_runtime::block_on(fetch_feed("not a url", false)).unwrap_err().code,
        "badUrl"
    );
    assert_eq!(
        tauri::async_runtime::block_on(fetch_feed("file:///C:/feed.json", true))
            .unwrap_err()
            .code,
        "badUrl"
    );
}

#[test]
fn production_endpoints_are_github_over_https() {
    let ep = Endpoints {
        feed_base: FEED_BASE.into(),
        trust: Trust {
            download_prefix: String::new(),
            release_page_prefix: String::new(),
        },
        allow_http: false,
    };
    assert!(ep.feed_base.starts_with("https://raw.githubusercontent.com/Kervent/nexdir/"));
    #[cfg(not(feature = "updater-test"))]
    {
        let real = endpoints();
        assert_eq!(real.feed_base, FEED_BASE);
        assert!(!real.allow_http);
        assert_eq!(real.trust.download_prefix, "https://github.com/Kervent/nexdir/releases/download/");
    }
}

#[test]
fn only_one_check_at_a_time() {
    let flag = AtomicBool::new(false);
    let first = Exclusive::acquire(&flag).expect("first check starts");
    assert!(Exclusive::acquire(&flag).is_none(), "second check is refused while the first runs");
    drop(first);
    assert!(Exclusive::acquire(&flag).is_some(), "released after the first check ends");
}

#[test]
fn errors_do_not_leak_feed_content() {
    let e: UpdateError = FeedError::Malformed("not valid JSON".into()).into();
    assert_eq!(e.code, "malformed");
    let e = status_error(429);
    assert!(!e.message.contains("http://"));
}

#[test]
fn installer_kind_must_match_the_installation() {
    // Development and test runs are not installed, so nothing is accepted.
    assert_eq!(install_kind(), InstallKind::Portable);
    assert!(check_installer_kind(b"MZ\x90\x00").is_err());
    assert!(check_installer_kind(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]).is_err());
}
