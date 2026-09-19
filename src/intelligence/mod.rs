//! Observation & intelligence layer (concept §7–§9).
//!
//! S2 adds the content-free activity ledger ([`ledger`]); S7–S9 add patterns, baseline,
//! balance, behaviour and knowledge on the ambient cron infrastructure.

pub mod ledger;

pub use ledger::{ActivityEvent, LedgerQuery, LedgerService};
