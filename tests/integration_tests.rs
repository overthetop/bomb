use std::process::Command;
use std::str;

#[test]
fn test_bomb_e2e_with_echo_websocket() {
    // Build the binary first
    let build_output = Command::new("cargo")
        .args(["build", "--release"])
        .output()
        .expect("Failed to build bomb binary");

    assert!(
        build_output.status.success(),
        "Failed to build bomb: {}",
        str::from_utf8(&build_output.stderr).unwrap_or("Unknown error")
    );

    // Run the actual bomb command: bomb -t wss://echo.websocket.org -c 2 -n 10
    let output = Command::new("./target/release/bomb")
        .args(["-t", "wss://echo.websocket.org", "-c", "2", "-n", "10"])
        .output()
        .expect("Failed to execute bomb command");

    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    let stderr = str::from_utf8(&output.stderr).unwrap_or("");

    // Print output for debugging if test fails
    if !output.status.success() {
        eprintln!("STDOUT: {}", stdout);
        eprintln!("STDERR: {}", stderr);
    }

    // Assert that the command executed successfully
    assert!(
        output.status.success(),
        "Command failed with status: {:?}\nSTDOUT: {}\nSTDERR: {}",
        output.status.code(),
        stdout,
        stderr
    );

    // Assert expected metrics only
    assert!(
        stdout.contains("Messages Sent:    10"),
        "Expected 10 messages sent"
    );
    assert!(
        stdout.contains("Messages Received: 10"),
        "Expected 10 messages received for echo server"
    );
    assert!(
        stdout.contains("Success Rate:     100.00%"),
        "Expected 100% success rate for echo server"
    );

    println!("✅ E2E test passed - bomb successfully tested with 2 clients and 10 messages");
}

#[test]
fn test_bomb_e2e_http_mode() {
    // Build the binary first
    let build_output = Command::new("cargo")
        .args(["build", "--release"])
        .output()
        .expect("Failed to build bomb binary");

    assert!(
        build_output.status.success(),
        "Failed to build bomb: {}",
        str::from_utf8(&build_output.stderr).unwrap_or("Unknown error")
    );

    // Test HTTP GET requests using DummyJSON API test endpoint
    let output = Command::new("./target/release/bomb")
        .args([
            "-t",
            "https://dummyjson.com/test",
            "-m",
            "http",
            "--http-method",
            "get",
            "-c",
            "2",
            "-n",
            "6",
            "-r",
            "3", // Lower rate to be respectful to the API
        ])
        .output()
        .expect("Failed to execute bomb HTTP command");

    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    let stderr = str::from_utf8(&output.stderr).unwrap_or("");

    // Print output for debugging if test fails
    if !output.status.success() {
        eprintln!("STDOUT: {}", stdout);
        eprintln!("STDERR: {}", stderr);
    }

    // Assert that the command executed successfully
    assert!(
        output.status.success(),
        "HTTP command failed with status: {:?}\nSTDOUT: {}\nSTDERR: {}",
        output.status.code(),
        stdout,
        stderr
    );

    // Assert expected HTTP test results
    assert!(
        stdout.contains("Messages Sent:    6"),
        "Expected 6 HTTP requests sent"
    );
    assert!(
        stdout.contains("Messages Received: 6"),
        "Expected 6 HTTP responses received"
    );
    assert!(
        stdout.contains("Success Rate:     100.00%"),
        "Expected 100% success rate for DummyJSON API"
    );
    assert!(
        stdout.contains("Connection Mode:  Http"),
        "Expected HTTP connection mode in output"
    );
    assert!(
        stdout.contains("HTTP Method:      Get"),
        "Expected GET method in output"
    );

    println!(
        "✅ E2E HTTP test passed - bomb successfully tested HTTP mode with 2 clients and 6 requests"
    );
}

#[test]
fn test_bomb_e2e_http_post_mode() {
    // Build the binary first
    let build_output = Command::new("cargo")
        .args(["build", "--release"])
        .output()
        .expect("Failed to build bomb binary");

    assert!(
        build_output.status.success(),
        "Failed to build bomb: {}",
        str::from_utf8(&build_output.stderr).unwrap_or("Unknown error")
    );

    // Test HTTP POST requests using DummyJSON API posts endpoint
    let output = Command::new("./target/release/bomb")
        .args([
            "-t", "https://dummyjson.com/posts/add",
            "-m", "http",
            "--http-method", "post",
            "-p", r#"{"id": "<rnd:uuid>", "title": "Test Post <rnd:uuid>", "body": "This is a test post", "userId": 1}"#,
            "-c", "1",
            "-n", "3",
            "-r", "2" // Lower rate to be respectful to the API
        ])
        .output()
        .expect("Failed to execute bomb HTTP POST command");

    let stdout = str::from_utf8(&output.stdout).unwrap_or("");
    let stderr = str::from_utf8(&output.stderr).unwrap_or("");

    // Print output for debugging if test fails
    if !output.status.success() {
        eprintln!("STDOUT: {}", stdout);
        eprintln!("STDERR: {}", stderr);
    }

    // Assert that the command executed successfully
    assert!(
        output.status.success(),
        "HTTP POST command failed with status: {:?}\nSTDOUT: {}\nSTDERR: {}",
        output.status.code(),
        stdout,
        stderr
    );

    // Assert expected HTTP POST test results
    assert!(
        stdout.contains("Messages Sent:    3"),
        "Expected 3 HTTP POST requests sent"
    );
    assert!(
        stdout.contains("Messages Received: 3"),
        "Expected 3 HTTP POST responses received"
    );
    assert!(
        stdout.contains("Success Rate:     100.00%"),
        "Expected 100% success rate for DummyJSON POST API"
    );
    assert!(
        stdout.contains("Connection Mode:  Http"),
        "Expected HTTP connection mode in output"
    );
    assert!(
        stdout.contains("HTTP Method:      Post"),
        "Expected POST method in output"
    );

    println!(
        "✅ E2E HTTP POST test passed - bomb successfully tested HTTP POST mode with 1 client and 3 requests"
    );
}
