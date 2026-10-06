use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use assert_cmd::Command;

const PAGE1: &str = r#"[{"id":1,"card_key":"a","rating":"good","reviewed_at":10,"interval_days":1.5,"stability":2.0,"difficulty":3.0,"response":null},{"id":2,"card_key":"b","rating":"hard","reviewed_at":11,"interval_days":1.0,"stability":1.0,"difficulty":5.0,"response":"memo"}]"#;
const PAGE2: &str = r#"[{"id":3,"card_key":"c","rating":"easy","reviewed_at":12,"interval_days":4.0,"stability":6.0,"difficulty":2.0,"response":null}]"#;

fn poko() -> Command {
    let mut command = Command::cargo_bin("poko").unwrap();
    command.env_remove("POKO_ENDPOINT");
    command.env_remove("POKO_ACCESS_CLIENT_ID");
    command.env_remove("POKO_ACCESS_CLIENT_SECRET");
    command
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("poko-export-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Hit {
    path: String,
    access: bool,
}

fn serve(responses: Vec<(u16, String)>) -> (String, Arc<Mutex<Vec<Hit>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&seen);
    thread::spawn(move || {
        let mut index = 0;
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else {
                continue;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let Some(hit) = read_hit(&mut stream) else {
                continue;
            };
            record.lock().unwrap().push(hit);
            let (status, body) = responses
                .get(index)
                .cloned()
                .unwrap_or((500, "extra".to_owned()));
            index += 1;
            let reason = if status == 200 { "OK" } else { "Error" };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (format!("http://{address}"), seen)
}

fn read_hit(stream: &mut impl Read) -> Option<Hit> {
    let mut buffer = Vec::new();
    let mut chunk = [0; 2048];
    loop {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buffer);
    let path = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))?
        .to_owned();
    let lower = head.to_ascii_lowercase();
    let access = lower.contains("cf-access-client-id: test-client-id")
        && lower.contains("cf-access-client-secret: test-client-secret");
    Some(Hit { path, access })
}

#[test]
fn export_joins_pages_into_jsonl() {
    let (url, seen) = serve(vec![(200, PAGE1.to_owned()), (200, PAGE2.to_owned())]);
    let dir = scratch("pages");
    let file = dir.join("reviews.jsonl");
    let assert = poko()
        .env("POKO_ACCESS_CLIENT_ID", "test-client-id")
        .env("POKO_ACCESS_CLIENT_SECRET", "test-client-secret")
        .args([
            "export",
            file.to_str().unwrap(),
            "--endpoint",
            &url,
            "--page-size",
            "2",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert_eq!(stdout, "exported 3\n");
    let text = std::fs::read_to_string(&file).unwrap();
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("\"card_key\":\"a\"") && !lines[0].contains("\"id\""));
    assert!(lines[1].contains("\"card_key\":\"b\"") && lines[1].contains("\"response\":\"memo\""));
    assert!(lines[2].contains("\"card_key\":\"c\""));
    let hits = seen.lock().unwrap();
    assert!(hits.iter().all(|hit| hit.access));
    assert_eq!(
        hits.iter().map(|hit| hit.path.as_str()).collect::<Vec<_>>(),
        [
            "/api/export/reviews?after=0&limit=2",
            "/api/export/reviews?after=2&limit=2",
        ]
    );
}

#[test]
fn export_keeps_the_file_when_the_second_page_fails() {
    let (url, seen) = serve(vec![(200, PAGE1.to_owned()), (500, "boom".to_owned())]);
    let dir = scratch("fail");
    let file = dir.join("reviews.jsonl");
    std::fs::write(&file, "keep\n").unwrap();
    let assert = poko()
        .args([
            "export",
            file.to_str().unwrap(),
            "--endpoint",
            &url,
            "--page-size",
            "2",
        ])
        .assert()
        .code(1);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(stderr.contains("500"), "{stderr}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "keep\n");
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["reviews.jsonl".to_owned()]);
    assert_eq!(seen.lock().unwrap().len(), 2);
}
