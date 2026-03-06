use route_warden::score::{NodeStats, ScoreWeights, score_nodes};

#[test]
fn availability_beats_latency() {
    let fast_but_unstable = NodeStats {
        node: "NodeA".to_string(),
        total: 10,
        success: 4,
        latencies_ms: vec![20.0, 22.0, 19.0, 18.0],
        consecutive_failures: 3,
    };

    let slower_but_stable = NodeStats {
        node: "NodeB".to_string(),
        total: 10,
        success: 10,
        latencies_ms: vec![120.0, 130.0, 125.0, 128.0, 122.0],
        consecutive_failures: 0,
    };

    let ranked = score_nodes(
        &[fast_but_unstable, slower_but_stable],
        ScoreWeights::default(),
    );

    assert_eq!(ranked[0].node, "NodeB");
    assert!(ranked[0].score > ranked[1].score);
}
