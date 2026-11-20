//! Suggestions d'options de lancement (règles métier pures).

use serde::{Deserialize, Serialize};

use super::proton::ProtonSummary;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaunchProfile {
    pub label: String,
    pub command: String,
    pub notes: String,
}

/// Construit des profils optionnels sans déduire de variables Proton d'un tier communautaire.
pub fn suggest_launch_profiles(summary: Option<&ProtonSummary>, _heavy: bool) -> Vec<LaunchProfile> {
    // `_heavy` conservé pour compatibilité API ; le profil « jeu lourd » a été retiré
    // volontairement (pas d’injection de variables Proton risquées).
    suggest_launch_profiles_for_game(summary, "")
}

pub fn suggest_launch_profiles_for_game(
    summary: Option<&ProtonSummary>,
    game_name: &str,
) -> Vec<LaunchProfile> {
    let mut profiles = vec![
        LaunchProfile {
        label: "GameMode (facultatif)".into(),
        command: "gamemoderun %command%".into(),
        notes: "Ajoute le wrapper GameMode au lancement du jeu. Nécessite GameMode; ne garantit pas de gain de FPS.".into(),
        },
        LaunchProfile {
        label: "Debug HUD".into(),
        command: "mangohud %command%".into(),
        notes: "Affiche les mesures FPS et ressources; nécessite MangoHud.".into(),
        },
    ];
    if summary.is_some_and(|summary| summary.app_id == 1_091_500)
        || game_name.to_lowercase().contains("cyberpunk 2077")
    {
        profiles.push(LaunchProfile {
            label: "Cyberpunk 2077 — test GameMode + MangoHud".into(),
            command: "gamemoderun mangohud %command%".into(),
            notes: "Profil facultatif de test : GameMode et l’overlay MangoHud. Nécessite ces outils et n’est pas appliqué automatiquement.".into(),
        });
    }
    profiles
}

pub fn suggest_launch_command(summary: Option<&ProtonSummary>, heavy: bool) -> String {
    suggest_launch_profiles(summary, heavy)[0].command.clone()
}

pub fn is_heavy_game(name: &str) -> bool {
    let n = name.to_lowercase();
    [
        "cyberpunk",
        "red dead",
        "rdr2",
        "star citizen",
        "microsoft flight",
        "alan wake",
        "hogwarts",
        "avatar",
        "black myth",
        "wukong",
        "stalker 2",
        "indiana jones",
    ]
    .iter()
    .any(|k| n.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heavy_detects_cyberpunk() {
        assert!(is_heavy_game("Cyberpunk 2077"));
        assert!(!is_heavy_game("Celeste"));
    }

    #[test]
    fn protondb_tier_does_not_infer_unsafe_launch_variables() {
        let s = ProtonSummary {
            app_id: 1,
            tier: "bronze".into(),
            total: 10,
            trending_tier: None,
            best_reported_tier: None,
            confidence: None,
            score: None,
        };
        let cmd = suggest_launch_command(Some(&s), false);
        assert_eq!(cmd, "gamemoderun %command%");
        let profiles = suggest_launch_profiles(Some(&s), false);
        assert!(!profiles.iter().any(|profile| {
            profile.command.contains("WINED3D") || profile.command.contains("NO_ESYNC")
        }));
    }

    #[test]
    fn gold_tier_does_not_change_generic_profiles() {
        let s = ProtonSummary {
            app_id: 1091500,
            tier: "gold".into(),
            total: 5000,
            trending_tier: Some("platinum".into()),
            best_reported_tier: None,
            confidence: Some("strong".into()),
            score: Some(0.8),
        };
        let profiles = suggest_launch_profiles(Some(&s), true);
        assert_eq!(profiles[0].command, "gamemoderun %command%");
    }

    #[test]
    fn cyberpunk_has_an_opt_in_test_profile_without_changing_default() {
        let summary = ProtonSummary {
            app_id: 1_091_500,
            tier: "platinum".into(),
            total: 1_000,
            trending_tier: None,
            best_reported_tier: None,
            confidence: None,
            score: None,
        };
        let profiles = suggest_launch_profiles(Some(&summary), true);
        assert_eq!(profiles[0].command, "gamemoderun %command%");
        assert!(profiles.iter().any(|profile| {
            profile.label.contains("Cyberpunk")
                && profile.command == "gamemoderun mangohud %command%"
                && profile.notes.contains("pas appliqué automatiquement")
        }));
    }

    #[test]
    fn cyberpunk_name_selects_test_profile_without_a_proton_lookup() {
        let profiles = suggest_launch_profiles_for_game(None, "Cyberpunk 2077");
        assert!(profiles
            .iter()
            .any(|profile| profile.label.contains("Cyberpunk")));
        assert_eq!(suggest_launch_command(None, true), "gamemoderun %command%");
    }

    #[test]
    fn silver_tier_does_not_disable_esync_by_default() {
        let s = ProtonSummary {
            app_id: 2,
            tier: "silver".into(),
            total: 100,
            trending_tier: None,
            best_reported_tier: None,
            confidence: None,
            score: None,
        };
        let profiles = suggest_launch_profiles(Some(&s), false);
        assert_eq!(profiles[0].command, "gamemoderun %command%");
        assert!(!profiles[0].command.contains("ESYNC"));
    }

    #[test]
    fn no_proton_still_has_balanced() {
        let profiles = suggest_launch_profiles(None, false);
        assert!(profiles.iter().any(|p| p.label.contains("GameMode")));
        assert!(!profiles.iter().any(|p| p.label.contains("Optimal")));
        assert!(!profiles.iter().any(|p| p.label.contains("Jeu lourd")));
    }

    #[test]
    fn heavy_keywords() {
        assert!(is_heavy_game("Red Dead Redemption 2"));
        assert!(is_heavy_game("Black Myth: Wukong"));
        assert!(is_heavy_game("Alan Wake 2"));
        assert!(!is_heavy_game("Stardew Valley"));
    }

    #[test]
    fn borked_tier_does_not_force_wined3d() {
        let s = ProtonSummary {
            app_id: 3,
            tier: "borked".into(),
            total: 5,
            trending_tier: None,
            best_reported_tier: None,
            confidence: None,
            score: None,
        };
        let profiles = suggest_launch_profiles(Some(&s), false);
        assert_eq!(profiles[0].command, "gamemoderun %command%");
        assert!(!profiles[0].command.contains("WINED3D"));
    }
}
