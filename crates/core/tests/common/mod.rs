use std::fs;
use std::path::PathBuf;

use rusqlite::Connection;

pub fn apply_migrations(conn: &Connection) {
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations");
    let mut paths: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("sql"))
        .collect();
    paths.sort();
    for path in paths {
        let sql = fs::read_to_string(&path).unwrap();
        conn.execute_batch(&sql).unwrap();
    }
}
