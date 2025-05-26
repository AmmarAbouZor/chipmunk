use std::path::Path;

use anyhow::Context;
use duckdb::Connection;
use itertools::Itertools;

use crate::paging::Paging;

#[derive(Debug)]
pub struct DuckDb {
    connection: Connection,
}

impl DuckDb {
    pub fn create(db_path: &Path) -> anyhow::Result<Self> {
        let connection = Connection::open(db_path).context("Error while connecting to database")?;

        let db = Self { connection };

        Ok(db)
    }
}

impl Paging for DuckDb {
    fn records_count(&mut self) -> usize {
        self.connection
            .query_row("SELECT MAX(rowid) FROM messages", [], |row| row.get(0))
            .unwrap()
    }

    fn load_records(&mut self, start: usize, end: usize) -> Vec<String> {
        // let limit = end - start;
        // let mut stmt = self
        //     .connection
        //     .prepare_cached("SELECT * from messages LIMIT ?1 OFFSET ?2")
        //     .unwrap();
        // let mut rows = stmt.query([limit, start]).unwrap();

        let mut stmt = self
            .connection
            .prepare_cached("SELECT * from messages WHERE rowid >= ?1 AND rowid <= ?2")
            .unwrap();

        let mut rows = stmt.query([start, end]).unwrap();

        let mut res = Vec::with_capacity(end - start);

        while let Some(row) = rows.next().unwrap() {
            let msg: String = Itertools::intersperse(
                (0..=10).map(|col| row.get_ref(col).unwrap().as_str().unwrap()),
                ";",
            )
            .collect();
            res.push(msg);
        }
        res
    }
}
