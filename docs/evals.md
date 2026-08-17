# Evals

`crates/evals` measures the planner layer against a labelled command
dataset. Run it:

```sh
cargo run -p neuralnav-evals --bin run-evals
```

## What is measured

| Metric | Definition |
| --- | --- |
| **Intent accuracy** | heuristic intent classification == expected label |
| **Clarification accuracy** | plan contains an `ask_user` node exactly when the case expects one |
| **Graph validity** | every produced plan passes `TaskGraph::validate` |
| **p50 plan latency** | median wall-clock of `Planner::plan` |

## Dataset

10 labelled commands in `evals/src/dataset.rs`: the canonical shopping demo,
simple navigations (`open youtube`, `go to wikipedia`), search tasks,
composite tasks, and deliberately ambiguous inputs (`open the second one`,
`stop`) that must trigger clarification, not guessing.

## Quality bars (enforced in CI via `cargo test`)

`runner::tests::mock_planner_clears_quality_bars` asserts:

- intent accuracy ≥ 90 %
- clarification accuracy ≥ 90 %
- graph validity = 100 %

Current mock-planner results: **10/10 intent, 10/10 clarification, 100 %
validity, p50 plan latency < 1 ms.**

Beyond the planner harness, the integration tests measure the system-level
behaviors the UI advertises:

- full demo wall-clock **< 8 s** (`demo_runs_end_to_end_under_eight_seconds`)
- stop settles **< 500 ms** (`stop_cancels_the_run_immediately`)
- demo replayability (`navigation_is_replayable`)

## Extending

Add cases to `default_dataset()`; to eval the Gemini planner, pass a
`GeminiPlanner` to `run_planner_evals` (same trait). The report serializes
to JSON for dashboards.
