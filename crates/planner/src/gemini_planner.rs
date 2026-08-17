//! GeminiPlanner — real LLM planning behind `USE_REAL_PLANNER=true`.
//!
//! Calls the Gemini generateContent API with a strict-JSON prompt, validates
//! the response through `neuralnav_core::schema::parse_task_graph`, retries
//! once with the parse error appended, and falls back to [`MockPlanner`] if
//! the output is still invalid or no API key is configured.

use crate::mock_planner::MockPlanner;
use crate::planner::Planner;
use crate::prompts::{build_user_prompt, SYSTEM_PROMPT};
use async_trait::async_trait;
use neuralnav_core::{schema, PlannerError, PlannerInput, TaskGraph};
use serde_json::{json, Value};

pub struct GeminiPlanner {
    api_key: Option<String>,
    model: String,
    client: reqwest::Client,
    fallback: MockPlanner,
}

impl GeminiPlanner {
    pub fn new(api_key: Option<String>, model: Option<String>) -> Self {
        Self {
            api_key: api_key.filter(|k| !k.trim().is_empty()),
            model: model
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| "gemini-2.0-flash".to_string()),
            client: reqwest::Client::new(),
            fallback: MockPlanner,
        }
    }

    async fn call_gemini(&self, key: &str, prompt: &str) -> Result<String, PlannerError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model
        );
        let body = json!({
            "system_instruction": { "parts": [{ "text": SYSTEM_PROMPT }] },
            "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
            "generationConfig": {
                "temperature": 0.2,
                "response_mime_type": "application/json"
            }
        });
        let resp = self
            .client
            .post(&url)
            .header("x-goog-api-key", key)
            .json(&body)
            .send()
            .await
            .map_err(|e| PlannerError::Transport(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(PlannerError::Transport(format!("gemini http {}", resp.status())));
        }
        let v: Value = resp
            .json()
            .await
            .map_err(|e| PlannerError::Transport(e.to_string()))?;
        v["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| PlannerError::InvalidOutput("no text candidate in response".into()))
    }

    async fn plan_via_gemini(
        &self,
        key: &str,
        input: &PlannerInput,
    ) -> Result<TaskGraph, PlannerError> {
        let prompt = build_user_prompt(input);
        let raw = self.call_gemini(key, &prompt).await?;
        match schema::parse_task_graph(&raw) {
            Ok(g) => Ok(g),
            Err(first_err) => {
                tracing::warn!(error = %first_err, "gemini output invalid; retrying once");
                let retry_prompt = format!(
                    "{prompt}\n\nYour previous output failed validation: {first_err}\n\
                     Return corrected JSON only."
                );
                let raw2 = self.call_gemini(key, &retry_prompt).await?;
                schema::parse_task_graph(&raw2)
            }
        }
    }
}

#[async_trait]
impl Planner for GeminiPlanner {
    async fn plan(&self, input: PlannerInput) -> Result<TaskGraph, PlannerError> {
        let Some(key) = self.api_key.clone() else {
            tracing::info!("GEMINI_API_KEY missing — using mock planner fallback");
            return self.fallback.plan(input).await;
        };
        match self.plan_via_gemini(&key, &input).await {
            Ok(graph) => Ok(graph),
            Err(err) => {
                tracing::warn!(error = %err, "gemini planning failed — mock planner fallback");
                self.fallback.plan(input).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_key_falls_back_to_mock() {
        let planner = GeminiPlanner::new(None, None);
        let g = planner
            .plan(PlannerInput {
                transcript: "Open Amazon and find a mechanical keyboard under 5,000 rupees".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(g.nodes.len(), 5, "fell back to the mock demo graph");
    }
}
