/*!
 * JSON export of the whole database.
 *
 * Reads every user table (the internal `sqlite_*` tables are skipped) and
 * writes it as a JSON document:
 *
 * ```json
 * { "tables": [ { "name": "task", "columns": ["id", "title"], "rows": [ { "id": 1, "title": "x" } ] } ] }
 * ```
 *
 * Only `rusqlite`, `serde` and `serde_json` are used. `serde` describes the
 * shape of the data, `serde_json` writes it as JSON text.
 */

use rusqlite::Connection;
use rusqlite::types::ValueRef;
use serde::{Serialize, Serializer};

use super::DataBase;
use super::database::quoteIdentifier;
use super::error::DbError;

/** One SQLite value, written by serde as a bare JSON value (`1`, `"a"`, `null`). */
#[derive(Serialize)]
#[serde(untagged)]
enum Cell {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    /* Blobs are written as an array of byte values. */
    Blob(Vec<u8>),
}

impl From<ValueRef<'_>> for Cell {
    /** Convert a raw SQLite value into its JSON counterpart. */
    fn from(value: ValueRef<'_>) -> Self {
        match value {
            ValueRef::Null => Cell::Null,
            ValueRef::Integer(number) => Cell::Integer(number),
            ValueRef::Real(number) => Cell::Real(number),
            ValueRef::Text(bytes) => Cell::Text(String::from_utf8_lossy(bytes).into_owned()),
            ValueRef::Blob(bytes) => Cell::Blob(bytes.to_vec()),
        }
    }
}

/** One table row: column names paired with values, kept in column order. */
struct Row(Vec<(String, Cell)>);

impl Serialize for Row {
    /** Write the row as a JSON object, preserving the column order. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.iter().map(|(name, cell)| (name, cell)))
    }
}

/** A table: its name, its columns (kept even when empty) and all its rows. */
#[derive(Serialize)]
struct Table {
    name: String,
    columns: Vec<String>,
    rows: Vec<Row>,
}

/** The whole database. */
#[derive(Serialize)]
struct DatabaseDump {
    tables: Vec<Table>,
}

/**
 * Export the whole database as a pretty-printed JSON string.
 *
 * Every user table is exported with all its rows. The connection lock is
 * held only while the data is read.
 *
 * # Example
 * ```rust,ignore
 * let json: String = export_json(&db)?;
 * std::fs::write("export.json", json)?;
 * ```
 */
pub fn exportJson(db: &DataBase) -> Result<String, DbError> {
    let dump: DatabaseDump = {
        let connection = db.lock();
        dumpDatabase(&connection)?
    };

    Ok(serde_json::to_string_pretty(&dump)?)
}

/** Read every user table into a serializable structure. */
fn dumpDatabase(connection: &Connection) -> rusqlite::Result<DatabaseDump> {
    let tables: Vec<Table> = table_names(connection)?
        .into_iter()
        .map(|name| read_table(connection, name))
        .collect::<rusqlite::Result<Vec<Table>>>()?;

    Ok(DatabaseDump { tables })
}

/** List the names of all user tables, sorted alphabetically. */
fn table_names(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_master \
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
         ORDER BY name",
    )?;
    let names: Vec<String> = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;

    Ok(names)
}

/** Read the columns and every row of one table. */
fn read_table(connection: &Connection, name: String) -> rusqlite::Result<Table> {
    let mut statement = connection.prepare(&format!("SELECT * FROM {}", quoteIdentifier(&name)))?;
    let columns: Vec<String> = statement
        .column_names()
        .into_iter()
        .map(String::from)
        .collect();
    let rows: Vec<Row> = statement
        .query_map([], |row| read_row(row, &columns))?
        .collect::<rusqlite::Result<Vec<Row>>>()?;

    Ok(Table {
        name,
        columns,
        rows,
    })
}

/** Convert one SQLite row into a `Row`, using `columns` as the keys. */
fn read_row(row: &rusqlite::Row<'_>, columns: &[String]) -> rusqlite::Result<Row> {
    columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            row.get_ref(index)
                .map(|value| (column.clone(), Cell::from(value)))
        })
        .collect::<rusqlite::Result<Vec<(String, Cell)>>>()
        .map(Row)
}
