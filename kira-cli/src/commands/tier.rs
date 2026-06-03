#![forbid(unsafe_code)]

//! /tier command — switch LLM tier (heavy, standard, light).

use super::DispatchResult;

pub fn execute(args: &str) -> DispatchResult {
    match args.to_lowercase().as_str() {
        "heavy" => DispatchResult::Immediate(Some("tier switched to: heavy".to_string())),
        "standard" => DispatchResult::Immediate(Some("tier switched to: standard".to_string())),
        "light" => DispatchResult::Immediate(Some("tier switched to: light".to_string())),
        "" => DispatchResult::Error("usage: /tier <heavy|standard|light>".to_string()),
        other => DispatchResult::Error(format!(
            "invalid tier: '{other}'. Valid: heavy, standard, light"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_heavy() {
        match execute("heavy") {
            DispatchResult::Immediate(Some(msg)) => assert!(msg.contains("heavy")),
            _ => panic!("expected Immediate"),
        }
    }

    #[test]
    fn tier_standard() {
        match execute("standard") {
            DispatchResult::Immediate(Some(msg)) => assert!(msg.contains("standard")),
            _ => panic!("expected Immediate"),
        }
    }

    #[test]
    fn tier_light() {
        match execute("light") {
            DispatchResult::Immediate(Some(msg)) => assert!(msg.contains("light")),
            _ => panic!("expected Immediate"),
        }
    }

    #[test]
    fn tier_invalid() {
        match execute("turbo") {
            DispatchResult::Error(msg) => assert!(msg.contains("invalid tier")),
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn tier_empty() {
        match execute("") {
            DispatchResult::Error(msg) => assert!(msg.contains("usage")),
            _ => panic!("expected Error"),
        }
    }
}
