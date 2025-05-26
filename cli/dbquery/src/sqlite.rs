use std::path::Path;

use anyhow::Context;
use itertools::Itertools;
use rusqlite::{Connection, OpenFlags};

use crate::paging::Paging;

#[derive(Debug)]
pub struct SqliteDb {
    connection: Connection,
}

impl SqliteDb {
    pub fn create(db_path: &Path) -> anyhow::Result<Self> {
        let connection = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_URI,
        )
        .context("Error while connecting to database")?;

        let db = Self { connection };

        Ok(db)
    }
}

impl Paging for SqliteDb {
    fn records_count(&mut self) -> usize {
        self.connection
            .query_row("SELECT MAX(id) FROM messages", [], |row| row.get(0))
            .unwrap()
    }

    fn load_records(&mut self, start: usize, end: usize) -> Vec<String> {
        //NOTE: Using `rowid` instead of generating `id` column wouldn't provide
        //any extra value because sqlite will shadow rowid in case we have another
        //column with `INTEGER PRIMARY KEY`.
        let mut stmt = self
            .connection
            .prepare_cached("SELECT * from messages WHERE id >= ?1 AND id <= ?2")
            .unwrap();

        let mut rows = stmt.query([start, end]).unwrap();

        let mut res = Vec::with_capacity(end - start);

        while let Some(row) = rows.next().unwrap() {
            let msg: String = Itertools::intersperse(
                (1..=11).map(|col| row.get_ref(col).unwrap().as_str().unwrap()),
                ";",
            )
            .collect();
            res.push(msg);
        }
        res
    }
}
