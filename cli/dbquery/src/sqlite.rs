use std::path::Path;

use anyhow::Context;
use itertools::Itertools;
use rusqlite::{Connection, OpenFlags};

use crate::{
    paging::Paging,
    search::{Case, Search, SearchType},
};

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

impl Search for SqliteDb {
    fn search(&mut self, mut pattern: String, typ: SearchType, case: Case) -> Vec<usize> {
        let query = match (typ, case) {
            (SearchType::Like, Case::Sensitive) => {
                pattern = format!("*{pattern}*");
                "SELECT id FROM messages \
                    WHERE Datetime GLOB ?1 \
                    OR ECUID GLOB ?1 \
                    OR VERS GLOB ?1 \
                    OR SID GLOB ?1 \
                    OR MCNT GLOB ?1 \
                    OR TMS GLOB ?1 \
                    OR EID GLOB ?1 \
                    OR APID GLOB ?1 \
                    OR CTID GLOB ?1 \
                    OR MSTP GLOB ?1 \
                    OR PAYLOAD GLOB ?1"
            }
            (SearchType::Like, Case::Insensitive) => {
                pattern = format!("%{pattern}%");
                "SELECT id FROM messages \
                    WHERE Datetime LIKE ?1 \
                    OR ECUID LIKE ?1 \
                    OR VERS LIKE ?1 \
                    OR SID LIKE ?1 \
                    OR MCNT LIKE ?1 \
                    OR TMS LIKE ?1 \
                    OR EID LIKE ?1 \
                    OR APID LIKE ?1 \
                    OR CTID LIKE ?1 \
                    OR MSTP LIKE ?1 \
                    OR PAYLOAD LIKE ?1"
            }
            (SearchType::Regex, Case::Sensitive) => todo!(),
            (SearchType::Regex, Case::Insensitive) => todo!(),
        };

        let mut stmt = self.connection.prepare_cached(query).unwrap();
        let rows = stmt.query_map([pattern], |row| row.get(0)).unwrap();

        let mut ids = Vec::new();
        for id in rows {
            ids.push(id.unwrap());
        }

        ids
    }
}
