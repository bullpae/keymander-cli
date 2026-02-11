//! `kmd history` — view and manage launch history

use color_eyre::Result;

pub enum Action {
    List(usize, bool),
    Clear,
}

pub fn run(action: Option<Action>) -> Result<()> {
    let db = super::open_db()?;

    match action.unwrap_or(Action::List(20, false)) {
        Action::List(limit, json) => {
            let history = db.query_history(limit);

            if json {
                let entries: Vec<serde_json::Value> = history
                    .iter()
                    .map(|h| {
                        serde_json::json!({
                            "display": h.display,
                            "value": h.value,
                            "type": h.item_type,
                            "frequency": h.frequency,
                            "last_used": h.executed_at,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else if history.is_empty() {
                println!("No history yet.");
            } else {
                println!("Recent launches:\n");
                for (i, h) in history.iter().enumerate() {
                    println!(
                        "  {:>2}. {:<30} [{:>3}x]  {}",
                        i + 1,
                        h.display,
                        h.frequency,
                        h.executed_at,
                    );
                }
            }
        }
        Action::Clear => {
            db.clear_history()?;
            println!("History cleared.");
        }
    }

    Ok(())
}
