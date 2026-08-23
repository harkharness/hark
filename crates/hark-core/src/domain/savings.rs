//! The savings meter: what the architecture AVOIDED spending, computed
//! from the ledger with the formulas stated in the report itself. Honest
//! by construction — every counter carries its own methodology sentence,
//! estimates are labeled, empty windows yield zeros (never NaN).

/// Ledger-derived inputs for one window.
#[derive(Debug, Clone, Copy, Default)]
pub struct SavingsInputs {
    /// Dispatches the gate intercepted (outcome `gate:meta_hark`).
    pub gate_blocked: u64,
    /// What the gate itself cost in the window.
    pub gate_cost_usd: f64,
    /// Average cost of a real worker turn (live rows).
    pub avg_worker_turn_usd: f64,
    /// Questions answered by the zero-token local layer (model = "local").
    pub local_answers: u64,
    /// Average cost of a full ask turn (live rows).
    pub avg_ask_usd: f64,
    /// Cache tokens READ instead of re-sent (live rows).
    pub cache_read_tokens: u64,
    /// Ledger-derived USD per input token (0 when the window has no data).
    pub usd_per_input_token: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SavingsReport {
    pub avoided_gate_usd: f64,
    pub avoided_local_usd: f64,
    /// Estimate — cache pricing is not billed linearly; labeled as such.
    pub avoided_cache_usd: f64,
    pub total_usd: f64,
    /// One sentence per counter, the formula in words. The UI shows THIS.
    pub methodology: Vec<String>,
}

/// Compute the report. Floors at zero: a gate that cost more than it
/// saved reports 0, not a negative saving.
pub fn compute(i: &SavingsInputs) -> SavingsReport {
    let gate = (i.gate_blocked as f64 * i.avg_worker_turn_usd - i.gate_cost_usd).max(0.0);
    let local = i.local_answers as f64 * i.avg_ask_usd;
    let cache = i.cache_read_tokens as f64 * i.usd_per_input_token;
    SavingsReport {
        avoided_gate_usd: gate,
        avoided_local_usd: local,
        avoided_cache_usd: cache,
        total_usd: gate + local + cache,
        methodology: vec![
            format!(
                "gate: {} despachos interceptados × ${:.4} (turno médio de worker) − ${:.4} gastos pelo próprio gate, piso em zero",
                i.gate_blocked, i.avg_worker_turn_usd, i.gate_cost_usd
            ),
            format!(
                "local: {} respostas da camada determinística × ${:.4} (custo médio de um ask completo)",
                i.local_answers, i.avg_ask_usd
            ),
            format!(
                "cache (estimado): {} tokens lidos do cache × ${:.8}/token derivado do ledger — estimativa, cache não é cobrado linearmente",
                i.cache_read_tokens, i.usd_per_input_token
            ),
            "roteador de modelos: fora do total até haver amostra comparável no ledger".into(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_window_is_all_zeros_never_nan() {
        let r = compute(&SavingsInputs::default());
        assert_eq!(r.total_usd, 0.0);
        assert!(r.avoided_gate_usd == 0.0 && r.avoided_local_usd == 0.0);
        assert!(r.total_usd.is_finite());
        assert!(!r.methodology.is_empty(), "methodology even when empty");
    }

    #[test]
    fn gate_savings_subtract_its_own_cost_and_floor_at_zero() {
        let r = compute(&SavingsInputs {
            gate_blocked: 3,
            gate_cost_usd: 0.05,
            avg_worker_turn_usd: 0.40,
            ..Default::default()
        });
        assert!((r.avoided_gate_usd - 1.15).abs() < 1e-9);

        let negative = compute(&SavingsInputs {
            gate_blocked: 1,
            gate_cost_usd: 0.50,
            avg_worker_turn_usd: 0.10,
            ..Default::default()
        });
        assert_eq!(negative.avoided_gate_usd, 0.0, "never a negative saving");
    }

    #[test]
    fn local_answers_multiply_the_full_ask_average() {
        let r = compute(&SavingsInputs {
            local_answers: 10,
            avg_ask_usd: 0.04,
            ..Default::default()
        });
        assert!((r.avoided_local_usd - 0.40).abs() < 1e-9);
        assert!((r.total_usd - 0.40).abs() < 1e-9);
    }

    #[test]
    fn cache_is_labeled_estimated_in_the_methodology() {
        let r = compute(&SavingsInputs {
            cache_read_tokens: 1_000_000,
            usd_per_input_token: 0.000_003,
            ..Default::default()
        });
        assert!((r.avoided_cache_usd - 3.0).abs() < 1e-9);
        assert!(
            r.methodology.iter().any(|m| m.contains("estimad")),
            "the estimate is labeled"
        );
    }

    #[test]
    fn methodology_states_every_counter_with_its_numbers() {
        let r = compute(&SavingsInputs {
            gate_blocked: 2,
            gate_cost_usd: 0.02,
            avg_worker_turn_usd: 0.30,
            local_answers: 5,
            avg_ask_usd: 0.04,
            cache_read_tokens: 100,
            usd_per_input_token: 0.000_003,
        });
        assert_eq!(r.methodology.len(), 4);
        assert!(r.methodology[0].contains('2') && r.methodology[0].contains("0.30"));
        assert!(r.methodology[1].contains('5'));
    }
}
