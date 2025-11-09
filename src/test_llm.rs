#[cfg(test)]
mod tests {
    use litellm_rs::{completion, user_message, system_message, CompletionOptions};

    #[tokio::test]
    async fn test_llm_connection() {
        // API key is read from environment by litellm-rs
        std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

        // Create messages
        let messages = vec![
            system_message("You are a helpful assistant."),
            user_message("Say 'Hello, Rust!' in one sentence."),
        ];

        // Create request options
        let options = CompletionOptions {
            temperature: Some(0.7),
            max_tokens: Some(50),
            ..Default::default()
        };

        // Send request
        println!("Sending request to OpenAI...");
        let response = completion("gpt-3.5-turbo", messages, Some(options))
            .await
            .expect("Request failed");

        // Print full response for debugging
        println!("Full response: {:?}", response);

        // Try to get content
        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_ref())
            .expect("No content found in response");

        println!("\nResponse content: {}", content);
        assert!(!content.is_empty(), "Response content should not be empty");
    }
}