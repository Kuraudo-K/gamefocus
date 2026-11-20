//! Client HTTP ProtonDB.

use crate::domain::proton::parse_proton_summary_json;
use crate::domain::ProtonSummary;
use crate::ports::ProtonLookup;

const API: &str = "https://www.protondb.com/api/v1/reports/summaries";

pub struct ProtonDbHttp {
    timeout_secs: u64,
}

impl ProtonDbHttp {
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            timeout_secs: timeout_secs.max(1),
        }
    }
}

impl ProtonLookup for ProtonDbHttp {
    fn lookup(&self, app_id: u32) -> Result<ProtonSummary, String> {
        let url = format!("{API}/{app_id}.json");
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(self.timeout_secs))
            .user_agent("gamefocus/0.1 (GLF; +https://gaming-linux.fr)")
            .build();

        let resp = agent
            .get(&url)
            .call()
            .map_err(|e| format!("ProtonDB HTTP {app_id}: {e}"))?;

        if resp.status() == 404 {
            return Err(format!("pas de rapport ProtonDB pour appid {app_id}"));
        }
        if resp.status() >= 400 {
            return Err(format!("ProtonDB status {}", resp.status()));
        }

        let raw = resp.into_string().map_err(|e| e.to_string())?;
        parse_proton_summary_json(app_id, &raw)
    }
}

/// Lookup en mémoire (tests / démo offline).
pub struct StaticProtonLookup {
    entries: Vec<ProtonSummary>,
}

impl StaticProtonLookup {
    pub fn new(entries: Vec<ProtonSummary>) -> Self {
        Self { entries }
    }
}

impl ProtonLookup for StaticProtonLookup {
    fn lookup(&self, app_id: u32) -> Result<ProtonSummary, String> {
        self.entries
            .iter()
            .find(|e| e.app_id == app_id)
            .cloned()
            .ok_or_else(|| format!("pas de résumé local pour {app_id}"))
    }
}
