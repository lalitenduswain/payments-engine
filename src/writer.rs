use std::io::Write;

use crate::account::AccountStore;

pub fn write_accounts<W: Write>(writer: W, accounts: &AccountStore) -> Result<(), csv::Error> {
    let mut wtr = csv::WriterBuilder::new().from_writer(writer);
    wtr.write_record(["client", "available", "held", "total", "locked"])?;

    let mut sorted: Vec<_> = accounts.iter().collect();
    sorted.sort_by_key(|a| a.client_id);

    for account in sorted {
        wtr.write_record([
            account.client_id.to_string(),
            format!("{:.4}", account.available),
            format!("{:.4}", account.held),
            format!("{:.4}", account.total()),
            account.locked.to_string(),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}
