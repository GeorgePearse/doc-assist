use litellm_rs::{completion, user_message, system_message, CompletionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check for API key
    let api_key = std::env::var("OPENAI_API_KEY");
    if let Err(_) = api_key {
        eprintln!("❌ OPENAI_API_KEY not found");
        eprintln!("Please set: export OPENAI_API_KEY=your-api-key");
        std::process::exit(1);
    }

    println!("✓ Found OPENAI_API_KEY");

    // Create messages
    let messages = vec![
        system_message("You are a helpful assistant."),
        user_message("Say 'Hello from Rust litellm-rs!' in one sentence."),
    ];

    // Create request options
    let options = CompletionOptions {
        temperature: Some(0.3),
        max_tokens: Some(50),
        ..Default::default()
    };

    // Try different model names
    let models = vec![
        "gpt-3.5-turbo",
        "openai/gpt-3.5-turbo",
        "gpt-4",
    ];

    println!("\nTesting model configurations:");
    println!("{}", "=".repeat(50));

    for model_name in models {
        print!("Testing model '{}': ", model_name);

        match completion(model_name, messages.clone(), Some(options.clone())).await {
            Ok(response) => {
                // Extract response text
                let response_text = response.choices
                    .first()
                    .and_then(|choice| choice.message.content.as_ref())
                    .and_then(|content| {
                        match content {
                            litellm_rs::MessageContent::Text(text) => Some(text.clone()),
                            _ => None,
                        }
                    })
                    .unwrap_or_else(|| String::from("[Empty response]"));

                println!("✓ SUCCESS");
                println!("  Response: {}", response_text);
                println!("  This model name works!");
                break;
            }
            Err(e) => {
                println!("✗ FAILED: {}", e);
            }
        }
    }

    println!("\n{}", "=".repeat(50));
    println!("Test complete!");

    Ok(())
}