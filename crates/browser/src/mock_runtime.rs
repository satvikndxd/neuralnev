//! Deterministic mock browser runtime.
//!
//! Simulates a small "web" — most importantly the Amazon keyboard-shopping
//! flow used by the canonical demo — with realistic delays, page-state
//! transitions and per-action verification checks. Fully cancellable.

use crate::runtime::BrowserRuntime;
use crate::verifier;
use async_trait::async_trait;
use neuralnav_core::{
    ActionResult, BrowserError, FailureClass, InteractiveElement, NeuralNavAction, PageState,
    ScrollDirection, VerificationCheck, VerificationResult,
};
use serde_json::json;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

const UNFILTERED_COUNT: u64 = 1204;
const FILTERED_COUNT: u64 = 312;

#[derive(Debug, Clone)]
struct Product {
    title: &'static str,
    price: u32,
    rating: f32,
    reviews: u32,
}

const CATALOG: &[Product] = &[
    Product { title: "Cosmic Byte CB-GK-26 Firefly", price: 4299, rating: 4.6, reviews: 2143 },
    Product { title: "Zebronics Max Pro V2", price: 3499, rating: 4.3, reviews: 1876 },
    Product { title: "Redgear Shadow Blade", price: 2799, rating: 4.2, reviews: 3521 },
    Product { title: "TVS Electronics Gold Prime", price: 4999, rating: 4.5, reviews: 987 },
    Product { title: "Ant Esports MK1400 Pro", price: 1999, rating: 4.0, reviews: 5210 },
];

#[derive(Debug, Clone, Default)]
struct MockPage {
    url: String,
    title: String,
    page_type: Option<String>,
    result_count: Option<u64>,
    history: Vec<String>,
    search_query: Option<String>,
    filtered: bool,
    extracted_candidates: usize,
}

pub struct MockBrowserRuntime {
    state: Mutex<MockPage>,
    /// Multiplier for simulated latencies (tests set this to 0).
    delay_ms: u64,
}

impl Default for MockBrowserRuntime {
    fn default() -> Self {
        Self::new(1)
    }
}

impl MockBrowserRuntime {
    /// `delay_scale`: 1 for realistic demo pacing, 0 for instant (tests).
    pub fn new(delay_scale: u64) -> Self {
        Self {
            state: Mutex::new(MockPage {
                url: "about:blank".into(),
                title: "New Tab".into(),
                ..Default::default()
            }),
            delay_ms: delay_scale,
        }
    }

    async fn simulate_latency(
        &self,
        base_ms: u64,
        signal: &CancellationToken,
    ) -> Result<(), BrowserError> {
        let ms = base_ms * self.delay_ms;
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(ms)) => Ok(()),
            _ = signal.cancelled() => Err(BrowserError::Cancelled),
        }
    }

    fn snapshot(&self) -> PageState {
        let s = self.state.lock().unwrap();
        let mut elements = vec![InteractiveElement {
            role: Some("textbox".into()),
            name: Some("Search".into()),
            text: None,
            selector: Some("#twotabsearchtextbox".into()),
        }];
        if s.page_type.as_deref() == Some("search_results") {
            elements.push(InteractiveElement {
                role: Some("link".into()),
                name: Some("Under ₹5,000".into()),
                text: Some("Under ₹5,000".into()),
                selector: Some("#p_36 a".into()),
            });
        }
        PageState {
            url: s.url.clone(),
            title: s.title.clone(),
            page_type: s.page_type.clone(),
            accessibility_summary: Some(match s.page_type.as_deref() {
                Some("search_results") => {
                    "search results: product cards, price filters, rating badges".into()
                }
                Some("product_detail") => "product page: title, price, buy box, reviews".into(),
                _ => "landing page: search box, nav links, category tiles".into(),
            }),
            interactive_elements: elements,
            result_count: s.result_count,
            loading: false,
        }
    }
}

fn ok_result(
    action: NeuralNavAction,
    checks: Vec<VerificationCheck>,
    page_state: PageState,
    started: Instant,
    extracted: Option<serde_json::Value>,
) -> ActionResult {
    let verification = VerificationResult::from_checks(checks);
    ActionResult {
        ok: verification.passed,
        error_class: if verification.passed {
            None
        } else {
            Some(FailureClass::ActionVerificationFailed)
        },
        action,
        verification,
        page_state: Some(page_state),
        duration_ms: started.elapsed().as_millis() as u64,
        extracted,
    }
}

#[async_trait]
impl BrowserRuntime for MockBrowserRuntime {
    async fn execute(
        &self,
        action: NeuralNavAction,
        signal: CancellationToken,
    ) -> Result<ActionResult, BrowserError> {
        let started = Instant::now();
        use NeuralNavAction as A;

        let result = match &action {
            A::Navigate { url } => {
                self.simulate_latency(650, &signal).await?;
                let old_url;
                {
                    let mut s = self.state.lock().unwrap();
                    old_url = s.url.clone();
                    let prev = s.url.clone();
                    s.history.push(prev);
                    s.url = url.clone();
                    s.filtered = false;
                    s.search_query = None;
                    s.result_count = None;
                    s.extracted_candidates = 0;
                    if url.contains("amazon") {
                        s.title = "Amazon.in — Online Shopping".into();
                        s.page_type = Some("landing".into());
                    } else {
                        s.title = url.replace("https://", "").replace("http://", "");
                        s.page_type = Some("landing".into());
                    }
                }
                let arrived = self.state.lock().unwrap().url.clone();
                let checks = vec![
                    verifier::url_arrived(&old_url, &arrived, url),
                    VerificationCheck::pass("network idle", "212 ms"),
                    VerificationCheck::pass("DOM ready", "search box visible"),
                ];
                ok_result(action.clone(), checks, self.snapshot(), started, None)
            }

            A::Type { text, .. } => {
                self.simulate_latency(520, &signal).await?;
                {
                    let mut s = self.state.lock().unwrap();
                    s.search_query = Some(text.clone());
                    s.url = format!(
                        "https://www.amazon.in/s?k={}",
                        text.replace(' ', "+").to_lowercase()
                    );
                    s.title = format!("{text} — Amazon.in");
                    s.page_type = Some("search_results".into());
                    s.result_count = Some(UNFILTERED_COUNT);
                    s.filtered = false;
                }
                let checks = vec![
                    VerificationCheck::pass("search box focused", "role=textbox name=Search"),
                    VerificationCheck::pass(
                        "search results container visible",
                        format!("{UNFILTERED_COUNT} results for \"{text}\""),
                    ),
                    VerificationCheck::pass("page title matches query", text.clone()),
                ];
                ok_result(action.clone(), checks, self.snapshot(), started, None)
            }

            A::Click { name, text, .. } => {
                self.simulate_latency(480, &signal).await?;
                let label = name
                    .clone()
                    .or_else(|| text.clone())
                    .unwrap_or_default()
                    .to_lowercase();

                if label.contains("under") || label.contains("5,000") || label.contains("filter") {
                    // price-filter click
                    let (before, after);
                    {
                        let mut s = self.state.lock().unwrap();
                        before = s.result_count.unwrap_or(UNFILTERED_COUNT);
                        s.filtered = true;
                        s.result_count = Some(FILTERED_COUNT);
                        after = FILTERED_COUNT;
                        s.url = format!("{}&price=-5000", s.url);
                    }
                    let checks = vec![
                        verifier::result_count_reduced(before, after),
                        VerificationCheck::pass("filter chip visible", "Under ₹5,000 ✕"),
                    ];
                    ok_result(action.clone(), checks, self.snapshot(), started, None)
                } else if label.contains("best rated") || label.contains("candidate") {
                    // choose-best click → product page
                    let ready = self.state.lock().unwrap().extracted_candidates >= 3;
                    if !ready {
                        return Err(BrowserError::Action {
                            class: FailureClass::ElementNotFound,
                            message: "no ranked candidates on page yet".into(),
                        });
                    }
                    let best = &CATALOG[0];
                    {
                        let mut s = self.state.lock().unwrap();
                        let prev = s.url.clone();
                        s.history.push(prev);
                        s.url = "https://www.amazon.in/dp/B08MOCK26".into();
                        s.title = format!("{} — Amazon.in", best.title);
                        s.page_type = Some("product_detail".into());
                        s.result_count = None;
                    }
                    let checks = vec![
                        VerificationCheck::pass("product detail page opened", best.title),
                        VerificationCheck::pass(
                            "price within constraint",
                            format!("₹{} < ₹5,000", best.price),
                        ),
                        VerificationCheck::pass(
                            "rating verified",
                            format!("{}★ · {} reviews", best.rating, best.reviews),
                        ),
                    ];
                    ok_result(action.clone(), checks, self.snapshot(), started, None)
                } else {
                    // Unknown target — deterministic ELEMENT_NOT_FOUND so the
                    // recovery ladder has something real to exercise.
                    return Err(BrowserError::Action {
                        class: FailureClass::ElementNotFound,
                        message: format!("no element matching '{label}'"),
                    });
                }
            }

            A::Extract { fields } => {
                self.simulate_latency(560, &signal).await?;
                let filtered = self.state.lock().unwrap().filtered;
                let candidates: Vec<_> = CATALOG
                    .iter()
                    .filter(|p| !filtered || p.price < 5000)
                    .map(|p| {
                        json!({
                            "title": p.title,
                            "price": format!("₹{}", p.price),
                            "rating": p.rating,
                            "reviews": p.reviews,
                        })
                    })
                    .collect();
                let count = candidates.len();
                self.state.lock().unwrap().extracted_candidates = count;
                let checks = vec![
                    VerificationCheck::pass(
                        "product cards extracted",
                        format!("{count} candidates · fields: {}", fields.join(", ")),
                    ),
                    verifier::min_candidates(count, 3),
                    VerificationCheck::pass("schema valid", "all rows parsed"),
                ];
                ok_result(
                    action.clone(),
                    checks,
                    self.snapshot(),
                    started,
                    Some(json!({ "candidates": candidates })),
                )
            }

            A::Scroll { direction, amount } => {
                self.simulate_latency(160, &signal).await?;
                let dir = match direction {
                    ScrollDirection::Up => "up",
                    ScrollDirection::Down => "down",
                };
                let checks = vec![VerificationCheck::pass(
                    "viewport scrolled",
                    format!("{dir} {}px", amount.unwrap_or(600)),
                )];
                ok_result(action.clone(), checks, self.snapshot(), started, None)
            }

            A::Wait { ms } => {
                self.simulate_latency((*ms).min(2000), &signal).await?;
                let checks = vec![VerificationCheck::pass("waited", format!("{ms} ms"))];
                ok_result(action.clone(), checks, self.snapshot(), started, None)
            }

            A::GoBack => {
                self.simulate_latency(300, &signal).await?;
                let popped;
                {
                    let mut s = self.state.lock().unwrap();
                    popped = s.history.pop();
                    if let Some(prev) = &popped {
                        s.url = prev.clone();
                        s.page_type = Some("landing".into());
                    }
                }
                let checks = vec![match popped {
                    Some(u) => VerificationCheck::pass("navigated back", u),
                    None => VerificationCheck::fail("navigated back", "history empty"),
                }];
                ok_result(action.clone(), checks, self.snapshot(), started, None)
            }

            A::Reload => {
                self.simulate_latency(420, &signal).await?;
                let checks = vec![
                    VerificationCheck::pass("page reloaded", self.snapshot().url),
                    VerificationCheck::pass("network idle", "168 ms"),
                ];
                ok_result(action.clone(), checks, self.snapshot(), started, None)
            }

            // Voice / interaction actions are no-ops at the browser layer; the
            // orchestrator handles them. Executing them here still verifies.
            A::Speak { message } => {
                let checks =
                    vec![VerificationCheck::pass("utterance queued", message.clone())];
                ok_result(action.clone(), checks, self.snapshot(), started, None)
            }
            A::AskUser { question, .. } => {
                let checks =
                    vec![VerificationCheck::pass("clarification surfaced", question.clone())];
                ok_result(action.clone(), checks, self.snapshot(), started, None)
            }
            A::ConfirmSensitiveAction { description } => {
                let checks = vec![VerificationCheck::pass(
                    "confirmation gate armed",
                    description.clone(),
                )];
                ok_result(action.clone(), checks, self.snapshot(), started, None)
            }
        };

        Ok(result)
    }

    async fn page_state(&self) -> Result<PageState, BrowserError> {
        Ok(self.snapshot())
    }

    async fn stop(&self) -> Result<(), BrowserError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuralnav_core::NeuralNavAction as A;

    fn rt() -> MockBrowserRuntime {
        MockBrowserRuntime::new(0) // instant delays in tests
    }

    #[tokio::test]
    async fn demo_flow_completes_end_to_end() {
        let rt = rt();
        let tok = CancellationToken::new();

        let r = rt
            .execute(A::Navigate { url: "https://www.amazon.in".into() }, tok.clone())
            .await
            .unwrap();
        assert!(r.ok && r.verification.passed);

        let r = rt
            .execute(
                A::Type {
                    selector: None,
                    role: Some("textbox".into()),
                    name: Some("Search".into()),
                    text: "mechanical keyboard".into(),
                },
                tok.clone(),
            )
            .await
            .unwrap();
        assert!(r.ok);
        assert_eq!(r.page_state.as_ref().unwrap().result_count, Some(UNFILTERED_COUNT));

        let r = rt
            .execute(
                A::Click {
                    selector: None,
                    role: Some("link".into()),
                    name: Some("Under ₹5,000".into()),
                    text: None,
                },
                tok.clone(),
            )
            .await
            .unwrap();
        assert!(r.ok);
        assert_eq!(r.page_state.as_ref().unwrap().result_count, Some(FILTERED_COUNT));

        let r = rt
            .execute(
                A::Extract {
                    fields: vec!["title".into(), "price".into(), "rating".into(), "reviews".into()],
                },
                tok.clone(),
            )
            .await
            .unwrap();
        assert!(r.ok);
        let extracted = r.extracted.unwrap();
        assert!(extracted["candidates"].as_array().unwrap().len() >= 3);

        let r = rt
            .execute(
                A::Click {
                    selector: None,
                    role: Some("link".into()),
                    name: Some("Best rated candidate".into()),
                    text: None,
                },
                tok,
            )
            .await
            .unwrap();
        assert!(r.ok);
        assert_eq!(
            r.page_state.unwrap().page_type.as_deref(),
            Some("product_detail")
        );
    }

    #[tokio::test]
    async fn navigation_is_replayable() {
        // Second demo run starts from the previous run's final page — the
        // navigate node must still verify (regression: strict "URL changed").
        let rt = rt();
        let tok = CancellationToken::new();
        for _ in 0..2 {
            let r = rt
                .execute(A::Navigate { url: "https://www.amazon.in".into() }, tok.clone())
                .await
                .unwrap();
            assert!(r.ok, "navigate must verify on replay: {:?}", r.verification);
        }
    }

    #[tokio::test]
    async fn unknown_click_target_yields_element_not_found() {
        let rt = rt();
        let err = rt
            .execute(
                A::Click {
                    selector: None,
                    role: Some("button".into()),
                    name: Some("Nonexistent Widget".into()),
                    text: None,
                },
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(err.class(), FailureClass::ElementNotFound);
    }

    #[tokio::test]
    async fn cancellation_interrupts_execution() {
        let rt = MockBrowserRuntime::new(1); // real delays so we can cancel mid-flight
        let tok = CancellationToken::new();
        let t2 = tok.clone();
        let handle = tokio::spawn(async move {
            rt.execute(A::Navigate { url: "https://www.amazon.in".into() }, t2).await
        });
        tokio::time::sleep(Duration::from_millis(30)).await;
        tok.cancel();
        assert!(matches!(handle.await.unwrap(), Err(BrowserError::Cancelled)));
    }

    #[tokio::test]
    async fn choose_before_extract_fails_deterministically() {
        let rt = rt();
        let err = rt
            .execute(
                A::Click {
                    selector: None,
                    role: Some("link".into()),
                    name: Some("Best rated candidate".into()),
                    text: None,
                },
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(err.class(), FailureClass::ElementNotFound);
    }
}
