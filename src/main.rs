use std::fs::File;
use std::io::BufReader;

use anyhow::{Context, Result};

use payments_engine::{engine, reader, writer};

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("Usage: payments-engine <transactions.csv>")?;

    let file = File::open(&path).with_context(|| format!("Cannot open file: {path}"))?;
    let buf = BufReader::new(file);

    let mut engine = engine::TransactionEngine::new();

    for result in reader::stream_transactions(buf) {
        match result {
            Ok(record) => {
                if let Err(e) = engine.process(record) {
                    eprintln!("warn: {e}");
                }
            }
            Err(e) => eprintln!("warn: skipping malformed row: {e}"),
        }
    }

    writer::write_accounts(std::io::stdout(), engine.accounts())
        .context("Failed to write output")?;

    Ok(())
}
