//! Résumé ProtonDB (modèle pur).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtonSummary {
    pub app_id: u32,
    pub tier: String,
    pub total: u64,
    pub trending_tier: Option<String>,
    pub best_reported_tier: Option<String>,
    pub confidence: Option<String>,
    pub score: Option<f64>,
}

/// Catégorie visuelle pour le tier ProtonDB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtonTierKind {
    /// Platinum / native — marche très bien
    Good,
    /// Gold — marche
    Ok,
    /// Silver — quelques soucis
    Caution,
    /// Bronze — problèmes
    Bad,
    /// Borked — ne marche pas
    Broken,
    /// Jamais testé / inconnu
    Untested,
}

impl ProtonSummary {
    pub fn tier_kind(&self) -> ProtonTierKind {
        tier_kind(&self.tier)
    }
}

pub fn tier_kind(tier: &str) -> ProtonTierKind {
    match tier.to_lowercase().as_str() {
        "native" | "platinum" => ProtonTierKind::Good,
        "gold" => ProtonTierKind::Ok,
        "silver" => ProtonTierKind::Caution,
        "bronze" => ProtonTierKind::Bad,
        "borked" => ProtonTierKind::Broken,
        _ => ProtonTierKind::Untested,
    }
}

/// RGB UI pour le badge ProtonDB.
pub fn tier_rgb(kind: ProtonTierKind) -> (u8, u8, u8) {
    match kind {
        ProtonTierKind::Good => (80, 200, 120),      // vert vif
        ProtonTierKind::Ok => (120, 200, 100),       // vert
        ProtonTierKind::Caution => (230, 170, 60),   // orange
        ProtonTierKind::Bad => (230, 130, 50),       // orange foncé
        ProtonTierKind::Broken => (220, 70, 70),     // rouge
        ProtonTierKind::Untested => (140, 150, 165), // gris
    }
}

pub fn tier_label_fr(kind: ProtonTierKind) -> &'static str {
    match kind {
        ProtonTierKind::Good => "marche très bien",
        ProtonTierKind::Ok => "marche",
        ProtonTierKind::Caution => "quelques problèmes",
        ProtonTierKind::Bad => "problèmes",
        ProtonTierKind::Broken => "ne marche pas",
        ProtonTierKind::Untested => "jamais testé",
    }
}

/// Parse un JSON ProtonDB déjà téléchargé (tests / cache / adapter).
pub fn parse_proton_summary_json(app_id: u32, raw: &str) -> Result<ProtonSummary, String> {
    #[derive(Deserialize)]
    struct ApiBody {
        tier: Option<String>,
        total: Option<u64>,
        #[serde(rename = "trendingTier")]
        trending_tier: Option<String>,
        #[serde(rename = "bestReportedTier")]
        best_reported_tier: Option<String>,
        confidence: Option<String>,
        score: Option<f64>,
    }

    let body: ApiBody = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    Ok(ProtonSummary {
        app_id,
        tier: body.tier.unwrap_or_else(|| "unknown".into()),
        total: body.total.unwrap_or(0),
        trending_tier: body.trending_tier,
        best_reported_tier: body.best_reported_tier,
        confidence: body.confidence,
        score: body.score,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample() {
        let raw = r#"{
  "tier": "gold",
  "score": 0.78,
  "total": 1234,
  "trendingTier": "platinum",
  "bestReportedTier": "native",
  "confidence": "strong"
}"#;
        let s = parse_proton_summary_json(1091500, raw).unwrap();
        assert_eq!(s.tier, "gold");
        assert_eq!(s.total, 1234);
        assert_eq!(s.trending_tier.as_deref(), Some("platinum"));
    }

    #[test]
    fn parse_minimal_defaults_unknown() {
        let s = parse_proton_summary_json(42, "{}").unwrap();
        assert_eq!(s.app_id, 42);
        assert_eq!(s.tier, "unknown");
        assert_eq!(s.total, 0);
    }

    #[test]
    fn parse_invalid_json_errors() {
        assert!(parse_proton_summary_json(1, "not-json").is_err());
    }

    #[test]
    fn tier_kinds_map_expected() {
        assert_eq!(tier_kind("platinum"), ProtonTierKind::Good);
        assert_eq!(tier_kind("gold"), ProtonTierKind::Ok);
        assert_eq!(tier_kind("silver"), ProtonTierKind::Caution);
        assert_eq!(tier_kind("bronze"), ProtonTierKind::Bad);
        assert_eq!(tier_kind("borked"), ProtonTierKind::Broken);
        assert_eq!(tier_kind("unknown"), ProtonTierKind::Untested);
    }
}
