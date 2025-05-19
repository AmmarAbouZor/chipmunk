use clap::Parser as _;
use cli_args::OutputFormat;
use tokio_util::sync::CancellationToken;

use session::{
    format::{
        binary::MsgBinaryWriter, duckdb::MsgDuckDbWriter, sqlite::MsgSqliteWriter,
        text::MsgTextWriter,
    },
    start_session,
};

mod cli_args;
mod session;

/// Runs the app parsing and validating the arguments, then starting the matching
/// session keeping track to cancel calls provided by [`cancel_token`].
pub async fn run_app(cancel_token: CancellationToken) -> anyhow::Result<()> {
    let cli = cli_args::Cli::parse();
    cli.validate()?;

    match cli.parser {
        cli_args::Parser::Dlt { fibex_files, input } => {
            // Create DLT parser.
            let with_storage_header = match &input {
                cli_args::InputSource::Tcp { .. } | cli_args::InputSource::Udp { .. } => false,
                cli_args::InputSource::File { .. } => true,
            };

            let fibex_metadata = session::parser::dlt::create_fibex_metadata(fibex_files);

            let parser = parsers::dlt::DltParser::new(
                None,
                fibex_metadata.as_ref(),
                None,
                None,
                with_storage_header,
            );

            use parsers::dlt::fmt;
            // Move to next part initializing the input source and starting the session.
            match cli.output_format {
                OutputFormat::Binary => {
                    let binary_writer = MsgBinaryWriter::new(&cli.output_path)?;

                    start_session(parser, input, binary_writer, cancel_token).await?;
                }
                OutputFormat::Text => {
                    let text_writer = MsgTextWriter::new(
                        &cli.output_path,
                        fmt::DLT_COLUMN_SENTINAL,
                        fmt::DLT_ARGUMENT_SENTINAL,
                        cli.text_columns_separator,
                        cli.text_args_separator,
                    )?;

                    start_session(parser, input, text_writer, cancel_token).await?;
                }
                OutputFormat::SQLite => {
                    let parser_info = session::parser::dlt::get_parser_info();
                    let sqlit_writer = MsgSqliteWriter::new(
                        &cli.output_path,
                        parser_info,
                        fmt::DLT_COLUMN_SENTINAL,
                    )
                    .await?;

                    start_session(parser, input, sqlit_writer, cancel_token).await?;
                }
                OutputFormat::DuckDB => {
                    let parser_info = session::parser::dlt::get_parser_info();
                    let duckdb_writer = MsgDuckDbWriter::new(
                        &cli.output_path,
                        parser_info,
                        fmt::DLT_COLUMN_SENTINAL,
                    )?;

                    start_session(parser, input, duckdb_writer, cancel_token).await?;
                }
            };
        }
    }

    Ok(())
}
