use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use assert_cmd::Command;

const ACCESS_DENIED: &str = "Access に拒否されました(POKO_ACCESS_CLIENT_ID / POKO_ACCESS_CLIENT_SECRET とサービス認証ポリシーを確認)";

struct Seen {
    path: String,
    headers: Vec<(String, String)>,
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../core/tests/fixtures")
        .join(name)
}

fn poko() -> Command {
    let mut command = Command::cargo_bin("poko").unwrap();
    command.env_remove("POKO_ENDPOINT");
    command.env_remove("POKO_ACCESS_CLIENT_ID");
    command.env_remove("POKO_ACCESS_CLIENT_SECRET");
    command
}

fn serve(status: u16, location: Option<&str>, body: &str) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&seen);
    let location = location.map(str::to_owned);
    let body = body.to_owned();
    thread::spawn(move || {
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else {
                continue;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let Some(hit) = read_request(&mut stream) else {
                continue;
            };
            record.lock().unwrap().push(hit);
            let reason = match status {
                200 => "OK",
                302 => "Found",
                403 => "Forbidden",
                _ => "Error",
            };
            let location_header = location
                .as_ref()
                .map(|value| format!("Location: {value}\r\n"))
                .unwrap_or_default();
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\n{location_header}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (format!("http://{address}"), seen)
}

fn read_request(stream: &mut impl Read) -> Option<Seen> {
    let mut buffer = Vec::new();
    let mut chunk = [0; 2048];
    let header_end = loop {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let head = String::from_utf8_lossy(&buffer[..header_end]);
    let length = head.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("content-length") {
            value.trim().parse::<usize>().ok()
        } else {
            None
        }
    });
    if let Some(length) = length {
        let mut have = buffer.len() - header_end;
        while have < length {
            let read = stream.read(&mut chunk).ok()?;
            if read == 0 {
                break;
            }
            have += read;
        }
    }
    let mut lines = head.lines();
    let path = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("")
        .to_owned();
    let headers = lines
        .take_while(|line| !line.is_empty())
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        })
        .collect();
    Some(Seen { path, headers })
}

fn header<'a>(seen: &'a Seen, name: &str) -> Option<&'a str> {
    seen.headers
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

fn stderr_of(assert: &assert_cmd::assert::Assert) -> String {
    String::from_utf8_lossy(&assert.get_output().stderr).into_owned()
}

#[test]
fn sync_lint_fails_before_network() {
    let assert = poko()
        .args([
            "sync",
            fixture("bad-vaults/duplicate-id").to_str().unwrap(),
            "--endpoint",
            "http://127.0.0.1:9",
        ])
        .assert()
        .code(1);
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("id の重複"), "{stdout}");
    assert!(stdout.contains("dup"), "{stdout}");
    assert!(!stderr.contains("接続失敗"), "{stderr}");
}

#[test]
fn sync_unreachable_endpoint_exits_one() {
    let assert = poko()
        .args([
            "sync",
            fixture("vault").to_str().unwrap(),
            "--endpoint",
            "http://127.0.0.1:9",
        ])
        .assert()
        .code(1);
    let stderr = stderr_of(&assert);
    assert!(stderr.contains("接続失敗"), "{stderr}");
}

#[test]
fn sync_help_mentions_env() {
    let assert = poko().args(["sync", "--help"]).assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("POKO_ENDPOINT"), "{stdout}");
    assert!(stdout.contains("POKO_ACCESS_CLIENT_ID"), "{stdout}");
    assert!(stdout.contains("POKO_ACCESS_CLIENT_SECRET"), "{stdout}");
}

#[test]
fn access_headers_are_sent_when_both_are_set() {
    let (url, seen) = serve(200, None, "[]");
    poko()
        .env("POKO_ACCESS_CLIENT_ID", "test-client-id")
        .env("POKO_ACCESS_CLIENT_SECRET", "test-client-secret")
        .args([
            "sync",
            fixture("vault").to_str().unwrap(),
            "--endpoint",
            &url,
        ])
        .assert()
        .success();
    let hits = seen.lock().unwrap();
    assert!(!hits.is_empty());
    for hit in hits.iter() {
        assert_eq!(header(hit, "cf-access-client-id"), Some("test-client-id"));
        assert!(
            header(hit, "cf-access-client-secret") == Some("test-client-secret"),
            "CF-Access-Client-Secret が無い、または違う"
        );
    }
}

#[test]
fn access_headers_are_absent_without_env() {
    let (url, seen) = serve(200, None, "[]");
    poko()
        .args([
            "sync",
            fixture("vault").to_str().unwrap(),
            "--endpoint",
            &url,
        ])
        .assert()
        .success();
    let hits = seen.lock().unwrap();
    assert!(!hits.is_empty());
    for hit in hits.iter() {
        assert!(header(hit, "cf-access-client-id").is_none());
        assert!(header(hit, "cf-access-client-secret").is_none());
    }
}

#[test]
fn endpoint_env_is_used_and_flag_wins() {
    let (url, seen) = serve(200, None, "[]");
    poko()
        .env("POKO_ENDPOINT", &url)
        .args(["sync", fixture("vault").to_str().unwrap()])
        .assert()
        .success();
    assert!(!seen.lock().unwrap().is_empty());

    let (url, seen) = serve(200, None, "[]");
    poko()
        .env("POKO_ENDPOINT", "http://127.0.0.1:9")
        .args([
            "sync",
            fixture("vault").to_str().unwrap(),
            "--endpoint",
            &url,
        ])
        .assert()
        .success();
    assert!(!seen.lock().unwrap().is_empty());
}

#[test]
fn redirect_is_not_followed_and_is_access_denied() {
    let (url, seen) = serve(302, Some("http://127.0.0.1:1/followed"), "");
    let assert = poko()
        .args([
            "sync",
            fixture("vault").to_str().unwrap(),
            "--endpoint",
            &url,
        ])
        .assert()
        .code(1);
    let stderr = stderr_of(&assert);
    assert!(stderr.contains(ACCESS_DENIED), "{stderr}");
    assert!(!stderr.contains("接続失敗"), "{stderr}");
    let hits = seen.lock().unwrap();
    assert_eq!(hits.len(), 1, "リダイレクト先へ行っている");
    assert_eq!(hits[0].path, "/api/sync/manifest");
}

#[test]
fn forbidden_is_access_denied() {
    let (url, _) = serve(403, None, "denied");
    let assert = poko()
        .args([
            "sync",
            fixture("vault").to_str().unwrap(),
            "--endpoint",
            &url,
        ])
        .assert()
        .code(1);
    let stderr = stderr_of(&assert);
    assert!(stderr.contains(ACCESS_DENIED), "{stderr}");
    assert!(!stderr.contains("denied"), "{stderr}");
}

#[test]
fn only_client_id_is_a_config_error() {
    let assert = poko()
        .env("POKO_ACCESS_CLIENT_ID", "test-client-id")
        .args([
            "sync",
            fixture("vault").to_str().unwrap(),
            "--endpoint",
            "http://127.0.0.1:9",
        ])
        .assert()
        .code(1);
    let stderr = stderr_of(&assert);
    assert!(
        stderr.contains("POKO_ACCESS_CLIENT_SECRET が無い"),
        "{stderr}"
    );
    assert!(!stderr.contains("接続失敗"), "{stderr}");
    assert!(!stderr.contains("test-client-secret"), "{stderr}");
}
