use std::{path::Path, sync::OnceLock};

use anyhow::Context;
use itertools::Itertools;
use regex::{Regex, RegexBuilder};
use rusqlite::{Connection, OpenFlags, functions::FunctionFlags};

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

        let mut db = Self { connection };

        //TODO AAZ: Added here temporally.
        db.create_regex_funcs();

        Ok(db)
    }

    fn create_regex_funcs(&mut self) {
        self.connection
            .create_scalar_function(
                "my_regex_s",
                -1,
                FunctionFlags::SQLITE_UTF8
                    | FunctionFlags::SQLITE_DETERMINISTIC
                    | FunctionFlags::SQLITE_DIRECTONLY
                    | FunctionFlags::SQLITE_INNOCUOUS,
                |ctx| {
                    static REG: OnceLock<Regex> = OnceLock::new();

                    // Dirty solution to avoid creating regex on each call.
                    // This may be solved in production with global dictionary.
                    // This solution can work if we call the search on the database once only.
                    let regex = REG.get_or_init(|| {
                        let reg: &str = ctx.get_raw(0).as_str().unwrap();

                        Regex::new(reg).unwrap()
                    });
                    // let regex = Regex::new(reg).unwrap();

                    for idx in 1..ctx.len() {
                        let text = match ctx.get_raw(idx) {
                            rusqlite::types::ValueRef::Text(bytes) => {
                                std::str::from_utf8(bytes).unwrap()
                                // This provide better performance
                                // unsafe { std::str::from_utf8_unchecked(bytes) }
                            }
                            rusqlite::types::ValueRef::Null => continue,
                            invalid => panic!("Invalid {invalid:?}"),
                        };
                        if regex.is_match(text) {
                            return Ok(true);
                        }
                    }

                    Ok(false)
                },
            )
            .unwrap();

        self.connection
            .create_scalar_function(
                "my_regex_i",
                -1,
                FunctionFlags::SQLITE_UTF8
                    | FunctionFlags::SQLITE_DETERMINISTIC
                    | FunctionFlags::SQLITE_DIRECTONLY
                    | FunctionFlags::SQLITE_INNOCUOUS,
                |ctx| {
                    static REG: OnceLock<Regex> = OnceLock::new();

                    // Dirty solution to avoid creating regex on each call.
                    // This may be solved in production with global dictionary.
                    // This solution can work if we call the search on the database once only.
                    let regex = REG.get_or_init(|| {
                        let reg: &str = ctx.get_raw(0).as_str().unwrap();
                        RegexBuilder::new(reg)
                            .case_insensitive(true)
                            .build()
                            .unwrap()
                    });
                    // let regex = Regex::new(reg).unwrap();

                    for idx in 1..ctx.len() {
                        let text = match ctx.get_raw(idx) {
                            rusqlite::types::ValueRef::Text(bytes) => {
                                std::str::from_utf8(bytes).unwrap()
                            }
                            rusqlite::types::ValueRef::Null => continue,
                            invalid => panic!("Invalid {invalid:?}"),
                        };
                        if regex.is_match(text) {
                            return Ok(true);
                        }
                    }

                    Ok(false)
                },
            )
            .unwrap();
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
            (SearchType::Regex, Case::Sensitive) => {
                "SELECT id FROM messages \
                    WHERE my_regex_s(?1, Datetime, ECUID, VERS, SID, MCNT, TMS, EID, APID, CTID, MSTP, PAYLOAD)"
            }
            (SearchType::Regex, Case::Insensitive) => {
                "SELECT id FROM messages \
                    WHERE my_regex_i(?1, Datetime, ECUID, VERS, SID, MCNT, TMS, EID, APID, CTID, MSTP, PAYLOAD)"
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
