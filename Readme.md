# 🧪 Ozymandias

**Ozymandias** is a declarative, container-based integration test runner written in Rust. It enables simulating production-like environments by spinning up services like **Redis Cluster**, **Kafka**, and **mock HTTP servers**, and executing timed interactions based on a single configuration file (`.toml`).

Inspired by testing characters in fiction and named after the enigmatic genius **Ozymandias** from *Watchmen*, this tool is meant for powerful and flexible scenario-based testing — no code required.

---

## ✨ Features

- 📦 **Testcontainers Integration** – spin up Kafka, Redis (cluster), and mock HTTP servers
- 🧪 **Wiremock (Rust)** – define dynamic mock API responses for internal services
- ⏱️ **Timed Interactions** – simulate production timing via declarative schedules
- 💥 **Failure Simulation** – bring down containers mid-run to test resilience
- 📊 **Assertions** – validate Kafka messages, Redis keys, and HTTP mock hits
- 🔄 **Scenario Reusability** – use TOML to define reusable test scenarios
- 📁 **Docker Support** – run via CLI or container for CI integration
- 📝 **Dry-run Mode** – validate scenarios without launching services
- 📜 **HTML/JUnit Reports** – easy to visualize or plug into CI pipelines

---

## 📐 Project Structure

```

ozymandias/
├── crates/
│   ├── cli/                 # CLI entrypoint using clap
│   ├── engine/              # Core test execution engine
│   ├── containers/          # Redis, Kafka, Wiremock orchestration
│   ├── scenario/            # TOML schema + parser
│   └── reporter/            # Output generation: console, HTML, JUnit
├── scenarios/               # Example TOML scenarios
├── Dockerfile
├── README.md
└── ...

```

---

## 🧪 Sample Scenario

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

## 🚀 Usage

### 🛠️ From Source

```bash
cargo build --release
./target/release/ozymandias run ./scenarios/config-poller-redis-down.toml
```

### 🐳 From Docker

```bash
docker build -t ozymandias .
docker run -v $(pwd)/scenarios:/scenarios ozymandias run /scenarios/config-poller-redis-down.toml
```

---

## 🔧 Configuration Options

| Feature               | Configurable via TOML |
| --------------------- | --------------------- |
| Kafka topics & timing | ✅                     |
| Redis keys/assertions | ✅                     |
| Mock API definitions  | ✅                     |
| Timing of events      | ✅                     |
| Container failures    | ✅                     |
| CLI flags (dry run)   | ✅                     |

---

## 📦 Installation (Planned)

```bash
cargo install ozymandias
```

> OR use via Docker

---

## 🧱 Built With

* [Rust](https://www.rust-lang.org/)
* [testcontainers-rs](https://crates.io/crates/testcontainers)
* [wiremock-rs](https://crates.io/crates/wiremock)
* [clap](https://crates.io/crates/clap)
* [serde](https://crates.io/crates/serde) + [toml](https://crates.io/crates/toml)

---

## 🧠 Inspiration

Named after **Ozymandias**, the brilliant strategist and manipulator in *Watchmen*, this project embodies control, simulation, and foresight in testing.

---

## 👥 Contributing

1. Clone the repo
2. Write or modify scenarios in `scenarios/*.toml`
3. Add test logic or container logic in `crates/`
4. Open a PR

Please check [CONTRIBUTING.md](CONTRIBUTING.md) for details.

---

## 📄 License

MIT © \[Your Name or Org]

```

---

Let me know if you want to include:
- Contribution badges or GitHub Actions badge
- Link to example scenarios or CI pipeline
- GitHub Pages documentation or CLI autocompletion

Want me to generate a logo or badge for **Ozymandias** as well?
```