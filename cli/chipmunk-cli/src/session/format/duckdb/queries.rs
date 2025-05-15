///TODO AAZ: Docs and unit tests
/// Includes structures and methods to provide SQL queries to be used with `duckdb` crate.
use crate::session::parser::{ColumnInfo, ParserInfo};

pub const MESSAGES_TABLE_NAME: &str = "messages";
const ID_COLUMN_NAME: &str = "id";

/// Structure to include SQL queries
#[derive(Debug, Clone)]
pub struct SqlQueries {
    /// SQL query to generate the table for log messages.
    ///
    /// The query will follow the format:
    /// `CREATE TABLE IF NOT EXISTS messages (id INTEGER PRIMARY KEY, col1 TEXT, col2 TEXT)`
    pub create_msg_table: String,

    /// SQL query to get the max index for messages database.
    ///
    /// The query will follow the format:
    /// `SELECT MAX(id) FROM messages;`
    pub last_msg_idx: String,
}

impl SqlQueries {
    /// Create a new instance generating sql queries from the provided [`ParserInfo`]
    pub fn new(parser_info: &ParserInfo) -> Self {
        let create_msg_table =
            Self::generate_create_table(MESSAGES_TABLE_NAME, &parser_info.columns);

        let last_msg_idx = Self::generate_max_id(MESSAGES_TABLE_NAME);

        Self {
            create_msg_table,
            last_msg_idx,
        }
    }

    /// Generate SQL query to be used to create a table to be used with
    /// functions from `duckdb` crate.
    ///
    /// The generated query will follow the format:
    /// `CREATE TABLE IF NOT EXISTS my_table (id INTEGER PRIMARY KEY, col1 TEXT, col2 TEXT)`
    fn generate_create_table(table_name: &str, columns: &[ColumnInfo]) -> String {
        let mut query = format!(
            "CREATE TABLE IF NOT EXISTS {table_name} (\
                {ID_COLUMN_NAME} INTEGER PRIMARY KEY",
        );

        use std::fmt::Write;
        columns.iter().for_each(|col| {
            let _ = write!(&mut query, ", {} TEXT", col.caption);
        });

        query.push(')');

        query
    }

    /// Generate SQL query to get the max index for the provided table name
    /// in the database.
    ///
    /// The generated query will follow the format:
    /// `SELECT MAX(id) FROM my_table;`
    fn generate_max_id(table_name: &str) -> String {
        format!("SELECT MAX(id) FROM {table_name};",)
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
    fn get_max_id() {
        let table_name = "my_table";

        let gen_query = SqlQueries::generate_max_id(table_name);
        let expected_query = "SELECT MAX(id) FROM my_table;";

        assert_eq!(expected_query, gen_query.as_str());
    }
}
