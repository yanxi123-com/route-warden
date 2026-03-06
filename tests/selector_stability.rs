use route_warden::select::{Decision, DecisionInput, make_decision};

#[test]
fn does_not_switch_without_consecutive_wins() {
    let input = DecisionInput {
        current_node: "NodeA".to_string(),
        candidate_node: "NodeB".to_string(),
        current_score: 0.70,
        candidate_score: 0.90,
        consecutive_candidate_wins: 1,
        consecutive_current_failures: 0,
        min_wins: 3,
        min_improvement: 0.15,
        cooldown_sec: 600,
        last_switch_ts: Some(1_000),
        now_ts: 2_000,
        emergency_failures: 5,
    };

    let decision = make_decision(&input);
    assert!(matches!(
        decision,
        Decision::Keep {
            reason
        } if reason == "not_enough_consecutive_wins"
    ));
}

#[test]
fn switches_when_conditions_met() {
    let input = DecisionInput {
        current_node: "NodeA".to_string(),
        candidate_node: "NodeB".to_string(),
        current_score: 0.60,
        candidate_score: 0.90,
        consecutive_candidate_wins: 3,
        consecutive_current_failures: 0,
        min_wins: 3,
        min_improvement: 0.15,
        cooldown_sec: 600,
        last_switch_ts: Some(100),
        now_ts: 1_000,
        emergency_failures: 5,
    };

    let decision = make_decision(&input);
    assert_eq!(
        decision,
        Decision::Switch {
            from: "NodeA".to_string(),
            to: "NodeB".to_string(),
            reason: "stable_better_candidate".to_string()
        }
    );
}

#[test]
fn emergency_failover_ignores_cooldown() {
    let input = DecisionInput {
        current_node: "NodeA".to_string(),
        candidate_node: "NodeB".to_string(),
        current_score: 0.80,
        candidate_score: 0.81,
        consecutive_candidate_wins: 0,
        consecutive_current_failures: 6,
        min_wins: 3,
        min_improvement: 0.15,
        cooldown_sec: 600,
        last_switch_ts: Some(9_900),
        now_ts: 10_000,
        emergency_failures: 5,
    };

    let decision = make_decision(&input);
    assert_eq!(
        decision,
        Decision::Switch {
            from: "NodeA".to_string(),
            to: "NodeB".to_string(),
            reason: "emergency_failover".to_string()
        }
    );
}
