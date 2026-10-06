mod common;

use common::{poko, serve_seq};

const PAGE: &str = r#"[{"id":1,"card_key":"a","rating":"good","reviewed_at":10,"interval_days":1.0,"stability":1.0,"difficulty":1.0,"response":null},{"id":2,"card_key":"b","rating":"hard","reviewed_at":11,"interval_days":1.0,"stability":1.0,"difficulty":1.0,"response":null}]"#;

#[test]
fn optimize_does_not_put_when_there_are_too_few_reviews() {
    let (url, seen) = serve_seq(vec![(200, PAGE.to_owned())]);
    let assert = poko()
        .args(["optimize", "--endpoint", &url])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(
        stdout,
        "件数不足: 学習に使える復習 0 件(最低 400 件)。パラメータは変えない\n"
    );
    let hits = seen.lock().unwrap();
    assert!(
        hits.iter()
            .all(|hit| hit.path.starts_with("/api/export/reviews"))
    );
    assert!(hits.iter().all(|hit| !hit.path.contains("/api/params")));
}
