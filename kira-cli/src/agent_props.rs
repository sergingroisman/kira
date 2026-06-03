#![forbid(unsafe_code)]

//! Property-based tests for multi-agent output prefixing (Property 12)
//! and agent prefix matching (Property 8).

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::app::{AppState, Mode, OutputLineKind, Tier};
    use crate::commands::{PrefixMatch, find_by_prefix};
    use crate::config::Config;
    use crate::messages::AgentThread;

    // We need access to apply_message from tui.rs — but it's in the binary crate.
    // Instead, we replicate the core logic inline for testing since apply_message
    // is defined in the binary crate (tui.rs). The logic under test is:
    //   if state.active_agents.len() > 1 { format!("[{short_id}] {line}") } else { line }

    /// Apply the AgentOutput message logic (extracted from tui.rs apply_message).
    /// This replicates the exact logic for testability without depending on the binary crate.
    fn apply_agent_output(state: &mut AppState, agent_id: &str, line: &str) {
        // Update last_output for the agent
        if let Some(agent) = state.active_agents.iter_mut().find(|a| a.id == agent_id) {
            agent.last_output = Some(line.to_string());
        }

        // Prefix with short ID if multiple agents active
        let formatted = if state.active_agents.len() > 1 {
            let short_id: String = agent_id.chars().take(8).collect();
            format!("[{short_id}] {line}")
        } else {
            line.to_string()
        };

        state.push_output(formatted, OutputLineKind::Agent);
    }

    /// Generate a valid UUID v4 string.
    fn uuid_strategy() -> impl Strategy<Value = String> {
        // Generate 32 hex chars and format as UUID
        prop::collection::vec(
            prop::sample::select(vec![
                '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
            ]),
            32,
        )
        .prop_map(|chars| {
            format!(
                "{}-{}-4{}-{}-{}",
                chars[0..8].iter().collect::<String>(),
                chars[8..12].iter().collect::<String>(),
                chars[12..15].iter().collect::<String>(),
                chars[15..19].iter().collect::<String>(),
                chars[19..31].iter().collect::<String>(),
            )
        })
    }

    /// Create an AgentThread from a UUID string.
    fn make_agent(id: &str) -> AgentThread {
        AgentThread {
            id: id.to_string(),
            short_id: id.chars().take(8).collect(),
            kind: "writer".to_string(),
            status: "running".to_string(),
            cost_usd: 0.0,
            last_output: None,
        }
    }

    // ── Feature: tui-wiring, Property 12: Multi-Agent Output Prefixing ────────
    // **Validates: Requirements 6.5**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 12a: When multiple agents are active, the output line is
        /// prefixed with `[{first_8_chars_of_agent_id}] `.
        #[test]
        fn prop_multi_agent_output_prefixed(
            agent_id in uuid_strategy(),
            other_agent_id in uuid_strategy(),
            line in "[a-zA-Z0-9 _.!?]{1,100}",
        ) {
            // Only test when the two UUIDs are different
            prop_assume!(agent_id != other_agent_id);

            let config = Config::default();
            let mut state = AppState::new(config, Mode::Yolo, Tier::Standard);

            // Add two agents to make multiple active
            state.active_agents.push(make_agent(&agent_id));
            state.active_agents.push(make_agent(&other_agent_id));

            // Apply agent output
            apply_agent_output(&mut state, &agent_id, &line);

            // Verify the output line is prefixed with [short_id]
            let short_id: String = agent_id.chars().take(8).collect();
            let expected_prefix = format!("[{short_id}] ");
            let last_output = &state.output.last().unwrap().text;

            prop_assert!(
                last_output.starts_with(&expected_prefix),
                "Expected output to start with '{}', got '{}'",
                expected_prefix,
                last_output
            );
            // Verify the rest of the line is the original content
            let after_prefix = &last_output[expected_prefix.len()..];
            prop_assert_eq!(after_prefix, &line);
        }

        /// Property 12b: When only one agent is active, the output line is NOT
        /// prefixed (passed through as-is).
        #[test]
        fn prop_single_agent_output_not_prefixed(
            agent_id in uuid_strategy(),
            line in "[a-zA-Z0-9 _.!?]{1,100}",
        ) {
            let config = Config::default();
            let mut state = AppState::new(config, Mode::Yolo, Tier::Standard);

            // Add only one agent
            state.active_agents.push(make_agent(&agent_id));

            // Apply agent output
            apply_agent_output(&mut state, &agent_id, &line);

            // Verify the output line is NOT prefixed
            let last_output = &state.output.last().unwrap().text;
            prop_assert_eq!(last_output, &line);
        }
    }

    // ── Feature: tui-wiring, Property 8: Agent Prefix Matching ────────────────
    // **Validates: Requirements 5.7, 5.10**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 8a: When exactly one agent's ID starts with the prefix,
        /// find_by_prefix returns Found with that agent's full ID.
        #[test]
        fn prop_prefix_match_found_exactly_one(
            uuids in prop::collection::vec(uuid_strategy(), 2..=10),
            prefix_len in 1usize..=8usize,
        ) {
            // Pick the first UUID and use a prefix of it
            let target = &uuids[0];
            let prefix: String = target.chars().take(prefix_len).collect();

            // Filter to only agents whose id or short_id starts with this prefix
            let agents: Vec<AgentThread> = uuids.iter().map(|id| make_agent(id)).collect();
            let matching_count = agents.iter()
                .filter(|a| a.id.starts_with(&prefix) || a.short_id.starts_with(&prefix))
                .count();

            let result = find_by_prefix(&agents, &prefix);

            match matching_count {
                0 => prop_assert_eq!(result, PrefixMatch::NotFound),
                1 => {
                    // Should be Found with the matching agent's full ID
                    match &result {
                        PrefixMatch::Found(found_id) => {
                            // Verify the found ID actually starts with the prefix
                            let agent = agents.iter().find(|a| a.id == *found_id).unwrap();
                            prop_assert!(
                                agent.id.starts_with(&prefix) || agent.short_id.starts_with(&prefix),
                                "Found agent doesn't match prefix"
                            );
                        }
                        other => prop_assert!(false, "Expected Found, got {:?}", other),
                    }
                }
                n => prop_assert_eq!(result, PrefixMatch::Ambiguous(n)),
            }
        }

        /// Property 8b: When no agent's ID starts with the prefix,
        /// find_by_prefix returns NotFound.
        #[test]
        fn prop_prefix_match_not_found(
            uuids in prop::collection::vec(uuid_strategy(), 1..=10),
        ) {
            let agents: Vec<AgentThread> = uuids.iter().map(|id| make_agent(id)).collect();

            // Use a prefix that cannot match any UUID (starts with 'z' which isn't hex)
            let prefix = "zzzzz";
            let result = find_by_prefix(&agents, prefix);
            prop_assert_eq!(result, PrefixMatch::NotFound);
        }

        /// Property 8c: When multiple agents share the same prefix,
        /// find_by_prefix returns Ambiguous with the correct count.
        #[test]
        fn prop_prefix_match_ambiguous(
            shared_prefix in "[0-9a-f]{1,4}",
            suffix1 in "[0-9a-f]{4,27}",
            suffix2 in "[0-9a-f]{4,27}",
            other_uuids in prop::collection::vec(uuid_strategy(), 0..=5),
        ) {
            prop_assume!(suffix1 != suffix2);

            // Create two UUIDs that share the same prefix
            let id1 = format_uuid_from_hex(&format!("{shared_prefix}{suffix1}"));
            let id2 = format_uuid_from_hex(&format!("{shared_prefix}{suffix2}"));

            let mut agents: Vec<AgentThread> = vec![make_agent(&id1), make_agent(&id2)];

            // Add other agents that should NOT match the shared prefix
            for uuid in &other_uuids {
                if !uuid.starts_with(&shared_prefix)
                    && !uuid.chars().take(8).collect::<String>().starts_with(&shared_prefix)
                {
                    agents.push(make_agent(uuid));
                }
            }

            let result = find_by_prefix(&agents, &shared_prefix);

            // Count how many actually match
            let actual_matches = agents.iter()
                .filter(|a| a.id.starts_with(&shared_prefix) || a.short_id.starts_with(&shared_prefix))
                .count();

            match actual_matches.cmp(&1) {
                std::cmp::Ordering::Greater => {
                    prop_assert_eq!(result, PrefixMatch::Ambiguous(actual_matches));
                }
                std::cmp::Ordering::Equal => {
                    match &result {
                        PrefixMatch::Found(_) => {} // ok
                        other => prop_assert!(false, "Expected Found, got {:?}", other),
                    }
                }
                std::cmp::Ordering::Less => {
                    prop_assert_eq!(result, PrefixMatch::NotFound);
                }
            }
        }

        /// Property 8d: find_by_prefix matches against both full id and short_id.
        /// A prefix that matches the short_id (first 8 chars) should still find the agent.
        #[test]
        fn prop_prefix_matches_short_id(
            agent_id in uuid_strategy(),
            prefix_len in 1usize..=8usize,
        ) {
            let agent = make_agent(&agent_id);
            let short_id = &agent.short_id;
            let prefix: String = short_id.chars().take(prefix_len).collect();

            let agents = vec![agent];
            let result = find_by_prefix(&agents, &prefix);

            // Since there's only one agent and the prefix matches its short_id,
            // it should be Found
            match &result {
                PrefixMatch::Found(found_id) => {
                    prop_assert_eq!(found_id, &agent_id);
                }
                other => prop_assert!(false, "Expected Found, got {:?}", other),
            }
        }
    }

    /// Format a hex string (at least 31 chars) into UUID format.
    /// Takes the first 31 hex chars and formats as: 8-4-4-4-12
    fn format_uuid_from_hex(hex: &str) -> String {
        // Pad to at least 32 hex chars
        let padded: String = if hex.len() < 32 {
            format!("{hex:0<32}")
        } else {
            hex.chars().take(32).collect()
        };

        format!(
            "{}-{}-{}-{}-{}",
            &padded[0..8],
            &padded[8..12],
            &padded[12..16],
            &padded[16..20],
            &padded[20..32],
        )
    }
}
