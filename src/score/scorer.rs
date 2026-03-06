#[derive(Debug, Clone)]
pub struct NodeStats {
    pub node: String,
    pub total: usize,
    pub success: usize,
    pub latencies_ms: Vec<f64>,
    pub consecutive_failures: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct ScoreWeights {
    pub availability_weight: f64,
    pub p50_weight: f64,
    pub p95_weight: f64,
    pub jitter_weight: f64,
    pub failure_penalty_weight: f64,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            availability_weight: 0.7,
            p50_weight: 0.15,
            p95_weight: 0.1,
            jitter_weight: 0.05,
            failure_penalty_weight: 0.2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeScore {
    pub node: String,
    pub score: f64,
    pub availability: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub jitter_ms: f64,
    pub penalty: f64,
}

pub fn score_nodes(stats: &[NodeStats], weights: ScoreWeights) -> Vec<NodeScore> {
    let mut scores: Vec<NodeScore> = stats
        .iter()
        .map(|item| {
            let availability = if item.total == 0 {
                0.0
            } else {
                item.success as f64 / item.total as f64
            };
            let p50_ms = percentile(&item.latencies_ms, 0.50);
            let p95_ms = percentile(&item.latencies_ms, 0.95);
            let jitter_ms = stddev(&item.latencies_ms);

            let availability_score = availability;
            let p50_score = invert_ms(p50_ms);
            let p95_score = invert_ms(p95_ms);
            let jitter_score = invert_ms(jitter_ms);
            let penalty =
                (item.consecutive_failures as f64 * 0.1).min(1.0) * weights.failure_penalty_weight;

            let score = weights.availability_weight * availability_score
                + weights.p50_weight * p50_score
                + weights.p95_weight * p95_score
                + weights.jitter_weight * jitter_score
                - penalty;

            NodeScore {
                node: item.node.clone(),
                score,
                availability,
                p50_ms,
                p95_ms,
                jitter_ms,
                penalty,
            }
        })
        .collect();

    scores.sort_by(|a, b| b.score.total_cmp(&a.score));
    scores
}

fn invert_ms(value: f64) -> f64 {
    if value <= 0.0 {
        return 1.0;
    }
    1.0 / (1.0 + value / 1000.0)
}

fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return f64::INFINITY;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let rank = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[rank]
}

fn stddev(values: &[f64]) -> f64 {
    if values.len() <= 1 {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| {
            let diff = value - mean;
            diff * diff
        })
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt()
}
