///TODO AAZ: Docs and unit tests
/// Includes structures and methods to provide SQL queries to be used with `duckdb` crate.
use crate::session::parser::{ColumnInfo, ParserInfo};

pub const MESSAGES_TABLE_NAME: &str = "messages";

/// Structure to include SQL queries
///
/// # NOTE:
/// We are using `rowid` as line indexes to avoid handling the current index manually.
/// Otherwise, we must handle the index manually because the `Appender` doesn't support
/// default values in `duckdb.rs`.
/// Using `rowid` as index is valid here because we will never delete any existing row.
#[derive(Debug, Clone)]
pub struct SqlQueries {
    /// SQL query to generate the table for log messages.
    ///
    /// The query will follow the format:
    /// `CREATE TABLE IF NOT EXISTS messages (id INTEGER PRIMARY KEY, col1 TEXT, col2 TEXT)`
    pub create_msg_table: String,
}

impl SqlQueries {
    /// Create a new instance generating sql queries from the provided [`ParserInfo`]
    pub fn new(parser_info: &ParserInfo) -> Self {
        let create_msg_table =
            Self::generate_create_table(MESSAGES_TABLE_NAME, &parser_info.columns);

        Self { create_msg_table }
    }

    /// Generate SQL query to be used to create a table to be used with
    /// functions from `duckdb` crate.
    ///
    /// The generated query will follow the format:
    /// `CREATE TABLE IF NOT EXISTS my_table (col1 TEXT, col2 TEXT)`
    fn generate_create_table(table_name: &str, columns: &[ColumnInfo]) -> String {
        let mut query = format!("CREATE TABLE IF NOT EXISTS {table_name} (",);

        use std::fmt::Write;
        let mut cols = columns.iter();
        let Some(first) = cols.next() else {
            query.push(')');

            return query;
        };

        let _ = write!(&mut query, "{} TEXT", first.caption);

        cols.for_each(|col| {
            let _ = write!(&mut query, ", {} TEXT", col.caption);
        });

        query.push(')');

        query
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
        let expected_query = "CREATE TABLE IF NOT EXISTS my_table (col1 TEXT)";

        assert_eq!(expected_query, gen_query.as_str());
    }

    #[test]
    fn gen_create_table_two_cols() {
        let table_name = "my_table";
        let cols = vec![ColumnInfo::new("col1"), ColumnInfo::new("col2")];

        let gen_query = SqlQueries::generate_create_table(table_name, &cols);
        let expected_query = "CREATE TABLE IF NOT EXISTS my_table (col1 TEXT, col2 TEXT)";

        assert_eq!(expected_query, gen_query.as_str());
    }

    #[test]
    fn gen_create_table_empty_cols() {
        let table_name = "my_table";
        let cols = Vec::new();

        let gen_query = SqlQueries::generate_create_table(table_name, &cols);
        let expected_query = "CREATE TABLE IF NOT EXISTS my_table ()";

        assert_eq!(expected_query, gen_query.as_str());
    }
}
