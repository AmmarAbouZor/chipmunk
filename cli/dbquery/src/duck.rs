use std::path::Path;

use anyhow::Context;
use duckdb::Connection;
use itertools::Itertools;

use crate::{
    paging::Paging,
    search::{Case, Search, SearchType},
};

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

impl Search for DuckDb {
    fn search(&mut self, mut pattern: String, typ: SearchType, case: Case) -> Vec<usize> {
        let query = match (typ, case) {
            (SearchType::Like, Case::Sensitive) => {
                // This should provide the same results and
                // it could provide slightly better performance.
                // "SELECT rowid FROM messages \
                //         WHERE \
                //         regexp_matches(Datetime, ?1, 'l') OR \
                //         regexp_matches(ECUID, ?1, 'l') OR \
                //         regexp_matches(VERS, ?1, 'l') OR \
                //         regexp_matches(SID, ?1, 'l') OR \
                //         regexp_matches(MCNT, ?1, 'l') OR \
                //         regexp_matches(TMS, ?1, 'l') OR \
                //         regexp_matches(EID, ?1, 'l') OR \
                //         regexp_matches(APID, ?1, 'l') OR \
                //         regexp_matches(CTID, ?1, 'l') OR \
                //         regexp_matches(MSTP, ?1, 'l') OR \
                //         regexp_matches(PAYLOAD, ?1, 'l')"

                "SELECT rowid FROM messages \
                        WHERE \
                        contains(Datetime, ?1) OR \
                        contains(ECUID, ?1) OR \
                        contains(VERS, ?1) OR \
                        contains(SID, ?1) OR \
                        contains(MCNT, ?1) OR \
                        contains(TMS, ?1) OR \
                        contains(EID, ?1) OR \
                        contains(APID, ?1) OR \
                        contains(CTID, ?1) OR \
                        contains(MSTP, ?1) OR \
                        contains(PAYLOAD, ?1)"
            }
            (SearchType::Like, Case::Insensitive) => {
                pattern = format!("%{pattern}%");
                "SELECT rowid FROM messages \
                    WHERE Datetime ILIKE ?1 \
                    OR ECUID ILIKE ?1 \
                    OR VERS ILIKE ?1 \
                    OR SID ILIKE ?1 \
                    OR MCNT ILIKE ?1 \
                    OR TMS ILIKE ?1 \
                    OR EID ILIKE ?1 \
                    OR APID ILIKE ?1 \
                    OR CTID ILIKE ?1 \
                    OR MSTP ILIKE ?1 \
                    OR PAYLOAD ILIKE ?1"
            }
            (SearchType::Regex, Case::Sensitive) => {
                "SELECT rowid FROM messages \
                        WHERE \
                        regexp_matches(Datetime, ?1, 'c') OR \
                        regexp_matches(ECUID, ?1, 'c') OR \
                        regexp_matches(VERS, ?1, 'c') OR \
                        regexp_matches(SID, ?1, 'c') OR \
                        regexp_matches(MCNT, ?1, 'c') OR \
                        regexp_matches(TMS, ?1, 'c') OR \
                        regexp_matches(EID, ?1, 'c') OR \
                        regexp_matches(APID, ?1, 'c') OR \
                        regexp_matches(CTID, ?1, 'c') OR \
                        regexp_matches(MSTP, ?1, 'c') OR \
                        regexp_matches(PAYLOAD, ?1, 'c')"
            }
            (SearchType::Regex, Case::Insensitive) => {
                "SELECT rowid FROM messages \
                        WHERE \
                        regexp_matches(Datetime, ?1, 'i') OR \
                        regexp_matches(ECUID, ?1, 'i') OR \
                        regexp_matches(VERS, ?1, 'i') OR \
                        regexp_matches(SID, ?1, 'i') OR \
                        regexp_matches(MCNT, ?1, 'i') OR \
                        regexp_matches(TMS, ?1, 'i') OR \
                        regexp_matches(EID, ?1, 'i') OR \
                        regexp_matches(APID, ?1, 'i') OR \
                        regexp_matches(CTID, ?1, 'i') OR \
                        regexp_matches(MSTP, ?1, 'i') OR \
                        regexp_matches(PAYLOAD, ?1, 'i')"
            }
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
