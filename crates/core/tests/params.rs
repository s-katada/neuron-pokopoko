use poko_core::{Statement, Value, fsrs_params_query, upsert_fsrs_params_statement};
use rusqlite::{Connection, params_from_iter};

mod common;

#[test]
fn upsert_keeps_one_row_and_the_later_values() {
    let conn = Connection::open_in_memory().unwrap();
    common::apply_migrations(&conn);
    let first = format!("[{}]", ["1.0"; 21].join(","));
    exec(&conn, &upsert_fsrs_params_statement(&first, 3, 10));
    let later: Vec<String> = (1..=21).map(|index| format!("{index}.0")).collect();
    let second = format!("[{}]", later.join(","));
    exec(&conn, &upsert_fsrs_params_statement(&second, 9, 20));

    let n: i64 = conn
        .query_row("SELECT count(*) FROM fsrs_params", [], |row| row.get(0))
        .unwrap();
    assert_eq!(n, 1);
    let statement = fsrs_params_query();
    let (params, review_count, updated_at): (String, i64, i64) = conn
        .query_row(&statement.sql, [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap();
    assert_eq!(params, second);
    assert_eq!(review_count, 9);
    assert_eq!(updated_at, 20);
}

fn exec(conn: &Connection, statement: &Statement) {
    let params: Vec<_> = statement.params.iter().map(to_sql).collect();
    conn.prepare(&statement.sql)
        .unwrap()
        .execute(params_from_iter(params))
        .unwrap();
}

fn to_sql(value: &Value) -> rusqlite::types::Value {
    match value {
        Value::Null => rusqlite::types::Value::Null,
        Value::Integer(n) => rusqlite::types::Value::Integer(*n),
        Value::Real(n) => rusqlite::types::Value::Real(*n),
        Value::Text(text) => rusqlite::types::Value::Text(text.clone()),
    }
}
