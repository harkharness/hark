//! The cheap lane's runner, said in the agent's own vocabulary.

use crate::domain::agents::{self, AgentEntry};
use crate::domain::intent::Models;
use crate::ports::{AgentRunner, TurnRequest};
use hark_agent::{AgentEvent, TurnResult};

/// The cheap lane asks for a TIER ("haiku" is the global table's light);
/// each agent calls that tier something else. Translated here, once, so
/// the ask sites (voice, intent router, gate — app and CLI alike) never
/// learn agent vocabularies. No id for the tier = the agent's own default
/// model (an empty `model`).
pub struct TieredRunner {
    pub inner: Box<dyn AgentRunner + Send + Sync>,
    pub entry: AgentEntry,
    pub tiers: Models,
}

impl AgentRunner for TieredRunner {
    fn ask(
        &self,
        request: &TurnRequest,
        on_event: &mut dyn FnMut(&AgentEvent),
    ) -> anyhow::Result<TurnResult> {
        let model = agents::model_id(&self.entry, request.model, &self.tiers).unwrap_or_default();
        self.inner.ask(&TurnRequest { model: &model, ..*request }, on_event)
    }
}
