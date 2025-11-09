#!/usr/bin/env python3

import os
import subprocess
import time

def test_with_invalid_key():
    """Test that the tool fails fast with an invalid API key."""
    print("Testing with invalid API key...")

    # Set a fake API key
    env = os.environ.copy()
    env['OPENAI_API_KEY'] = 'sk-fake-test-key-12345'

    # Time the execution
    start_time = time.time()

    # Run the tool with a small test, redirect stderr to stdout to capture all output
    result = subprocess.run(
        ['./target/release/docassist', '--depth', 'quick', '--model', 'gpt-3.5-turbo', '.'],
        env=env,
        capture_output=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        timeout=30  # Should fail fast, not take 30 seconds
    )

    elapsed_time = time.time() - start_time
    print(f"Exit code: {result.returncode} (took {elapsed_time:.1f} seconds)")
    print("\nOutput (last 3000 chars):")
    print(result.stdout[-3000:])

    # Check if it failed appropriately
    if result.returncode != 0:
        error_indicators = [
            "LLM connection test failed",
            "Documentation generation failed",
            "Generation error",
            "LLM request failed",
            "Parsing error",
            "No suitable provider found"
        ]

        for indicator in error_indicators:
            if indicator in result.stdout:
                print(f"\n✓ Test PASSED: Tool correctly detected invalid API key (found: '{indicator}')")
                print(f"   Failed fast in {elapsed_time:.1f} seconds")
                return True

    print("\n✗ Test FAILED: Tool did not fail fast with invalid API key")
    return False

def test_without_key():
    """Test that the tool fails fast without any API key."""
    print("Testing without API key...")

    # Remove API keys
    env = os.environ.copy()
    env.pop('OPENAI_API_KEY', None)
    env.pop('ANTHROPIC_API_KEY', None)

    # Run the tool
    result = subprocess.run(
        ['./target/release/docassist', '--depth', 'quick', '--model', 'gpt-3.5-turbo', '.'],
        env=env,
        capture_output=True,
        text=True,
        timeout=5  # Should fail immediately
    )

    print("Exit code:", result.returncode)
    print("\nOutput snippet:")
    print(result.stdout[:500])

    if result.returncode != 0 and "API key" in result.stdout:
        print("\n✓ Test PASSED: Tool correctly detected missing API key")
        return True

    print("\n✗ Test FAILED: Tool did not fail appropriately without API key")
    return False

if __name__ == "__main__":
    print("=" * 60)
    print("Testing doc-assist fail-fast behavior")
    print("=" * 60)

    try:
        test1 = test_without_key()
        print("\n" + "=" * 60 + "\n")
        test2 = test_with_invalid_key()

        if test1 and test2:
            print("\n✅ All tests passed! The tool properly fails fast.")
        else:
            print("\n❌ Some tests failed. Review the output above.")
    except subprocess.TimeoutExpired:
        print("\n❌ Test timed out - tool is not failing fast!")
    except Exception as e:
        print(f"\n❌ Test error: {e}")