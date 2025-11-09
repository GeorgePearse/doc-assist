use crate::context::ContextManager;
use crate::error::{DocAssistError, Result};
use crate::planner::{Query, QueryPlan, Phase};
use crate::state::GenerationState;
use litellm_rs::{
    completion,
    user_message,
    system_message,
    CompletionOptions,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub query_id: usize,
    pub prompt: String,
    pub response: String,
    pub tokens_used: usize,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResult {
    pub phase_results: HashMap<String, Vec<QueryResult>>,
    pub total_tokens: usize,
    pub total_cost_usd: f64,
    pub total_duration_ms: u64,
    pub errors: Vec<String>,
    pub completed_queries: usize,
}

pub struct Generator {
    model: String,
    api_key: String,
    max_concurrent_requests: usize,
    rate_limit_per_minute: usize,
    retry_attempts: usize,
    context_manager: Arc<Mutex<ContextManager>>,
    state_manager: Arc<Mutex<GenerationState>>,
}

impl Generator {
    pub fn new(
        api_key: String,
        model: String,
        max_concurrent_requests: usize,
        rate_limit_per_minute: usize,
    ) -> Result<Self> {
        // Set the appropriate environment variable based on the model
        if model.contains("claude") {
            std::env::set_var("ANTHROPIC_API_KEY", api_key.clone());
        } else if model.contains("gpt") {
            std::env::set_var("OPENAI_API_KEY", api_key.clone());
        } else {
            return Err(DocAssistError::ConfigError(
                format!("Unsupported model: {}. Use 'claude' or 'gpt' models", model)
            ));
        }

        let context_manager = Arc::new(Mutex::new(ContextManager::new(128_000))); // 128k token context
        let state_manager = Arc::new(Mutex::new(GenerationState::new()));

        Ok(Self {
            model,
            api_key,
            max_concurrent_requests,
            rate_limit_per_minute,
            retry_attempts: 3,
            context_manager,
            state_manager,
        })
    }

    pub async fn execute_plan(&self, plan: &QueryPlan) -> Result<GenerationResult> {
        info!(
            "Starting documentation generation with {} queries across {} phases",
            plan.total_queries,
            plan.phases.len()
        );

        // Test LLM connection before starting the full generation
        self.test_llm_connection().await?;

        let multi_progress = MultiProgress::new();
        let overall_progress = multi_progress.add(ProgressBar::new(plan.total_queries as u64));
        overall_progress.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} queries ({percent}%) {msg}")
                .unwrap()
                .progress_chars("##-"),
        );
        overall_progress.set_message("Generating documentation...");

        let mut result = GenerationResult {
            phase_results: HashMap::new(),
            total_tokens: 0,
            total_cost_usd: 0.0,
            total_duration_ms: 0,
            errors: vec![],
            completed_queries: 0,
        };

        // Semaphore for rate limiting
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent_requests));
        let rate_limiter = Arc::new(Mutex::new(RateLimiter::new(self.rate_limit_per_minute)));

        // Track consecutive failures for fail-fast behavior
        let mut consecutive_failures = 0;
        const MAX_CONSECUTIVE_FAILURES: usize = 5;

        for (phase_idx, phase) in plan.phases.iter().enumerate() {
            info!("Executing phase {}/{}: {}", phase_idx + 1, plan.phases.len(), phase.name);

            let phase_progress = multi_progress.add(ProgressBar::new(phase.queries.len() as u64));
            phase_progress.set_style(
                ProgressStyle::default_bar()
                    .template("  └─ {msg} [{bar:30.green/white}] {pos}/{len}")
                    .unwrap()
                    .progress_chars("##-"),
            );
            phase_progress.set_message(format!("Phase: {}", phase.name));

            let phase_results = self.execute_phase(
                phase,
                &semaphore,
                &rate_limiter,
                &overall_progress,
                &phase_progress,
            ).await?;

            // Update results and check for failures
            let mut phase_failures = 0;
            for query_result in &phase_results {
                result.total_tokens += query_result.tokens_used;
                result.total_duration_ms += query_result.duration_ms;
                if query_result.error.is_none() && !query_result.response.is_empty() {
                    result.completed_queries += 1;
                    consecutive_failures = 0; // Reset on success
                } else {
                    phase_failures += 1;
                    consecutive_failures += 1;

                    let error_msg = if let Some(ref error) = query_result.error {
                        format!("Query {}: {}", query_result.query_id, error)
                    } else {
                        format!("Query {}: Empty response received", query_result.query_id)
                    };
                    result.errors.push(error_msg.clone());
                    warn!("{}", error_msg);
                }
            }

            // Check if we should fail fast
            if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                overall_progress.abandon_with_message("Documentation generation failed - too many consecutive errors!");
                return Err(DocAssistError::GenerationError(
                    format!("Aborting: {} consecutive query failures. Check your LLM configuration.", consecutive_failures)
                ));
            }

            // Fail if entire phase failed
            if phase_failures == phase.queries.len() {
                overall_progress.abandon_with_message("Documentation generation failed - entire phase failed!");
                return Err(DocAssistError::GenerationError(
                    format!("All {} queries in phase '{}' failed. Check your model configuration and API keys.",
                           phase.queries.len(), phase.name)
                ));
            }

            result.phase_results.insert(phase.name.clone(), phase_results);
            phase_progress.finish_with_message(format!("✓ {} ({}/{} successful)",
                phase.name,
                phase.queries.len() - phase_failures,
                phase.queries.len()
            ));
        }

        // Final validation - ensure we have at least some successful queries
        if result.completed_queries == 0 {
            overall_progress.abandon_with_message("Documentation generation failed - no successful queries!");
            return Err(DocAssistError::GenerationError(
                "No queries completed successfully. Check your LLM provider configuration and API keys.".to_string()
            ));
        }

        // Warn if more than 50% of queries failed
        let failure_rate = (result.errors.len() as f64) / (plan.total_queries as f64);
        if failure_rate > 0.5 {
            warn!("High failure rate: {:.1}% of queries failed", failure_rate * 100.0);
            overall_progress.finish_with_message(format!("⚠ Generation completed with warnings ({}/{} successful)",
                result.completed_queries, plan.total_queries));
        } else {
            overall_progress.finish_with_message("Documentation generation complete!");
        }

        // Calculate cost
        result.total_cost_usd = self.calculate_cost(result.total_tokens);

        info!(
            "Generation complete: {}/{} queries successful, {} tokens, ${:.2}, {:.1}s",
            result.completed_queries,
            plan.total_queries,
            result.total_tokens,
            result.total_cost_usd,
            result.total_duration_ms as f64 / 1000.0
        );

        Ok(result)
    }

    async fn execute_phase(
        &self,
        phase: &Phase,
        semaphore: &Arc<Semaphore>,
        rate_limiter: &Arc<Mutex<RateLimiter>>,
        overall_progress: &ProgressBar,
        phase_progress: &ProgressBar,
    ) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();
        let mut tasks = vec![];

        for query in &phase.queries {
            let semaphore = semaphore.clone();
            let rate_limiter = rate_limiter.clone();
            let model = self.model.clone();
            let _api_key = self.api_key.clone();  // Used for setting env var in spawned task
            let context_manager = self.context_manager.clone();
            let state_manager = self.state_manager.clone();
            let query = query.clone();
            let retry_attempts = self.retry_attempts;

            let task = tokio::spawn(async move {
                // Acquire semaphore permit
                let _permit = semaphore.acquire().await.unwrap();

                // Rate limiting
                rate_limiter.lock().await.wait_if_needed().await;

                // Build context for this query
                let context = context_manager.lock().await.get_context_for_query(&query).await;

                // Execute query with retries
                let mut attempts = 0;
                let mut last_error = None;

                while attempts < retry_attempts {
                    match execute_single_query(&model, &query, &context).await {
                        Ok(result) => {
                            // Update state and context
                            state_manager.lock().await.mark_query_completed(query.id);
                            context_manager.lock().await.add_response(query.id, &result.response);

                            return result;
                        }
                        Err(e) => {
                            attempts += 1;
                            last_error = Some(e.to_string());
                            warn!("Query {} failed (attempt {}/{}): {}", query.id, attempts, retry_attempts, e);

                            if attempts < retry_attempts {
                                sleep(Duration::from_secs(2u64.pow(attempts as u32))).await; // Exponential backoff
                            }
                        }
                    }
                }

                // All retries failed
                QueryResult {
                    query_id: query.id,
                    prompt: query.prompt.clone(),
                    response: String::new(),
                    tokens_used: 0,
                    duration_ms: 0,
                    error: last_error,
                }
            });

            tasks.push(task);
        }

        // Wait for all tasks to complete
        for (i, task) in tasks.into_iter().enumerate() {
            let result = task.await.map_err(|e| DocAssistError::GenerationError(e.to_string()))?;
            results.push(result);
            overall_progress.inc(1);
            phase_progress.inc(1);

            // Save state periodically
            if i % 5 == 0 {
                self.state_manager.lock().await.save_to_disk().await?;
            }
        }

        Ok(results)
    }

    async fn test_llm_connection(&self) -> Result<()> {
        info!("Testing LLM connection with model: {}", self.model);

        let test_query = Query {
            id: 0,
            prompt: "Respond with 'OK' to confirm the connection works.".to_string(),
            description: "Connection test".to_string(),
            context_spec: crate::planner::ContextSpec::None,
            priority: crate::planner::QueryPriority::Critical,
            estimated_tokens: 50,
            target: crate::planner::QueryTarget::Overview,
        };

        match execute_single_query(&self.model, &test_query, "").await {
            Ok(result) => {
                if result.response.is_empty() {
                    return Err(DocAssistError::GenerationError(
                        format!("LLM connection test failed: Empty response from model '{}'", self.model)
                    ));
                }
                info!("✓ LLM connection test successful");
                Ok(())
            }
            Err(e) => {
                return Err(DocAssistError::GenerationError(
                    format!("LLM connection test failed: {}. Please check:\n  1. Your API key is set correctly (ANTHROPIC_API_KEY or OPENAI_API_KEY)\n  2. The model '{}' is valid and accessible\n  3. Your API key has sufficient credits", e, self.model)
                ));
            }
        }
    }

    fn calculate_cost(&self, total_tokens: usize) -> f64 {
        // Pricing per million tokens (approximate)
        let price_per_million = if self.model.contains("claude") {
            10.0 // Claude average pricing
        } else if self.model.contains("gpt-4") {
            20.0 // GPT-4 average pricing
        } else {
            1.0 // Cheaper model fallback
        };

        (total_tokens as f64 / 1_000_000.0) * price_per_million
    }

    pub async fn resume_from_state(&self, state: GenerationState, plan: &mut QueryPlan) -> Result<GenerationResult> {
        info!("Resuming generation from saved state with {} completed queries", state.completed_queries.len());

        // Skip already completed queries
        plan.skip_completed(&state.completed_queries);

        // Load previous context
        self.context_manager.lock().await.restore_from_state(&state);

        // Continue execution
        self.execute_plan(plan).await
    }
}

async fn execute_single_query(
    model: &str,
    query: &Query,
    context: &str,
) -> Result<QueryResult> {
    let start_time = std::time::Instant::now();

    let full_prompt = format!(
        "{}\n\nContext from previous documentation:\n{}\n\n{}",
        query.prompt,
        context,
        "Please provide a comprehensive and detailed response suitable for technical documentation."
    );

    // Create chat messages for LiteLLM
    let messages = vec![
        system_message("You are a technical documentation expert. Provide clear, accurate, and comprehensive documentation based on the codebase analysis. Use markdown formatting for better readability."),
        user_message(&full_prompt),
    ];

    // Create request options
    let options = CompletionOptions {
        temperature: Some(0.3), // Lower temperature for more consistent documentation
        max_tokens: Some(4000),
        ..Default::default()
    };

    // Send request using LiteLLM
    debug!("Sending request for query {} with model {}", query.id, model);

    let response = completion(model, messages, Some(options)).await
        .map_err(|e| DocAssistError::GenerationError(format!("LLM request failed: {}", e)))?;

    let duration_ms = start_time.elapsed().as_millis() as u64;

    // Extract response text - MessageContent is an enum, need to extract the string
    let response_text = response.choices
        .first()
        .and_then(|choice| choice.message.content.as_ref())
        .and_then(|content| {
            // Extract text from MessageContent enum
            match content {
                litellm_rs::MessageContent::Text(text) => Some(text.clone()),
                _ => None,
            }
        })
        .unwrap_or_else(|| String::new());

    // Debug log the response
    debug!("Query {}: Response length = {}", query.id, response_text.len());
    if response_text.is_empty() {
        warn!("Query {} returned empty response", query.id);
    }

    // Calculate tokens from usage or estimate
    let tokens_used = response.usage
        .map(|usage| usage.total_tokens as usize)
        .unwrap_or_else(|| (full_prompt.len() + response_text.len()) / 4);

    Ok(QueryResult {
        query_id: query.id,
        prompt: query.prompt.clone(),
        response: response_text,
        tokens_used,
        duration_ms,
        error: None,
    })
}

struct RateLimiter {
    requests_per_minute: usize,
    request_times: Vec<std::time::Instant>,
}

impl RateLimiter {
    fn new(requests_per_minute: usize) -> Self {
        Self {
            requests_per_minute,
            request_times: Vec::new(),
        }
    }

    async fn wait_if_needed(&mut self) {
        let now = std::time::Instant::now();
        let minute_ago = now - Duration::from_secs(60);

        // Remove old requests
        self.request_times.retain(|&time| time > minute_ago);

        // Check if we need to wait
        if self.request_times.len() >= self.requests_per_minute {
            let oldest = self.request_times[0];
            let wait_time = Duration::from_secs(60) - (now - oldest);
            if wait_time > Duration::from_secs(0) {
                debug!("Rate limiting: waiting for {:?}", wait_time);
                sleep(wait_time).await;
            }
        }

        self.request_times.push(now);
    }
}

/// Execute queries from a plan
pub async fn execute_queries(
    plan: &QueryPlan,
    config: &crate::config::Config,
    existing_state: Option<&GenerationState>,
) -> Result<GenerationResult> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .map_err(|_| DocAssistError::ConfigError("No API key found".to_string()))?;

    let generator = Generator::new(
        api_key,
        config.model.clone(),
        config.rate_limit as usize / 6, // Max concurrent requests (rate_limit per minute / 6 = per 10 seconds)
        config.rate_limit as usize,
    )?;

    if let Some(state) = existing_state {
        let mut mutable_plan = plan.clone();
        generator.resume_from_state(state.clone(), &mut mutable_plan).await
    } else {
        generator.execute_plan(plan).await
    }
}