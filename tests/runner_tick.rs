use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use route_warden::runner::{Runner, RunnerHooks};

#[derive(Clone, Default)]
struct MockHooks {
    calls: Arc<Mutex<Vec<String>>>,
}

impl RunnerHooks for MockHooks {
    fn fetch_groups(&self) -> Result<Vec<String>> {
        self.calls.lock().unwrap().push("fetch_groups".to_string());
        Ok(vec!["GOOGLE_GROUP".to_string(), "GLOBAL_BEST".to_string()])
    }

    fn probe_group(&self, group: &str) -> Result<()> {
        self.calls.lock().unwrap().push(format!("probe:{group}"));
        Ok(())
    }

    fn maybe_switch_group(&self, group: &str) -> Result<()> {
        self.calls.lock().unwrap().push(format!("switch:{group}"));
        Ok(())
    }

    fn persist_round(&self) -> Result<()> {
        self.calls.lock().unwrap().push("persist_round".to_string());
        Ok(())
    }
}

#[test]
fn one_tick_runs_probe_score_select_pipeline() {
    let hooks = MockHooks::default();
    let recorder = hooks.calls.clone();
    let runner = Runner::new(hooks, Duration::from_secs(3));

    runner.tick().unwrap();

    let calls = recorder.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec![
            "fetch_groups",
            "probe:GOOGLE_GROUP",
            "switch:GOOGLE_GROUP",
            "probe:GLOBAL_BEST",
            "switch:GLOBAL_BEST",
            "persist_round",
        ]
    );
}
