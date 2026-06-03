#![forbid(unsafe_code)]

//! /mode command — switch between YOLO and Plan modes.

use super::DispatchResult;

pub fn execute(args: &str) -> DispatchResult {
    match args.to_lowercase().as_str() {
        "yolo" => DispatchResult::Immediate(Some("mode switched to: yolo".to_string())),
        "plan" => DispatchResult::Immediate(Some("mode switched to: plan".to_string())),
        "" => DispatchResult::Error("usage: /mode <yolo|plan>".to_string()),
        other => DispatchResult::Error(format!("invalid mode: '{other}'. Valid: yolo, plan")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_yolo() {
        match execute("yolo") {
            DispatchResult::Immediate(Some(msg)) => assert!(msg.contains("yolo")),
            _ => panic!("expected Immediate"),
        }
    }

    #[test]
    fn mode_plan() {
        match execute("plan") {
            DispatchResult::Immediate(Some(msg)) => assert!(msg.contains("plan")),
            _ => panic!("expected Immediate"),
        }
    }

    #[test]
    fn mode_invalid() {
        match execute("invalid") {
            DispatchResult::Error(msg) => assert!(msg.contains("invalid mode")),
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn mode_empty() {
        match execute("") {
            DispatchResult::Error(msg) => assert!(msg.contains("usage")),
            _ => panic!("expected Error"),
        }
    }
}
