use std::path::PathBuf;
use std::process::Command;

fn poko() -> Command {
    Command::new(env!("CARGO_BIN_EXE_poko"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../core/tests/fixtures")
        .join(name)
}

#[test]
fn lint_clean_vault_exits_zero() {
    let output = poko().arg("lint").arg(fixture("vault")).output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "OK (1 notes, 4 cards)\n"
    );
}

#[test]
fn lint_duplicate_id_exits_one() {
    let output = poko()
        .arg("lint")
        .arg(fixture("bad-vaults/duplicate-id"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("id の重複"));
    assert!(stdout.contains("dup"));
}
