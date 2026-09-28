//! List prices of the Claude models, for ESTIMATES only.
//!
//! The ledger's dollars come from the CLI (`total_cost_usd`, `costUSD`),
//! never from here: the CLI prices with its own table, corrected by rules
//! this one does not know (data residency, a managed price table). What a
//! table of our own buys is a number BEFORE the CLI has one — the running
//! cost of a turn in flight, which is what a per-turn ceiling needs, and
//! what a message is about to cost, which is what a pre-flight needs.
//! Every figure derived here is marked as an estimate where it is shown.
//!
//! Dollars per million tokens. Cache writes are 1.25× the input price for
//! the 5-minute TTL and 2× for the 1-hour TTL; reads are 0.1× except where
//! a model prices them apart. Source: the Claude API pricing page, as of
//! 2026-06. A model missing here is unpriced, not free.

/// Dollars per million tokens of one model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write_5m: f64,
    pub cache_write_1h: f64,
}

/// The tokens of one API call, cache writes split by the TTL they were
/// written with — the two are priced 1.6× apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CallTokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write_5m: u64,
    pub cache_write_1h: u64,
}

const fn price(input: f64, output: f64, cache_read: f64) -> Price {
    Price { input, output, cache_read, cache_write_5m: input * 1.25, cache_write_1h: input * 2.0 }
}

const TABLE: &[(&str, Price)] = &[
    ("claude-fable-5-1", price(10.0, 50.0, 0.25)),
    ("claude-fable-5", price(10.0, 50.0, 0.25)),
    ("claude-mythos-5-1", price(10.0, 50.0, 0.25)),
    ("claude-opus-5-5", price(4.0, 20.0, 0.20)),
    ("claude-opus-5", price(5.0, 25.0, 0.5)),
    ("claude-opus-4-8", price(5.0, 25.0, 0.5)),
    ("claude-opus-4-7", price(5.0, 25.0, 0.5)),
    ("claude-opus-4-6", price(5.0, 25.0, 0.5)),
    ("claude-sonnet-5", price(2.0, 10.0, 0.2)),
    ("claude-sonnet-4-6", price(3.0, 15.0, 0.3)),
    ("claude-haiku-4-5", price(1.0, 5.0, 0.1)),
];

/// The list price of a model, by the id the CLI reports: the window tag
/// (`claude-opus-5[1m]`) and a dated snapshot suffix
/// (`claude-haiku-4-5-20251001`) name the same price as the bare id.
pub fn lookup(model: &str) -> Option<Price> {
    let bare = model.split('[').next().unwrap_or_default().trim();
    let undated = match bare.rsplit_once('-') {
        Some((head, tail)) if tail.len() == 8 && tail.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => bare,
    };
    TABLE.iter().find(|(id, _)| *id == undated).map(|(_, p)| *p)
}

/// What one call cost at list price.
pub fn cost(price: &Price, tokens: &CallTokens) -> f64 {
    let per_million = |n: u64, rate: f64| n as f64 * rate / 1_000_000.0;
    per_million(tokens.input, price.input)
        + per_million(tokens.output, price.output)
        + per_million(tokens.cache_read, price.cache_read)
        + per_million(tokens.cache_write_5m, price.cache_write_5m)
        + per_million(tokens.cache_write_1h, price.cache_write_1h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn a_model_is_priced_by_the_id_the_cli_reports() {
        let p = lookup("claude-fable-5-1").expect("priced");
        assert!(close(p.input, 10.0) && close(p.output, 50.0));
        assert!(close(p.cache_read, 0.25), "Fable reads at a quarter dollar, not 0.1×");
        assert!(close(p.cache_write_5m, 12.5) && close(p.cache_write_1h, 20.0));
    }

    #[test]
    fn the_window_tag_and_the_date_suffix_are_not_part_of_the_name() {
        // The ledger has "claude-opus-5[1m]" and "claude-haiku-4-5-20251001".
        assert_eq!(lookup("claude-opus-5[1m]"), lookup("claude-opus-5"));
        assert_eq!(lookup("claude-haiku-4-5-20251001"), lookup("claude-haiku-4-5"));
        assert!(lookup("claude-opus-5").is_some());
    }

    #[test]
    fn a_point_release_is_not_its_predecessor() {
        let five = lookup("claude-opus-5").unwrap();
        let five_five = lookup("claude-opus-5-5").unwrap();
        assert!(close(five.input, 5.0) && close(five_five.input, 4.0));
    }

    #[test]
    fn an_unknown_model_is_unpriced_not_free() {
        assert_eq!(lookup("unknown"), None);
        assert_eq!(lookup("gemini-2.5-pro"), None);
        assert_eq!(lookup(""), None);
    }

    #[test]
    fn a_cold_resume_of_a_million_tokens_on_fable_costs_twenty_dollars() {
        // 25/09: the first call of the resumed terminal session, on the
        // subscription's 1-hour cache — what a $2 ceiling never saw coming.
        let p = lookup("claude-fable-5-1").unwrap();
        let cold = CallTokens { input: 2_600, cache_write_1h: 1_000_000, output: 1_500, ..Default::default() };
        let usd = cost(&p, &cold);
        assert!(usd > 20.0 && usd < 20.2, "{usd}");
        // Warm, the same context is a quarter dollar.
        let warm = CallTokens { input: 2_600, cache_read: 1_000_000, output: 1_500, ..Default::default() };
        let usd = cost(&p, &warm);
        assert!(usd > 0.35 && usd < 0.36, "{usd}");
    }

    #[test]
    fn each_kind_of_token_is_priced_at_its_own_rate() {
        let p = lookup("claude-sonnet-5").unwrap();
        let t = CallTokens { input: 1_000_000, output: 1_000_000, cache_read: 1_000_000, cache_write_5m: 1_000_000, cache_write_1h: 1_000_000 };
        assert!(close(cost(&p, &t), 2.0 + 10.0 + 0.2 + 2.5 + 4.0));
        assert!(close(cost(&p, &CallTokens::default()), 0.0));
    }
}
