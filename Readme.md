<div align="center">

# Ozymandias

A declarative, container-based integration test runner written in Rust.

---

[![CI](https://github.com/anitnilay20/ozymandias/actions/workflows/ci.yml/badge.svg)](https://github.com/anitnilay20/ozymandias/actions/workflows/ci.yml) [![codecov](https://codecov.io/gh/anitnilay20/ozymandias/graph/badge.svg?token=4A9K5KXL5M)](https://codecov.io/gh/anitnilay20/ozymandias) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
</div>

It enables simulating production-like environments by spinning up services like **Redis Cluster**, **Kafka**, and **mock HTTP servers**, and executing timed interactions based on a single configuration file (`.toml`).

Inspired by testing characters in fiction and named after the enigmatic genius **Ozymandias** from *Watchmen*, this tool is meant for powerful and flexible scenario-based testing — no code required.


## Features

- **Testcontainers Integration** – Spin up Kafka, Redis (cluster), and mock HTTP servers
- **Wiremock (Rust)** – Define dynamic mock API responses for internal services
- **Timed Interactions** – Simulate production timing via declarative schedules
- **Failure Simulation** – Bring down containers mid-run to test resilience
- **Assertions** – Validate Kafka messages, Redis keys, and HTTP mock hits
- **Scenario Reusability** – Use TOML to define reusable test scenarios
- **Docker Support** – Run via CLI or container for CI integration
- **Dry-run Mode** – Validate scenarios without launching services
- **HTML/JUnit Reports** – Easy to visualize or plug into CI pipelines

---

## Quick Start

### From Source

```bash
# Clone the repository
git clone https://github.com/anitnilay20/ozymandias.git
cd ozymandias

# Build the project
cargo build --release

# Run a sample scenario
./target/release/ozymandias run ./scenarios/1.toml
```

### From Docker

```bash
# Build the Docker image
docker build -t ozymandias .

# Run a scenario using Docker
docker run -v $(pwd)/scenarios:/scenarios ozymandias run /scenarios/1.toml
```

---

## Project Structure

```
ozymandias/
├── cli/                  # CLI interface using clap
├── logging/              # Logging utilities
├── ozymandias_core/      # Core test execution engine
│   ├── containers/       # Redis, Kafka, Wiremock orchestration
│   ├── scenario.rs       # TOML schema + parser
│   └── mock_server.rs    # HTTP mock server handling
├── scenarios/            # Example TOML scenarios
├── Cargo.toml
└── README.md
```

---

## Sample Scenario

```toml
[metadata]
name = "config-poller-redis-down"

[services.kafka]
topics = ["config-input"]
startup_delay_ms = 0

[services.redis_cluster]
startup_delay_ms = 0

[services.ccs]
mocks = [
  { path = "/config", method = "GET", status = 200, body = '{"config": "v1"}' }
]

[events]
# Time is relative to test start in ms
[[events]]
time_ms = 5000
action = "stop"
target = "redis_cluster"

[[events]]
time_ms = 10000
action = "start"
target = "redis_cluster"

[assertions.redis]
keys_exist = ["config:v1"]

[assertions.kafka]
received_messages = [
  { topic = "config-input", count = 1 }
]
```

---

## Configuration Options

| Feature               | Configurable via TOML |
| --------------------- | --------------------- |
| Kafka topics & timing | ✅                     |
| Redis keys/assertions | ✅                     |
| Mock API definitions  | ✅                     |
| Timing of events      | ✅                     |
| Container failures    | ✅                     |
| CLI flags (dry run)   | ✅                     |

---

## Installation

```bash
# Coming soon
cargo install ozymandias
```

> Alternatively, use the Docker image for CI integration

---

## Built With

* [Rust](https://www.rust-lang.org/) - Systems programming language
* [testcontainers-rs](https://crates.io/crates/testcontainers) - Container orchestration
* [wiremock-rs](https://crates.io/crates/wiremock) - HTTP API mocking
* [clap](https://crates.io/crates/clap) - Command line argument parsing
* [serde](https://crates.io/crates/serde) + [toml](https://crates.io/crates/toml) - Configuration parsing

---

## Inspiration

Named after **Ozymandias**, the brilliant strategist and manipulator in *Watchmen*, this project embodies control, simulation, and foresight in testing.

---

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the project
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
