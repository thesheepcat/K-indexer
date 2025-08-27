# K-indexer

> **⚠️ Proof of Concept - Testnet Only**  
> This is experimental software. Use only on testnet environments.

K-indexer is a simplified Kaspa transaction indexer designed specifically for indexing and serving K protocol transactions.

## 🚀 What K-indexer Does

K-indexer provides a complete indexing and API solution for the K protocol:

- **🔗 Node Connection**: Connects to your running rusty-kaspa node
- **📡 Real-time Processing**: Receives all transactions from new blocks in the BlockDAG
- **🔍 Smart Filtering**: Identifies and extracts only K protocol transactions
- **💾 Data Storage**: Persists K protocol data (posts, replies, users, etc) in a local database
- **🌐 Web API**: Serves a REST API for K web applications to access indexed data

## 📚 Protocol Documentation

Technical specifications for the K protocol are available in the [official K repository](https://github.com/thesheepcat/K).

---

## 🛠️ Installation & Setup

### Prerequisites

- Linux Ubuntu/Mint (recommended)
- Rust toolchain installed
- rusty-kaspa repository locally available
- Running rusty-kaspa node

### 📋 Step-by-Step Instructions

> **💡 Tip**: Run K-indexer on the same network as your rusty-kaspa node network for optimal performance, reducing latency.

#### 1. **Clone K-indexer**
```bash
# Clone K-indexer in the same development folder
git clone https://github.com/thesheepcat/K-indexer.git
```

#### 2. **Start Your Kaspa Node**
Follow the [documentation here on how to run rusty-kaspa](https://kaspa.aspectron.org/running-rusty-kaspa.html)

**Required Node Parameters:**
- `--testnet`: Run on testnet (required for safety)
- `--utxoindex`: Enable UTXO indexing
- `--rpclisten-borsh=0.0.0.0:17120`: Enable BORSH RPC on all interfaces

#### 3. **Build K-indexer**
```bash
cd K-indexer
cargo build --release
```

#### 4. **Run K-indexer**
```bash
cd target/release
./K-indexer --rusty-kaspa-address=localhost:17120 --database-path=/home/K-indexer/K-indexer.db
```

### ✅ Verify Connection

If everything is working correctly, you should see:

```
[2025-08-05 19:01:11 UTC] [INFO] Web server starting on 0.0.0.0:3000
[2025-08-05 19:01:11 UTC] [INFO] Connected to Kaspa node - Server: 1.0.1
```

### 🌐 Network Access

If you're running a frontend on a different machine:
- Ensure **port 3000** is open and accessible
- The API will be available at `http://your-server-ip:3000`

---

## 🔧 Configuration Options

| Parameter | Default | Description |
|-----------|---------|-------------|
| `--rusty-kaspa-address` | `localhost:17120` | Address of your rusty-kaspa node |
| `--database-path` | `k-indexer.db` | Path to database location  |
| `--bind-address` | `0.0.0.0:3000` | REST API listening port |

---

## 📖 API Endpoints

Once running, K-indexer provides REST endpoints for:
- **Posts**: Retrieve user posts
- **Replies**: Access post replies  
- **Users**: Get user profiles and introductions
- **Mentions**: Find posts where users are mentioned

You can find all details of the API techical specification in the API_TECHNICAL_SPECIFICATIONS.md document.

In case you need any support, please join us at the Kluster Discord server: https://discord.gg/vuKyjtRGKB
