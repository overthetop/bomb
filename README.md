# 💣 Bomb - HTTP & WebSocket Stress Testing Tool

[![Rust](https://github.com/username/bomb/workflows/CI/badge.svg)](https://github.com/username/bomb/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/bomb.svg)](https://crates.io/crates/bomb)
[![Documentation](https://docs.rs/bomb/badge.svg)](https://docs.rs/bomb)

A high-performance, production-grade Rust console application for stress-testing both HTTP endpoints and WebSocket
servers. Bomb spawns multiple concurrent clients that can either send HTTP requests to REST APIs or establish WebSocket
connections to send JSON messages and verify responses.

## 📋 Table of Contents

- [Features](#-features)
- [Quick Start](#-quick-start)
- [Installation](#-installation)
- [Usage](#-usage)
- [Random Generation](#-random-generation-system)
- [Sample Output](#-sample-output)
- [Testing Your Server](#-testing-your-websocket-server)
- [Configuration Tips](#-configuration-tips)
- [Troubleshooting](#-troubleshooting)
- [Contributing](#-contributing)

## ✨ Features

### 🌐 **Dual Protocol Support**

- **HTTP Stress Testing (beta)**: Support for GET, POST, PUT, DELETE, and PATCH methods
- **WebSocket Testing**: Full WebSocket client implementation with message verification
- **Flexible URL Support**: Supports http://, https://, ws://, and wss:// protocols

### 🚀 **Performance & Concurrency**

- **Concurrent Clients**: Spawn N concurrent clients for maximum load
- **Rate Limiting**: Configurable message/request rate per client
- **Comprehensive Metrics**: Detailed performance reporting with RTT statistics
- **Flexible Termination**: Run for a specific duration or send a total number of requests

### 🔧 **Reliability & Resilience**

- **Graceful Shutdown**: Handles Ctrl+C gracefully, waiting for pending responses
- **Connection Resilience**: Automatic WebSocket reconnection with exponential backoff
- **Timeout Handling**: Configurable response timeouts with failure tracking
- **Custom Headers**: Support for authentication and custom HTTP headers

### 📊 **Advanced Testing Features**

- **Message Verification**: Tracks unique message IDs to verify echo responses (WebSocket)
- **HTTP Status Tracking**: Monitors HTTP response codes and success rates
- **Dynamic Payloads**: Random UUID and data generation for realistic testing
- **Memory Protection**: Configurable limits to prevent resource exhaustion

## 🚀 Quick Start

```bash
# Install from crates.io
cargo install bomb

# Test WebSocket echo server
bomb -t wss://echo.websocket.org -c 2 -n 5

# Test HTTP API
bomb -t https://httpbin.org/get -m http -c 2 -n 5
```

## 📦 Installation

### Option 1: Install from crates.io (Recommended)

```bash
cargo install bomb
```

### Option 2: Build from Source

```bash
git clone https://github.com/username/bomb.git
cd bomb
cargo build --release
```

The binary will be available at `target/release/bomb`.

### Prerequisites

- Rust 1.70+ (2024 edition)
- Cargo package manager

## 🛠️ Usage

### Basic Examples

```bash
# WebSocket echo test
bomb -t wss://echo.websocket.org -c 5 -n 10

# HTTP GET test
bomb -t https://httpbin.org/get -m http -c 5 -n 10

# HTTP POST with JSON
bomb -t https://httpbin.org/post -m http --http-method post \
  -p '{"name": "test", "id": "<rnd:uuid>"}' -c 2 -n 5
```

### WebSocket Testing

```bash
# Test local WebSocket server
bomb -t ws://localhost:8080/ws -c 10 -d 30

# Test with custom payload
bomb -t ws://localhost:8080/ws -p '{"id": "<rnd:uuid>", "type": "ping"}' -c 5 -n 20

# Broadcast mode testing
bomb -t ws://localhost:8080/broadcast --ws-mode broadcast -c 5 -n 10
```

### HTTP Testing

```bash
# Load test API endpoint
bomb -t https://api.example.com/health -m http -c 20 -d 60

# Test with authentication
bomb -t https://api.example.com/data -m http \
  -H "Authorization: Bearer your-token" -c 10 -d 30

# Different HTTP methods
bomb -t https://httpbin.org/put -m http --http-method put -c 2 -n 5
bomb -t https://httpbin.org/delete -m http --http-method delete -c 2 -n 5
```

### Advanced Usage

```bash
# High load with custom headers
bomb -t wss://api.example.com/ws -c 50 -d 60 -r 20 \
  -H "Authorization: Bearer token" -H "X-API-Key: key" -v

# Dynamic URLs with random data
bomb -t "ws://localhost:8080/session/<rnd:uuid>/ws" -c 5 -n 20

# Complex JSON payload with random values
bomb -t wss://api.example.com/ws -p '{
  "id": "<rnd:uuid>",
  "userId": <rnd:int[1000, 9999]>,
  "timestamp": <rnd:ts>
}' -c 10 -r 5
```

```

### Command Line Options

```

USAGE:
bomb [OPTIONS] --target <URL>

OPTIONS:
-t, --target <URL>            Target server URL (supports random generation) [required]
-m, --mode <MODE>             Connection mode: ws (WebSocket) or http [default: ws]
-c, --clients <N>             Number of concurrent clients [default: 10]
-d, --duration <SECONDS>      Duration of test in seconds (mutually exclusive with -n)
-n, --total-messages <COUNT>  Total number of messages/requests to send across all clients
-r, --message-rate <PER_SEC>  Messages per second per client [default: 100] (0 = unlimited)
-T, --timeout <SECONDS>       Timeout to wait for response [default: 30]
-p, --payload <JSON>          JSON payload to send (supports random generation)
-H, --header <KEY:VALUE>      Add custom HTTP header (can be used multiple times)
--http-method <METHOD>        HTTP method for requests [default: get]
--ws-mode <MODE>              WebSocket mode: echo or broadcast [default: echo]
-k, --insecure Allow insecure connections (self-signed certificates)
-v, --verbose Enable verbose logging
-h, --help Print help information
-V, --version Print version information

```

## 🎲 Random Generation System

Bomb supports powerful random data generation that can be used in both target URLs and JSON payloads. This enables realistic testing scenarios with dynamic data values.

### Supported Random Patterns

| Pattern | Description | Example Input | Example Output |
|---------|-------------|---------------|----------------|
| `<rnd:uuid>` | Random UUID | `<rnd:uuid>` | `550e8400-e29b-41d4-a716-446655440000` |
| `<rnd:int[min, max]>` | Random integer in range | `<rnd:int[1, 100]>` | `42` |
| `<rnd:float[min, max]>` | Random float with precision | `<rnd:float[-0.5, 7.3]>` | `3.7` |
| `<rnd:ts[start, end]>` | Random timestamp in range | `<rnd:ts[1680000000, 1690000000]>` | `1685432100` |
| `<rnd:ts>` | Random timestamp (last 30 days) | `<rnd:ts>` | `1694567890` |
| `<rnd:datetime[start_dt, end_dt]>` | Random datetime in RFC3339 format with range | `<rnd:datetime[2024-01-01T00:00:00Z, 2024-12-31T23:59:59Z]>` | `2024-06-15T14:30:22+00:00` |
| `<rnd:datetime>` | Random datetime in RFC3339 format (last 30 days) | `<rnd:datetime>` | `2024-08-20T09:15:33+00:00` |

### Examples

```bash
# Dynamic URLs with unique session IDs
bomb -t "ws://localhost:8080/session/<rnd:uuid>/ws" -c 5 -n 20

# JSON payload with random data
bomb -t ws://api.example.com/ws -p '{
  "id": "<rnd:uuid>",
  "userId": <rnd:int[1000, 9999]>,
  "score": <rnd:float[0.0, 100.0]>,
  "timestamp": <rnd:ts>
}' -c 10 -r 5
```

**Key Features:**

- Each message gets fresh random values
- Range validation (automatically swaps min/max if needed)
- Float precision matches input format
- Works in both URLs and JSON payloads

## 📊 Sample Output

### Echo Mode (Default)

```
🚀 WebSocket Stress Test Configuration:
   Target:           ws://localhost:8080/ws
   Clients:          20
   Duration:         30s
   Message Rate:     15 msg/s per client
   Timeout:          5s
   Mode:             Echo
   Custom Headers:   2 headers
                     Authorization: Bearer token123
                     X-API-Key: secret-key

📊 WebSocket Stress Test Results
═══════════════════════════════════════════════════════════════

🔧 Configuration:
   Target:           ws://localhost:8080/ws
   Clients:          20
   Duration:         30s
   Message Rate:     15 msg/s per client
   Timeout:          5s
   Custom Headers:   2 headers
                     Authorization: Bearer token123
                     X-API-Key: secret-key

📈 Overall Results:
   Test Duration:    30.12s
   Messages Sent:    8,847
   Messages Received: 8,831
   Messages Failed:  16
   Success Rate:     99.82%

⚡ Performance:
   Messages/sec:     293.71
   Per Client:       14.69 msg/s

🔄 Round-Trip Time:
   Average RTT:      23.45ms
   Min RTT:          8ms
   Max RTT:          156ms

✅ Test completed successfully with excellent performance!
```

### Broadcast Mode

```
🚀 WebSocket Stress Test Configuration:
   Target:           ws://localhost:8080/broadcast
   Clients:          5
   Total Messages:   20
   Per Client:       4 messages
   Message Rate:     10 msg/s per client
   Timeout:          5s
   Mode:             Broadcast

📊 WebSocket Stress Test Results
═══════════════════════════════════════════════════════════════

🔧 Configuration:
   Target:           ws://localhost:8080/broadcast
   Clients:          5
   Total Messages:   20
   Per Client:       4 messages
   Message Rate:     10 msg/s per client
   Timeout:          5s
📡 Broadcast Statistics:
   Expected Deliveries: 100
   Actual Deliveries:   95
   Broadcast Completeness: 95.00%
⚠️  Incomplete Broadcasts: 5 messages
📈 Overall Results:
   Test Duration:    2.15s
   Messages Sent:    20
   Messages Received: 95
   Messages Failed:  0
   Success Rate:     100.00%

⚡ Performance:
   Messages/sec:     9.30
   Per Client:       1.86 msg/s

👥 Per-Client Summary:
   Client ID | Sent | Received | Failed | Success% | Avg RTT
   ----------|------|----------|--------|----------|--------
           0 |    4 |       20 |      0 |   100.0% |    0.0ms
           1 |    4 |       18 |      0 |   100.0% |    0.0ms
           2 |    4 |       19 |      0 |   100.0% |    0.0ms
           3 |    4 |       19 |      0 |   100.0% |    0.0ms
           4 |    4 |       19 |      0 |   100.0% |    0.0ms

🎯 Test completed successfully!
```

**Note on Broadcast Mode Success Rate**: In broadcast mode, each client receives messages from all other clients (
including their own).

## 🧪 Testing Your WebSocket Server

### Expected Server Behavior

Bomb supports two different server behavior modes:

#### Echo Mode (Default)

For echo servers, your WebSocket server should:

1. **Echo messages back**: Return the same JSON message that was sent
2. **Preserve message ID**: The `id` field must be identical in the response
3. **Handle concurrent connections**: Support multiple simultaneous clients
4. **Respond promptly**: Reply within the configured timeout period

#### Broadcast Mode

For broadcast servers, your WebSocket server should:

1. **Broadcast to all clients**: Send received messages to all connected clients
2. **Preserve message content**: Forward the complete JSON message
3. **Handle concurrent connections**: Support multiple simultaneous clients
4. **Reliable delivery**: Ensure messages reach all active connections

### Example Echo Server

Here's a simple Node.js echo server for testing:

```javascript
const WebSocket = require('ws');
const wss = new WebSocket.Server({port: 8080});

wss.on('connection', (ws) => {
    console.log('Client connected');

    ws.on('message', (data) => {
        // Echo the message back
        ws.send(data.toString());
    });

    ws.on('close', () => {
        console.log('Client disconnected');
    });
});

console.log('WebSocket server running on ws://localhost:8080');
```

### Example Broadcast Server

Here's a simple Node.js broadcast server for testing:

```javascript
const WebSocket = require('ws');
const wss = new WebSocket.Server({port: 8080});

// Track all connected clients
const clients = new Set();

wss.on('connection', (ws) => {
    console.log('Client connected');
    clients.add(ws);

    ws.on('message', (data) => {
        // Broadcast message to all connected clients
        const message = data.toString();
        clients.forEach(client => {
            if (client.readyState === WebSocket.OPEN) {
                client.send(message);
            }
        });
    });

    ws.on('close', () => {
        console.log('Client disconnected');
        clients.delete(ws);
    });
});

console.log('WebSocket broadcast server running on ws://localhost:8080');
```

### Message Format

#### Default Payload

By default, Bomb sends simple JSON messages with this structure:

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000"
}
```

#### Custom Payloads

You can send custom JSON payloads using the `-p` option. The payload **must** contain an `id` field for response
tracking:

```bash
# Simple custom payload
bomb -t ws://localhost:8080/ws -p '{"id": "<rnd:uuid>", "type": "ping"}'

# Complex payload with nested data
bomb -t ws://localhost:8080/ws -p '{
  "id": "<rnd:uuid>",
  "type": "order",
  "data": {
    "symbol": "BTCUSD",
    "side": "buy",
    "quantity": 1.5,
    "price": 45000
  },
  "timestamp": 1683024000000
}'

# Fixed ID for testing specific scenarios
bomb -t ws://localhost:8080/ws -p '{"id": "test-123", "command": "status"}'
```

#### Random Data Generation

Use the enhanced random generation patterns in your payloads:

```bash
# Multiple random patterns in one payload
bomb -t ws://localhost:8080/ws -p '{
  "id": "<rnd:uuid>",
  "sessionId": "<rnd:uuid>",
  "userId": <rnd:int[1000, 9999]>,
  "score": <rnd:float[0.0, 100.0]>,
  "timestamp": <rnd:ts>,
  "message": "test data"
}'

# Event scheduling with datetime ranges
bomb -t ws://localhost:8080/ws -p '{
  "id": "<rnd:uuid>",
  "sessionId": "<rnd:uuid>",
  "eventTime": "<rnd:datetime[2024-01-01T00:00:00Z, 2024-12-31T23:59:59Z]>",
  "userId": <rnd:int[1000, 9999]>
}'
```

**Important**: Each message gets a unique set of random values. The `id` field is used for response tracking and RTT
calculation.

## 🗺️ Common Test Patterns

### Load Testing

```bash
# Gradual load increase
bomb -t ws://your-server/api -c 10 -d 60 -r 5
bomb -t ws://your-server/api -c 25 -d 60 -r 10
bomb -t ws://your-server/api -c 50 -d 60 -r 15

# HTTP API load testing
bomb -t https://api.example.com/endpoint -m http -c 20 -d 60
```

### Capacity Testing

```bash
# Find maximum concurrent connections
bomb -t ws://server/api -c 100 -d 30 -r 1
bomb -t ws://server/api -c 500 -d 30 -r 1

# HTTP capacity testing
bomb -t https://api.example.com/health -m http -c 100 -d 30
```

### Authentication Testing

```bash
# JWT authentication
bomb -t wss://api.example.com/ws -c 10 -d 30 \
  -H "Authorization: Bearer your-jwt-token"

# API key authentication
bomb -t https://api.example.com/data -m http -c 5 -n 50 \
  -H "X-API-Key: your-api-key"
```

## 🔧 Configuration Tips

### Performance Tuning

- **Client Count**: Start with 10-20 clients (`-c 10`) and increase gradually
- **Message Rate**: Begin with 5-10 msg/s per client (`-r 5`) for baseline testing
- **Timeout**: Use 5-10 seconds (`-T 5`) for local testing, 10-30s for remote servers
- **Headers**: Add authentication/custom headers with `-H "Key: Value"`

### Network Considerations

- **Local Testing**: Use `ws://localhost` or `ws://127.0.0.1`
- **Remote Testing**: Ensure firewall allows WebSocket connections
- **SSL/TLS**: Use `wss://` for encrypted connections
- **Insecure Mode**: Only use `-k` or `--insecure` for testing with self-signed certificates
- **Authentication**: Use `-H` to add required auth headers for protected endpoints

### High Failure Rates

```
Success Rate: 45.2%
```

- Increase timeout value with `-T 10` or `--timeout 10`
- Reduce message rate with `-r 5` or `--message-rate 5`
- Check server capacity and logs
- Verify server properly echoes messages

## 📝 Development

```bash
# Run tests
cargo test

# Check code quality
cargo clippy --all-features -- -D warnings
cargo fmt
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Add tests for new functionality
5. Ensure `cargo test` and `cargo clippy` pass
6. Commit your changes (`git commit -m 'Add amazing feature'`)
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

**Made with ❤️ for stress testing**