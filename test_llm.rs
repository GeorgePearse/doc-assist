use llm_connector::{
    LlmClient,
    types::{ChatRequest, Message, Role},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key
    let api_key = std::env::var("OPENAI_API_KEY")?;

    // Create client
    let client = LlmClient::openai(&api_key)?;

    // Create messages
    let messages = vec![
        Message::text(Role::System, "You are a helpful assistant."),
        Message::text(Role::User, "Write a short poem about Rust programming."),
    ];

    // Build request
    let request = ChatRequest {
        model: "gpt-4o".to_string(),
        messages,
        max_tokens: Some(100),
        temperature: Some(0.7),
        ..Default::default()
    };

    // Send request
    println!("Sending request to OpenAI...");
    let response = client.chat(&request).await?;

    // Print full response for debugging
    println!("Full response: {:?}", response);

    // Try to get content
    if let Some(content) = response.get_content() {
        println!("\nResponse content: {}", content);
    } else {
        println!("\nNo content found in response");
    }

    Ok(())
}