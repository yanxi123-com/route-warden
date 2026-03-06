#[derive(Debug, Clone)]
pub struct DecisionInput {
    pub current_node: String,
    pub candidate_node: String,
    pub current_score: f64,
    pub candidate_score: f64,
    pub consecutive_candidate_wins: u32,
    pub consecutive_current_failures: u32,
    pub min_wins: u32,
    pub min_improvement: f64,
    pub cooldown_sec: u64,
    pub last_switch_ts: Option<i64>,
    pub now_ts: i64,
    pub emergency_failures: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Keep { reason: String },
    Switch { from: String, to: String, reason: String },
}

pub fn make_decision(input: &DecisionInput) -> Decision {
    if input.current_node == input.candidate_node {
        return Decision::Keep {
            reason: "candidate_same_as_current".to_string(),
        };
    }

    if input.consecutive_current_failures >= input.emergency_failures {
        return Decision::Switch {
            from: input.current_node.clone(),
            to: input.candidate_node.clone(),
            reason: "emergency_failover".to_string(),
        };
    }

    if input.consecutive_candidate_wins < input.min_wins {
        return Decision::Keep {
            reason: "not_enough_consecutive_wins".to_string(),
        };
    }

    let improvement = relative_improvement(input.current_score, input.candidate_score);
    if improvement < input.min_improvement {
        return Decision::Keep {
            reason: "improvement_below_threshold".to_string(),
        };
    }

    if !cooldown_passed(input.last_switch_ts, input.now_ts, input.cooldown_sec) {
        return Decision::Keep {
            reason: "cooldown_not_passed".to_string(),
        };
    }

    Decision::Switch {
        from: input.current_node.clone(),
        to: input.candidate_node.clone(),
        reason: "stable_better_candidate".to_string(),
    }
}

fn relative_improvement(current: f64, candidate: f64) -> f64 {
    if candidate <= current {
        return 0.0;
    }
    if current.abs() < f64::EPSILON {
        return 1.0;
    }
    (candidate - current) / current.abs()
}

fn cooldown_passed(last_switch_ts: Option<i64>, now_ts: i64, cooldown_sec: u64) -> bool {
    match last_switch_ts {
        None => true,
        Some(last) => {
            let elapsed = now_ts.saturating_sub(last);
            elapsed >= cooldown_sec as i64
        }
    }
}
