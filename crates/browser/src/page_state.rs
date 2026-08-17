//! Page-state summarization: turn a raw page snapshot into the compact,
//! accessibility-tree-first summary the planner reasons over (tiered:
//! a11y tree → simplified DOM → vision fallback happens in the worker).

use neuralnav_core::{InteractiveElement, PageState};

/// Compact single-line summary for prompts and logs.
pub fn summarize(state: &PageState) -> String {
    let kind = state.page_type.as_deref().unwrap_or("unknown");
    let elems = state
        .interactive_elements
        .iter()
        .take(8)
        .map(describe_element)
        .collect::<Vec<_>>()
        .join("; ");
    let count = state
        .result_count
        .map(|c| format!(" · {c} results"))
        .unwrap_or_default();
    format!("[{kind}] {} — {}{count} | interactive: {elems}", state.title, state.url)
}

fn describe_element(e: &InteractiveElement) -> String {
    match (&e.role, &e.name) {
        (Some(r), Some(n)) => format!("{r} \"{n}\""),
        (Some(r), None) => r.clone(),
        (None, Some(n)) => format!("\"{n}\""),
        _ => e.text.clone().unwrap_or_else(|| "element".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_is_compact_and_role_first() {
        let s = PageState {
            url: "https://www.amazon.in/s?k=keyboard".into(),
            title: "keyboard — Amazon.in".into(),
            page_type: Some("search_results".into()),
            accessibility_summary: None,
            interactive_elements: vec![InteractiveElement {
                role: Some("textbox".into()),
                name: Some("Search".into()),
                text: None,
                selector: None,
            }],
            result_count: Some(1204),
            loading: false,
        };
        let out = summarize(&s);
        assert!(out.contains("[search_results]"));
        assert!(out.contains("textbox \"Search\""));
        assert!(out.contains("1204 results"));
    }
}
