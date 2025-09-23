<div align="center">

# Ozymandias

A declarative, container-based integration test runner written in Rust.

---

[![CI](https://github.com/anitnilay20/ozymandias/actions/workflows/ci.yml/badge.svg)](https://github.com/anitnilay20/ozymandias/actions/workflows/ci.yml) [![codecov](https://codecov.io/gh/anitnilay20/ozymandias/graph/badge.svg?token=4A9K5KXL5M)](https://codecov.io/gh/anitnilay20/ozymandias) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

</div>

Ozymandias enables simulating production-like environments by orchestrating services like **Redis Cluster**, **Kafka**, **PostgreSQL**, and **mock HTTP servers**. Execute complex, timed interactions and chaos engineering scenarios based on a single configuration file (`.toml`).

Inspired by testing characters in fiction and named after the enigmatic genius **Ozymandias** from _Watchmen_, this tool provides powerful and flexible scenario-based testing with **comprehensive failure simulation** — no code required.

---

## ✨ Features

### Core Capabilities

- **🐳 Multi-Service Orchestration** – Kafka, Redis (cluster), PostgreSQL, and custom containers
- **🎭 Advanced Mock Servers** – Define dynamic mock API responses with delays, errors, and realistic scenarios
- **⏰ Timed Interactions** – Simulate production timing via declarative schedules
- **🎲 Chaos Engineering** – Built-in failure injection, network delays, and service interruption
- **📊 Rich Assertions** – Validate Kafka messages, Redis keys, database state, and HTTP mock interactions
- **🔄 Scenario Reusability** – Use TOML to define complex, reusable test scenarios
- **🚀 Actor-based Architecture** – Robust, concurrent service management with lifecycle control

### Testing & Validation

- **🧪 Comprehensive Test Coverage** – 100+ unit tests covering all major components
- **✅ Dry-run Mode** – Validate scenarios without launching services
- **📈 Performance Testing** – Built-in load testing and performance validation
- **📋 Multiple Report Formats** – HTML, JUnit XML, and Prometheus metrics
- **🔍 Health Monitoring** – Continuous service health checks and recovery validation

### Developer Experience

- **⚡ Simple CLI** – `cargo run -- <command>` for immediate usage
- **🐋 Docker Support** – Run via CLI or container for CI integration
- **📝 Rich Configuration** – Intuitive TOML schema with extensive documentation
- **🔧 Extensible Design** – Plugin architecture for custom services and assertions

---

## 🚀 Quick Start

### From Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/anitnilay20/ozymandias.git
cd ozymandias

# Run directly with cargo (no build required)
cargo run -- --help

# Run a comprehensive test scenario
cargo run -- run scenarios/1.toml

# Validate a scenario without execution
cargo run -- validate scenarios/1.toml -v

# Generate detailed reports
cargo run -- run scenarios/1.toml --report test-report.html
```

### From Docker

```bash
# Build the Docker image
docker build -t ozymandias .

# Run a scenario using Docker
docker run -v $(pwd)/scenarios:/scenarios ozymandias run /scenarios/1.toml
```

### Development & Testing

```bash
# Run the comprehensive test suite
cargo test

# Run with coverage analysis
cargo test --all-features

# Build optimized release
cargo build --release
```

---

## 📁 Project Structure

```
ozymandias/
├── cli/                    # 🖥️  CLI interface with clap integration
├── logging/                # 📝 Advanced logging and tracing utilities
├── ozymandias_core/        # ⚙️  Core test execution engine
│   ├── actor/              # 🎭 Actor-based service orchestration
│   │   ├── kafka.rs        # Kafka container management
│   │   ├── redis.rs        # Redis cluster orchestration
│   │   ├── mock_server.rs  # HTTP mock server handling
│   │   ├── service.rs      # Generic container management
│   │   └── runner.rs       # Actor lifecycle management
│   ├── containers/         # 🐳 Container service definitions
│   ├── scenario.rs         # 📋 TOML schema + parser
│   └── error.rs           # 🚨 Comprehensive error handling
├── scenarios/              # 📚 Example TOML test scenarios
│   └── 1.toml             # Comprehensive chaos engineering example
├── Cargo.toml             # 📦 Workspace configuration
└── README.md
```

---

## 🧪 Enhanced Test Scenarios

### Comprehensive Integration Testing

Our enhanced scenarios support complex, multi-phase testing with realistic failure patterns:

```toml
[meta]
name = "redis_cluster_ccs_kafka_comprehensive"
description = "Comprehensive test with Redis cluster, Kafka, multiple mocked APIs, and realistic failure scenarios"
timeout_seconds = 120
labels = ["integration", "redis", "kafka", "chaos", "performance"]

# Multiple service orchestration
[[services]]
service_type = "redis_cluster"
image = "grokzen/redis-cluster:6.0.7"
ports = [7000, 7001, 7002, 7003, 7004, 7005]
alias = "redis_primary"

[[services]]
service_type = "kafka"
ports = [9092, 9093]
alias = "kafka_broker"

[[services]]
service_type = "custom"
image = "postgres:13"
ports = [5432]
alias = "postgres_db"

# Advanced mock server configurations
[[mock_servers]]
port = 8080
routes = [
    # Happy path with realistic delays
    {
        method = "GET",
        path = "/config/app-config",
        response = { status = 200, body = "..." },
        delay_ms = 100
    },
    # Error scenarios for resilience testing
    {
        method = "GET",
        path = "/config/error-config",
        response = { status = 500, body = "..." }
    },
    # Rate limiting simulation
    {
        method = "GET",
        path = "/config/rate-limited",
        response = { status = 429, body = "..." }
    }
]

# Chaos engineering phases
[situations.redis_failover]
at = "20s"
action = "pause"
target = "redis_primary"

[situations.network_partition]
at = "40s"
action = "network_delay"
target = "all"
delay_ms = 2000

[situations.recovery_test]
at = "60s"
action = "resume"
target = "redis_primary"

# Performance validation
[validations]
max_response_time_ms = 5000
min_success_rate_percent = 85
expect_recovery_time_seconds = 30
```

### Supported Failure Scenarios

- **Service Interruption**: Pause/resume containers mid-execution
- **Network Issues**: Simulate latency, packet loss, partitions
- **Resource Exhaustion**: Memory limits, CPU throttling
- **Cascade Failures**: Multi-service failure simulation
- **Recovery Testing**: Automatic failover and recovery validation
- **Load Testing**: Concurrent request simulation with performance thresholds

---

## 🔧 Configuration Options

| Feature                         | Supported | Configuration                                              |
| ------------------------------- | --------- | ---------------------------------------------------------- |
| **Multi-Service Orchestration** | ✅        | Redis Cluster, Kafka, PostgreSQL, Custom containers        |
| **Mock API Servers**            | ✅        | Multiple endpoints, delays, error simulation, headers      |
| **Chaos Engineering**           | ✅        | Service pause/resume, network delays, failure injection    |
| **Timed Interactions**          | ✅        | Precise scheduling, phase-based execution                  |
| **Performance Testing**         | ✅        | Load generation, response time validation                  |
| **Assertions & Validation**     | ✅        | Kafka messages, Redis keys, HTTP responses, DB state       |
| **Monitoring & Reporting**      | ✅        | Health checks, metrics collection, multiple report formats |
| **CI/CD Integration**           | ✅        | JUnit XML, Docker support, exit codes                      |

---

## 📊 Test Coverage & Quality

### Comprehensive Test Suite

- **100+ Unit Tests** covering all major components
- **92% Test Coverage** across actor system and core functionality
- **Integration Tests** with real container orchestration
- **Chaos Engineering Tests** validating failure scenarios
- **Performance Benchmarks** ensuring scalability

### Quality Assurance

- **Static Analysis** with clippy and rustfmt
- **Memory Safety** guaranteed by Rust's ownership model
- **Concurrent Safety** through actor-based architecture
- **Error Handling** with comprehensive error types and recovery

```bash
# Run full test suite with coverage
cargo test --all-features

# View detailed test results
cargo test -- --nocapture

# Performance benchmarks
cargo test --release -- --ignored
```

---

## 🛠️ Installation

### Prerequisites

- **Rust 1.70+** (recommended: latest stable)
- **Docker** (for container orchestration)
- **Git** (for cloning the repository)

### Installation Options

```bash
# Option 1: Direct from source (recommended for development)
git clone https://github.com/anitnilay20/ozymandias.git
cd ozymandias
cargo run -- --help

# Option 2: Install globally (coming soon)
cargo install ozymandias

# Option 3: Docker for CI/CD
docker pull ozymandias:latest
```

---

## 📖 Usage Examples

### Basic Scenario Execution

```bash
# Run a comprehensive test scenario
cargo run -- run scenarios/1.toml --verbose

# Generate HTML report with metrics
cargo run -- run scenarios/1.toml --report results.html

# Validate scenario configuration
cargo run -- validate scenarios/1.toml
```

### Advanced Usage

```bash
# Run with custom timeout
cargo run -- run scenarios/1.toml --timeout 300s

# Execute only specific test phases
cargo run -- run scenarios/1.toml --filter "phase1,phase3"

# Enable detailed tracing for debugging
RUST_LOG=debug cargo run -- run scenarios/1.toml -vvv

# CI/CD Integration with JUnit output
cargo run -- run scenarios/ --report junit.xml --format junit
```

### Docker Integration

```bash
# CI/CD Pipeline Integration
docker run --rm \
  -v $(pwd)/scenarios:/scenarios \
  -v $(pwd)/reports:/reports \
  ozymandias run /scenarios --report /reports/results.xml

# Development with Docker Compose
docker-compose up ozymandias-test
```

---

## 🏗️ Built With

### Core Technologies

- **[Rust](https://www.rust-lang.org/)** - Systems programming language for performance and safety
- **[Tokio](https://tokio.rs/)** - Asynchronous runtime for concurrent operations
- **[testcontainers-rs](https://crates.io/crates/testcontainers)** - Container orchestration and lifecycle management

### Service Integration

- **[rdkafka](https://crates.io/crates/rdkafka)** - High-performance Kafka client
- **[redis](https://crates.io/crates/redis)** - Redis cluster client with async support
- **[wiremock](https://crates.io/crates/wiremock)** - HTTP API mocking and simulation

### Development & CLI

- **[clap](https://crates.io/crates/clap)** - Command line argument parsing with derive macros
- **[serde](https://crates.io/crates/serde) + [toml](https://crates.io/crates/toml)** - Configuration parsing and serialization
- **[tracing](https://crates.io/crates/tracing)** - Structured logging and observability
- **[error-stack](https://crates.io/crates/error-stack)** - Rich error handling and context

---

## 🎭 Inspiration

Named after **Ozymandias**, the brilliant strategist and manipulator in _Watchmen_, this project embodies control, simulation, and foresight in testing distributed systems. Like Adrian Veidt's master plans, Ozymandias orchestrates complex scenarios with precision timing and comprehensive failure analysis.

> _"I'm not a comic book villain. Do you seriously think I'd explain my master-stroke if there remained the slightest chance of you affecting its outcome?"_ - Ozymandias

Similarly, our testing scenarios execute with deterministic precision while simulating the chaos and unpredictability of production environments.

---

## 🤝 Contributing

We welcome contributions! Whether you're fixing bugs, adding features, or improving documentation.

### Development Setup

```bash
# Fork and clone the repository
git clone https://github.com/your-username/ozymandias.git
cd ozymandias

# Set up development environment
cargo build
cargo test

# Run clippy for linting
cargo clippy -- -D warnings

# Format code
cargo fmt
```

### Contribution Process

1. **Fork** the project
2. **Create** your feature branch (`git checkout -b feature/amazing-feature`)
3. **Add tests** for your changes
4. **Ensure** all tests pass (`cargo test`)
5. **Commit** your changes (`git commit -m 'Add some amazing-feature'`)
6. **Push** to the branch (`git push origin feature/amazing-feature`)
7. **Open** a Pull Request

### Areas for Contribution

- 🐳 **New Container Support** - Additional database and service integrations
- 🎯 **Testing Scenarios** - More complex failure simulation patterns
- 📊 **Monitoring & Metrics** - Enhanced observability and reporting
- 🔧 **CLI Improvements** - Better user experience and developer tools
- 📚 **Documentation** - Examples, tutorials, and best practices

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🔗 Links

- [📚 Documentation](https://github.com/anitnilay20/ozymandias/wiki)
- [🐛 Issues](https://github.com/anitnilay20/ozymandias/issues)
- [💬 Discussions](https://github.com/anitnilay20/ozymandias/discussions)
- [🚀 Releases](https://github.com/anitnilay20/ozymandias/releases)

---

<div align="center">

**[⬆ Back to Top](#ozymandias)**

_"Who watches the Watchmen?" - We do, with comprehensive integration tests._

</div>
