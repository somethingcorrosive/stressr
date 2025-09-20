# stressr


A no-dependencies Rust-based CPU, memory, and disk stress tester.  
**Cross-platform. Zero crates. Pure standard library.**

---

##  Features

-  CPU load (configurable % per core)
-  Memory stress (as a percentage of system RAM)
-  Disk I/O stress: read/write, sequential/random, multi-path, multi-threaded
-  100% pure Rust standard library — **no `clap`, no `sysinfo`, no `rand`**, no dependencies!

---

##  Usage

### Build:

```bash
cargo build --release
```

```bash
./target/release/stressr \
  --cpu-percent 60 \
  --memory-percent 40 \
  --duration 30 \
  --io \
  --io-paths /tmp \
  --io-workers 2 \
  --io-size 100 \
  --io-duration 30 \
  --io-random \
  --io-read \
  --io-write \
  --chunk-size 64
```

### CLI Flags

| Flag                     | Description                                      |
|--------------------------|--------------------------------------------------|
| `--cpu-percent <N>`      | CPU load per thread (0–100)                      |
| `--memory-percent <N>`   | Percent of total RAM to allocate                 |
| `--duration <SECS>`      | Duration for CPU and memory stress (seconds)     |
| `--io`                   | Enable disk I/O stress                           |
| `--io-paths <DIR1,...>`  | Comma-separated list of target directories       |
| `--io-workers <N>`       | Number of threads per path                       |
| `--io-size <MB>`         | Size in MB to allocate per worker                |
| `--io-duration <SECS>`   | Duration of I/O stress test                      |
| `--io-read`              | Enable disk reads                                |
| `--io-write`             | Enable disk writes                               |
| `--io-random`            | Enable random (seek-based) access                |
| `--chunk-size <KB>`      | Chunk size per read/write operation              |


### Run Tests

```bash
cargo test
```

#### Includes internal sanity checks for:
- Memory detection
- Disk I/O write to `/tmp`
- Short CPU spin test


