/// Includes structures and methods to provide SQL queries to be used with `rusqlite` crate.
use crate::session::parser::{ColumnInfo, ParserInfo};
use std::fmt::Write;

pub const MESSAGES_TABLE_NAME: &str = "messages";

/// Structure to include SQL queries
#[derive(Debug, Clone)]
pub struct SqlQueries {
    /// SQL query to generate the table for log messages.
    ///
    /// The query will follow the format:
    /// `CREATE TABLE IF NOT EXISTS messages (id INTEGER PRIMARY KEY, col1 TEXT, col2 TEXT)`
    pub create_msg_table: String,

    /// SQL query to insert values into log messages table.
    ///
    /// The query will follow the format:
    /// `INSERT INTO messages (col1, col2) VALUES (?1, ?2)`
    pub insert_msg: String,
}

impl SqlQueries {
    /// Create a new instance generating sql queries from the provided [`ParserInfo`]
    pub fn new(parser_info: &ParserInfo) -> Self {
        let create_msg_table =
            Self::generate_create_table(MESSAGES_TABLE_NAME, &parser_info.columns);
        let insert_msg = Self::generate_insert_sql(MESSAGES_TABLE_NAME, &parser_info.columns);

        Self {
            create_msg_table,
            insert_msg,
        }
    }

    /// Generate SQL query to be used to create a table to be used with
    /// functions from `rusqlite` crate.
    ///
    /// The generated query will follow the format:
    /// `CREATE TABLE IF NOT EXISTS my_table (id INTEGER PRIMARY KEY, col1 TEXT, col2 TEXT)`
    fn generate_create_table(table_name: &str, columns: &[ColumnInfo]) -> String {
        let mut query = format!(
            "CREATE TABLE IF NOT EXISTS {table_name} (\
                id INTEGER PRIMARY KEY",
        );
        use std::fmt::Write;
        columns.iter().for_each(|col| {
            let _ = write!(&mut query, ", {} TEXT", col.caption);
        });

        query.push(')');

        query
    }

    /// Generate SQL query to be used to insert values into the table to be used
    /// with functions from `rusqlite` crate.
    ///
    /// The generated query will follow the format:
    /// `INSERT INTO my_table (col1, col2) VALUES (?1, ?2)`
    fn generate_insert_sql(table_name: &str, columns: &[ColumnInfo]) -> String {
        let mut sql = format!("INSERT INTO {table_name} (");
        let mut captions = columns.iter().map(|col| col.caption.as_str());
        let Some(first_col) = captions.next() else {
            sql.push_str(") VALUES ()");
            return sql;
        };

        sql.push_str(first_col);
        for capt in captions {
            sql.push_str(", ");
            sql.push_str(capt);
        }

        sql.push_str(") VALUES (?1");

        for num in 2..=columns.len() {
            let _ = write!(&mut sql, ", ?{num}");
        }

        sql.push(')');

        sql
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gen_create_table_one_col() {
        let table_name = "my_table";
        let cols = vec![ColumnInfo::new("col1")];

        let gen_query = SqlQueries::generate_create_table(table_name, &cols);
        let expected_query =
            "CREATE TABLE IF NOT EXISTS my_table (id INTEGER PRIMARY KEY, col1 TEXT)";

        assert_eq!(expected_query, gen_query.as_str());
    }

    #[test]
    fn gen_create_table_two_cols() {
        let table_name = "my_table";
        let cols = vec![ColumnInfo::new("col1"), ColumnInfo::new("col2")];

        let gen_query = SqlQueries::generate_create_table(table_name, &cols);
        let expected_query =
            "CREATE TABLE IF NOT EXISTS my_table (id INTEGER PRIMARY KEY, col1 TEXT, col2 TEXT)";

        assert_eq!(expected_query, gen_query.as_str());
    }

    #[test]
    fn gen_create_table_empty_cols() {
        let table_name = "my_table";
        let cols = Vec::new();

        let gen_query = SqlQueries::generate_create_table(table_name, &cols);
        let expected_query = "CREATE TABLE IF NOT EXISTS my_table (id INTEGER PRIMARY KEY)";

        assert_eq!(expected_query, gen_query.as_str());
    }

    #[test]
    fn gen_insert_sql_query_one_col() {
        let table_name = "my_table";
        let cols = vec![ColumnInfo::new("col1")];

        let gen_query = SqlQueries::generate_insert_sql(table_name, &cols);
        let expected_query = "INSERT INTO my_table (col1) VALUES (?1)";

        assert_eq!(expected_query, gen_query.as_str());
    }

    #[test]
    fn gen_insert_sql_query_two_cols() {
        let table_name = "my_table";
        let cols = vec![ColumnInfo::new("col1"), ColumnInfo::new("col2")];

        let gen_query = SqlQueries::generate_insert_sql(table_name, &cols);
        let expected_query = "INSERT INTO my_table (col1, col2) VALUES (?1, ?2)";

        assert_eq!(expected_query, gen_query.as_str());
    }

    #[test]
    fn gen_insert_sql_query_empty_cols() {
        let table_name = "my_table";
        let cols = Vec::new();

        let gen_query = SqlQueries::generate_insert_sql(table_name, &cols);
        let expected_query = "INSERT INTO my_table () VALUES ()";

        assert_eq!(expected_query, gen_query.as_str());
    }
}
