use std::path::PathBuf;

use assert_cmd::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../core/tests/fixtures")
        .join(name)
}

#[test]
fn sync_lint_fails_before_network() {
    let assert = Command::cargo_bin("poko")
        .unwrap()
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
    let assert = Command::cargo_bin("poko")
        .unwrap()
        .args([
            "sync",
            fixture("vault").to_str().unwrap(),
            "--endpoint",
            "http://127.0.0.1:9",
        ])
        .assert()
        .code(1);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(stderr.contains("接続失敗"), "{stderr}");
}
