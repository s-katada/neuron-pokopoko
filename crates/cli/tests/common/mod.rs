#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use assert_cmd::Command;

pub struct Hit {
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Hit {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    pub fn has_test_access(&self) -> bool {
        self.header("cf-access-client-id") == Some("test-client-id")
            && self.header("cf-access-client-secret") == Some("test-client-secret")
    }
}

pub fn poko() -> Command {
    let mut command = Command::cargo_bin("poko").unwrap();
    command.env_remove("POKO_ENDPOINT");
    command.env_remove("POKO_ACCESS_CLIENT_ID");
    command.env_remove("POKO_ACCESS_CLIENT_SECRET");
    command
}

/// すべてのリクエストに同じ応答を返す。
pub fn serve(status: u16, location: Option<&str>, body: &str) -> (String, Arc<Mutex<Vec<Hit>>>) {
    let location = location.map(str::to_owned);
    let body = body.to_owned();
    spawn(move |index| {
        let _ = index;
        (status, location.clone(), body.clone())
    })
}

/// リクエスト順に応答を返す。足りなくなったら 500。
pub fn serve_seq(responses: Vec<(u16, String)>) -> (String, Arc<Mutex<Vec<Hit>>>) {
    spawn(move |index| {
        let (status, body) = responses
            .get(index)
            .cloned()
            .unwrap_or((500, "extra".to_owned()));
        (status, None, body)
    })
}

fn spawn<F>(mut response_for: F) -> (String, Arc<Mutex<Vec<Hit>>>)
where
    F: FnMut(usize) -> (u16, Option<String>, String) + Send + 'static,
{
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
            let Some(hit) = read_request(&mut stream) else {
                continue;
            };
            record.lock().unwrap().push(hit);
            let (status, location, body) = response_for(index);
            index += 1;
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

fn read_request(stream: &mut impl Read) -> Option<Hit> {
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
    let head = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
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
            buffer.extend_from_slice(&chunk[..read]);
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
    let body = String::from_utf8_lossy(&buffer[header_end..]).into_owned();
    Some(Hit {
        path,
        headers,
        body,
    })
}
