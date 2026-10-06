use std::path::PathBuf;

mod common;

use common::{poko, serve};

const ACCESS_DENIED: &str = "Access に拒否されました(POKO_ACCESS_CLIENT_ID / POKO_ACCESS_CLIENT_SECRET とサービス認証ポリシーを確認)";

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../core/tests/fixtures")
        .join(name)
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
        assert_eq!(hit.header("cf-access-client-id"), Some("test-client-id"));
        assert!(
            hit.header("cf-access-client-secret") == Some("test-client-secret"),
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
        assert!(hit.header("cf-access-client-id").is_none());
        assert!(hit.header("cf-access-client-secret").is_none());
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
