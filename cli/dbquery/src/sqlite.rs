use std::path::Path;

use anyhow::Context;
use itertools::Itertools;
use rusqlite::Connection;

use crate::paging::Paging;

pub struct SqliteDb {
    connection: Connection,
}

impl SqliteDb {
    pub fn create(db_path: &Path) -> anyhow::Result<Self> {
        let connection = Connection::open(db_path).context("Error while connecting to database")?;

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
