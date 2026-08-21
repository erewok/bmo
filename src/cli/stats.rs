use crate::db::{Repository, find_db, open_db};
use crate::output::{OutputMode, make_printer};

pub fn run(json: bool, db: Option<String>) -> anyhow::Result<()> {
    let db_path = find_db(db.as_deref())?;
    let repo = open_db(&db_path)?;
    let printer = make_printer(if json {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    let stats = repo.get_stats()?;
    printer.print_stats(&stats);
    Ok(())
}
