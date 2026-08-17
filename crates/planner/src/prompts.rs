//! Prompt construction for the Gemini planner. The prompt demands strict
//! JSON conforming to the TaskGraph schema; the response is validated by
//! `neuralnav_core::schema::parse_task_graph` and rejected otherwise.

use neuralnav_core::{PermissionLevel, PlannerInput};

pub const SYSTEM_PROMPT: &str = r#"You are NeuralNav, a browser automation planner.

Return ONLY valid JSON (no prose, no markdown fences) matching this schema:

{
  "goal": string,
  "nodes": [
    {
      "id": string,                // unique snake_case id
      "title": string,             // 1-3 word human label
      "action": Action,            // see vocabulary below
      "depends_on": [string],      // ids of prerequisite nodes
      "success_check": string      // observable postcondition
    }
  ],
  "metadata": { "tts": { "<node_id>": string } }   // short spoken feedback per node
}

Action vocabulary (tagged on "type", snake_case):
  {"type":"navigate","url":string}
  {"type":"click","selector"?:string,"role"?:string,"name"?:string,"text"?:string}
  {"type":"type","selector"?:string,"role"?:string,"name"?:string,"text":string}
  {"type":"scroll","direction":"up"|"down","amount"?:number}
  {"type":"wait","ms":number}
  {"type":"extract","fields":[string]}
  {"type":"go_back"}
  {"type":"reload"}
  {"type":"speak","message":string}
  {"type":"ask_user","question":string,"options"?:[string]}
  {"type":"confirm_sensitive_action","description":string}

Rules:
- Keep actions atomic; one observable effect per node.
- Prefer accessible role+name over brittle CSS selectors.
- Every node MUST have a non-empty success_check.
- If the command is ambiguous, return a single ask_user node.
- Any payment, checkout, message-send, download or irreversible action MUST
  be a confirm_sensitive_action node before the acting node.
- Never emit JavaScript, shell commands, or any action outside the vocabulary."#;

pub fn build_user_prompt(input: &PlannerInput) -> String {
    let level = match input.permission_level {
        PermissionLevel::ReadOnly => "read_only (no typing/clicking allowed)",
        PermissionLevel::Interactive => "interactive (sensitive actions need confirmation)",
        PermissionLevel::Restricted => "restricted (every sensitive action needs confirmation)",
    };
    let page = input
        .page_state
        .as_ref()
        .map(|p| {
            format!(
                "current page: {} — {} (type: {}, {} interactive elements)",
                p.title,
                p.url,
                p.page_type.as_deref().unwrap_or("unknown"),
                p.interactive_elements.len()
            )
        })
        .unwrap_or_else(|| "current page: none (fresh session)".into());
    let prior = if input.prior_actions.is_empty() {
        "prior actions: none".to_string()
    } else {
        format!(
            "prior actions: {}",
            input
                .prior_actions
                .iter()
                .map(|a| a.kind())
                .collect::<Vec<_>>()
                .join(" → ")
        )
    };
    format!(
        "user command: {t}\n{page}\n{prior}\npermission level: {level}\n\nProduce the task graph JSON now.",
        t = input.transcript
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_prompt_includes_command_and_level() {
        let p = build_user_prompt(&PlannerInput {
            transcript: "open amazon".into(),
            ..Default::default()
        });
        assert!(p.contains("open amazon"));
        assert!(p.contains("interactive"));
    }
}
