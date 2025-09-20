use std::env;
use std::fs::{OpenOptions, remove_file};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

fn print_help() {
    println!(
        r#"stressr - A zero-dependency Rust stress testing tool

Usage:
  stressr [OPTIONS]

General Options:
  --cpu-percent <N>       CPU load per thread (0–100)
  --memory-percent <N>    Percent of total RAM to allocate
  --duration <SECS>       Duration for CPU and memory stress (seconds)

Disk I/O Options:
  --io                    Enable disk I/O stress
  --io-paths <DIR1,...>   Comma-separated list of target directories
  --io-workers <N>        Number of threads per path
  --io-size <MB>          Size in MB to allocate per worker
  --io-duration <SECS>    Duration of I/O stress test
  --io-read               Enable disk reads
  --io-write              Enable disk writes
  --io-random             Enable random (seek-based) access
  --chunk-size <KB>       Chunk size per read/write operation

Help:
  -h, --help              Show this help message
"#
    );
}

#[derive(Debug)]
struct Config {
    cpu_percent: u64,
    memory_percent: u64,
    duration_secs: u64,
    io_enabled: bool,
    io_paths: Vec<String>,
    io_workers: usize,
    io_size_mb: u64,
    io_duration_secs: u64,
    io_random: bool,
    io_read: bool,
    io_write: bool,
    chunk_size_kb: usize,
}

impl Config {
    fn from_args() -> Self {
        let args: Vec<String> = env::args().collect();

        if args.iter().any(|a| a == "--version" || a == "-v") {
            println!("stressr v0.1.0");
            std::process::exit(0);
        }

        if args.len() == 1 || args.iter().any(|arg| arg == "--help" || arg == "-h") {
            print_help();
            std::process::exit(0);
        }

        let mut cfg = Config {
            cpu_percent: 0,
            memory_percent: 0,
            duration_secs: 30,
            io_enabled: false,
            io_paths: vec!["/tmp".into()],
            io_workers: 2,
            io_size_mb: 100,
            io_duration_secs: 30,
            io_random: false,
            io_read: false,
            io_write: false,
            chunk_size_kb: 64,
        };

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--cpu-percent" => {
                    i += 1;
                    cfg.cpu_percent = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(100);
                }
                "--memory-percent" => {
                    i += 1;
                    cfg.memory_percent = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(100);
                }
                "--duration" => {
                    i += 1;
                    cfg.duration_secs = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(30);
                }
                "--io" => cfg.io_enabled = true,
                "--io-paths" => {
                    i += 1;
                    cfg.io_paths = args
                        .get(i)
                        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or(vec!["/tmp".into()]);
                }
                "--io-workers" => {
                    i += 1;
                    cfg.io_workers = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(2);
                }
                "--io-size" => {
                    i += 1;
                    cfg.io_size_mb = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(100);
                }
                "--io-duration" => {
                    i += 1;
                    cfg.io_duration_secs = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(30);
                }
                "--io-random" => cfg.io_random = true,
                "--io-read" => cfg.io_read = true,
                "--io-write" => cfg.io_write = true,
                "--chunk-size" => {
                    i += 1;
                    cfg.chunk_size_kb = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(64);
                }
                _ => {}
            }
            i += 1;
        }

        cfg
    }
}

fn stress_cpu(percent: u64, duration: Duration) {
    let threads = thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let busy = Duration::from_millis(percent);
    let idle = Duration::from_millis(100 - percent);

    println!("CPU: {} threads @ {}%", threads, percent);

    let mut handles = vec![];
    for _ in 0..threads {
        let busy = busy.clone();
        let idle = idle.clone();
        let duration = duration.clone();

        handles.push(thread::spawn(move || {
            let start = Instant::now();
            while start.elapsed() < duration {
                let t0 = Instant::now();
                while t0.elapsed() < busy {
                    std::hint::black_box(1 + 1);
                }
                thread::sleep(idle);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

fn read_total_memory_kb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(contents) = std::fs::read_to_string("/proc/meminfo") {
            for line in contents.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            return kb;
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        if let Ok(output) = Command::new("sysctl")
            .arg("hw.memsize")
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(mem_bytes) = stdout.split_whitespace().last() {
                    if let Ok(bytes) = mem_bytes.parse::<u64>() {
                        return bytes / 1024;
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        use std::mem::MaybeUninit;

        #[repr(C)]
        struct MEMORYSTATUSEX {
            dwLength: u32,
            dwMemoryLoad: u32,
            ullTotalPhys: u64,
            ullAvailPhys: u64,
            ullTotalPageFile: u64,
            ullAvailPageFile: u64,
            ullTotalVirtual: u64,
            ullAvailVirtual: u64,
            ullAvailExtendedVirtual: u64,
        }

        extern "system" {
            fn GlobalMemoryStatusEx(lpBuffer: *mut MEMORYSTATUSEX) -> i32;
        }

        unsafe {
            let mut mem_info = MaybeUninit::<MEMORYSTATUSEX>::zeroed();
            (*mem_info.as_mut_ptr()).dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;

            if unsafe { GlobalMemoryStatusEx(mem_info.as_mut_ptr()) } != 0 {
                let mem_info = mem_info.assume_init();
                return mem_info.ullTotalPhys / 1024;
            }
        }
    }

    println!("Unable to detect total memory, using fallback 1GB");
    1024 * 1024
}

fn stress_memory(percent: u64, duration: Duration) {
    let total_kb = read_total_memory_kb();
    let target_kb = total_kb * percent / 100;

    println!("Memory: Allocating ~{} MB", target_kb / 1024);

    let mut blocks = Vec::new();
    while (blocks.len() as u64) * 1024 < target_kb {
        blocks.push(vec![0u8; 1024 * 1024]);
    }

    thread::sleep(duration);
}

fn simple_prng(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    *state
}

fn disk_io_worker(
    path: &str,
    worker_id: usize,
    file_size_mb: u64,
    duration: Duration,
    chunk_kb: usize,
    random: bool,
    read: bool,
    write: bool,
) {
    let chunk_size = chunk_kb * 1024;
    let total_bytes = file_size_mb * 1024 * 1024;

    let file_path = PathBuf::from(path).join(format!("worker_{}.tmp", worker_id));
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&file_path)
        .expect("failed to open file");

    file.set_len(total_bytes).unwrap();

    let mut buffer = vec![0u8; chunk_size];
    let mut state = worker_id as u64;

    let start = Instant::now();
    let mut total_bytes_processed = 0;
    let mut ops = 0;

    while start.elapsed() < duration {
        let offset = if random {
            simple_prng(&mut state) % (total_bytes - chunk_size as u64)
        } else {
            (ops * chunk_size) as u64 % (total_bytes - chunk_size as u64)
        };

        if write {
            for b in buffer.iter_mut() {
                *b = (simple_prng(&mut state) % 256) as u8;
            }
            file.seek(SeekFrom::Start(offset)).unwrap();
            file.write_all(&buffer).unwrap();
        }

        if read {
            file.seek(SeekFrom::Start(offset)).unwrap();
            file.read_exact(&mut buffer).unwrap();
        }

        ops += 1;
        total_bytes_processed += chunk_size;
    }

    let mbps = (total_bytes_processed as f64) / start.elapsed().as_secs_f64() / 1024.0 / 1024.0;

    println!(
        "[I/O Worker {}] {:.2} MB/s | {} ops | mode={}{}",
        worker_id,
        mbps,
        ops,
        if write { "W" } else { "" },
        if read { "R" } else { "" }
    );

    let _ = remove_file(&file_path);
}

fn run_disk_io(cfg: &Config) {
    let mut handles = vec![];

    for path in &cfg.io_paths {
        for id in 0..cfg.io_workers {
            let path = path.clone();
            let dur = Duration::from_secs(cfg.io_duration_secs);
            let size = cfg.io_size_mb;
            let chunk = cfg.chunk_size_kb;
            let rand = cfg.io_random;
            let read = cfg.io_read;
            let write = cfg.io_write;

            handles.push(thread::spawn(move || {
                disk_io_worker(&path, id, size, dur, chunk, rand, read, write);
            }));
        }
    }

    for h in handles {
        h.join().unwrap();
    }
}

fn main() {
    let cfg = Config::from_args();
    println!("Running stress test:\n{:#?}", cfg);

    let mut handles = vec![];

    if cfg.cpu_percent > 0 {
        let dur = Duration::from_secs(cfg.duration_secs);
        handles.push(thread::spawn(move || {
            stress_cpu(cfg.cpu_percent, dur);
        }));
    }

    if cfg.memory_percent > 0 {
        let dur = Duration::from_secs(cfg.duration_secs);
        handles.push(thread::spawn(move || {
            stress_memory(cfg.memory_percent, dur);
        }));
    }

    if cfg.io_enabled {
        handles.push(thread::spawn(move || {
            run_disk_io(&cfg);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    println!("Done");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_detect_memory() {
        let mem_kb = read_total_memory_kb();
        assert!(mem_kb > 128_000, "Should detect >128MB of RAM, got {}", mem_kb);
    }

    #[test]
    fn test_simple_memory_stress() {
        stress_memory(1, Duration::from_secs(1));
    }

    #[test]
    fn test_disk_io_worker_write_only() {
        let tmp = std::env::temp_dir();
        let path = tmp.to_str().unwrap_or("/tmp");

        disk_io_worker(
            path,
            9999,
            1, // 1MB
            Duration::from_secs(1),
            4, // 4KB
            false, // sequential
            false, // no read
            true,  // yes write
        );
    }

    #[test]
    fn test_cpu_stress_smoke() {
        stress_cpu(10, Duration::from_secs(1));
    }
}
