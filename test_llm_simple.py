#!/usr/bin/env python3
"""
Simple test script to verify LLM connectivity using litellm (Python version).
This will help us understand the correct model names and configuration.
"""

import os
import sys

# Try to import litellm
try:
    import litellm
except ImportError:
    print("Error: litellm not installed. Install with: pip install litellm")
    sys.exit(1)

def test_openai():
    """Test OpenAI API connection."""
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        print("❌ OPENAI_API_KEY not found in environment")
        return False

    print(f"✓ OpenAI API key found (starts with: {api_key[:10]}...)")

    try:
        response = litellm.completion(
            model="gpt-3.5-turbo",
            messages=[{"role": "user", "content": "Say 'Hello from OpenAI' in 5 words or less"}],
            max_tokens=20,
            temperature=0.1
        )
        content = response.choices[0].message.content
        print(f"✓ OpenAI response: {content}")
        return True
    except Exception as e:
        print(f"❌ OpenAI error: {e}")
        return False

def test_gpt4o():
    """Test GPT-4o model specifically."""
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        print("❌ OPENAI_API_KEY not found for GPT-4o test")
        return False

    print("Testing GPT-4o model...")

    try:
        response = litellm.completion(
            model="gpt-4o",
            messages=[{"role": "user", "content": "Say 'Hello from GPT-4o' if you're the GPT-4o model"}],
            max_tokens=20,
            temperature=0.1
        )
        content = response.choices[0].message.content
        print(f"✓ GPT-4o response: {content}")
        return True
    except Exception as e:
        print(f"❌ GPT-4o error: {e}")
        # Try alternative model names
        for alt_model in ["gpt-4o-2024-08-06", "gpt-4o-mini"]:
            try:
                print(f"  Trying alternative model name: {alt_model}")
                response = litellm.completion(
                    model=alt_model,
                    messages=[{"role": "user", "content": f"Say 'Hello from {alt_model}'"}],
                    max_tokens=20,
                    temperature=0.1
                )
                content = response.choices[0].message.content
                print(f"✓ {alt_model} response: {content}")
                return True
            except Exception as e2:
                print(f"  ❌ {alt_model} error: {e2}")
        return False

def test_anthropic():
    """Test Anthropic API connection."""
    api_key = os.getenv("ANTHROPIC_API_KEY")
    if not api_key:
        print("❌ ANTHROPIC_API_KEY not found in environment")
        return False

    print(f"✓ Anthropic API key found (starts with: {api_key[:10]}...)")

    try:
        response = litellm.completion(
            model="claude-3-5-sonnet-20241022",
            messages=[{"role": "user", "content": "Say 'Hello from Claude' in 5 words or less"}],
            max_tokens=20,
            temperature=0.1
        )
        content = response.choices[0].message.content
        print(f"✓ Anthropic response: {content}")
        return True
    except Exception as e:
        print(f"❌ Anthropic error: {e}")
        # Try with different model name format
        try:
            print("  Trying alternative model name...")
            response = litellm.completion(
                model="claude-3-sonnet-20240229",
                messages=[{"role": "user", "content": "Say 'Hello from Claude' in 5 words or less"}],
                max_tokens=20,
                temperature=0.1
            )
            content = response.choices[0].message.content
            print(f"✓ Anthropic response (alt model): {content}")
            return True
        except Exception as e2:
            print(f"❌ Anthropic error (alt model): {e2}")
            return False

def main():
    print("=" * 50)
    print("LLM Connection Test")
    print("=" * 50)
    print()

    # Check environment variables
    print("Environment Check:")
    openai_key = os.getenv("OPENAI_API_KEY")
    anthropic_key = os.getenv("ANTHROPIC_API_KEY")

    if not openai_key and not anthropic_key:
        print("\n❌ No API keys found!")
        print("\nPlease set one of the following environment variables:")
        print("  export OPENAI_API_KEY=your-openai-key")
        print("  export ANTHROPIC_API_KEY=your-anthropic-key")
        sys.exit(1)

    print("\nTesting connections...")
    print("-" * 30)

    # Test available providers
    openai_works = False
    gpt4o_works = False
    anthropic_works = False

    if openai_key:
        openai_works = test_openai()
        print()
        gpt4o_works = test_gpt4o()
        print()

    if anthropic_key:
        anthropic_works = test_anthropic()
        print()

    # Summary
    print("=" * 50)
    print("Summary:")
    if openai_works:
        print("✓ OpenAI (GPT-3.5) connection successful")
        print("  Model: gpt-3.5-turbo")
    if gpt4o_works:
        print("✓ GPT-4o connection successful")
        print("  Models: gpt-4o, gpt-4o-mini, gpt-4o-2024-08-06")
    if anthropic_works:
        print("✓ Anthropic connection successful")
        print("  Model: claude-3-5-sonnet-20241022")

    if not openai_works and not gpt4o_works and not anthropic_works:
        print("❌ No working LLM connections found")
        sys.exit(1)
    else:
        print("\n✓ At least one LLM provider is working!")
        print("\nYou can now use docassist with the --model parameter:")
        if openai_works:
            print("  docassist --model gpt-3.5-turbo")
        if gpt4o_works:
            print("  docassist --model gpt-4o")
            print("  docassist --model gpt-4o-mini")
        if anthropic_works:
            print("  docassist --model claude-3-5-sonnet-20241022")

if __name__ == "__main__":
    main()