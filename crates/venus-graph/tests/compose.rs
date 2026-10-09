//! ss-cluster step-compose.
//! Profile off: default `docker compose` does not start the three SurrealDB processes.
//! Three processes: `--profile graph` makes :28730, :28731, and :28732 healthy at 1g each.
//! Independent disk: a row on :28731 survives a recreate of :28732.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use venus_graph::SURREAL_IMAGE;

const SERVICES: [&str; 3] = ["surreal-graph", "surreal-search-0", "surreal-search-1"];
const ONE_GIB: &str = "1073741824";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn docker(args: &[&str]) -> Output {
    Command::new("docker")
        .args(args)
        .current_dir(repo_root())
        .output()
        .unwrap_or_else(|err| panic!("docker {}: {err}", args.join(" ")))
}

fn compose(args: &[&str]) -> String {
    let mut full = vec!["compose"];
    full.extend_from_slice(args);
    let out = docker(&full);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "docker {} failed\n{stderr}\n{stdout}",
        full.join(" ")
    );
    stdout
}

fn service_lines(config_services: &str) -> Vec<&str> {
    config_services
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect()
}

/// Default `up` does not include the graph profile.
#[test]
fn profile_off() {
    let file = include_str!("../../../docker-compose.yml");
    assert!(
        file.contains("profiles: [\"graph\"]"),
        "surreal services must sit on profile graph"
    );
    assert!(file.contains(SURREAL_IMAGE));
    assert!(file.contains("SURREAL_ROCKSDB_BLOCK_CACHE_SIZE: 64MB"));
    assert!(!file.contains("SURREAL_ROCKSDB_BLOCK_CACHE_SIZE: ${"));

    let plain = compose(&["config", "--services"]);
    let names = service_lines(&plain);
    for service in SERVICES {
        assert!(
            !names.contains(&service),
            "default compose config must not start {service}: {plain}"
        );
    }
    assert!(
        names.contains(&"postgres") && names.contains(&"hub") && names.contains(&"web"),
        "default path stays postgres hub web: {plain}"
    );

    let with = compose(&["--profile", "graph", "config"]);
    for service in SERVICES {
        assert!(
            with.contains(&format!("  {service}:")),
            "profile graph must define {service}"
        );
    }
    assert!(with.contains("host_ip: 127.0.0.1"));
    assert!(
        !with.contains("host_ip: 0.0.0.0"),
        "host publish must not be 0.0.0.0"
    );
    for (host, volume) in [
        ("published: \"28730\"", "surreal-graph-data"),
        ("published: \"28731\"", "surreal-search-0-data"),
        ("published: \"28732\"", "surreal-search-1-data"),
    ] {
        assert!(with.contains(host), "missing {host}");
        assert!(with.contains(volume), "missing {volume}");
    }
    let search0 = with
        .split("surreal-search-0:")
        .nth(1)
        .expect("search-0 service");
    let search0 = search0.split("surreal-search-1:").next().unwrap();
    assert!(
        search0.contains("surreal-search-0-data") && !search0.contains("surreal-search-1-data"),
        "search-0 must mount only its own volume"
    );
}

/// `compose up --wait` fails at once on a container Docker still reports
/// `unhealthy` after an unpause, or whose name a removal from another test
/// binary still holds. Both clear within seconds.
fn graph_up(services: &[&str]) {
    let mut args = vec![
        "compose",
        "--profile",
        "graph",
        "up",
        "-d",
        "--wait",
        "--wait-timeout",
        "300",
        "--no-deps",
    ];
    args.extend_from_slice(services);
    for attempt in 1..=5 {
        for service in services {
            wait_healthy(&format!("venus-{service}-1"));
        }
        let out = docker(&args);
        if out.status.success() {
            return;
        }
        assert!(
            attempt < 5,
            "docker {}\n{}\n{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        );
        thread::sleep(Duration::from_secs(2));
    }
}

/// Docker reports a container `unhealthy` while paused and until its next probe
/// after unpause. `compose up --wait` fails on that instead of waiting.
fn wait_healthy(id: &str) {
    for _ in 0..60 {
        let out = docker(&[
            "inspect",
            "-f",
            "{{.State.Running}} {{if .State.Health}}{{.State.Health.Status}}{{end}}",
            id,
        ]);
        let text = String::from_utf8_lossy(&out.stdout);
        if text.trim() != "true unhealthy" {
            return;
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn live_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

struct GraphProfile {
    volumes: Vec<String>,
}

fn surreal_volume_names() -> Vec<String> {
    compose(&["--profile", "graph", "config"])
        .lines()
        .filter_map(|line| {
            let name = line.trim().strip_prefix("name: ")?;
            name.contains("surreal-").then(|| name.to_string())
        })
        .collect()
}

fn assert_host_port_free(port: u16) {
    let out = docker(&["ps", "--format", "{{.Names}}\t{{.Ports}}"]);
    let text = String::from_utf8_lossy(&out.stdout);
    let needle = format!(":{port}->");
    let holders: Vec<_> = text
        .lines()
        .filter(|line| line.contains(&needle) && !line.starts_with("venus-surreal-"))
        .collect();
    assert!(
        holders.is_empty(),
        "host port {port} is already published; stop that container before this test:\n{}",
        holders.join("\n")
    );
}

impl GraphProfile {
    fn start() -> Self {
        for port in [28730u16, 28731, 28732] {
            assert_host_port_free(port);
        }
        let profile = Self {
            volumes: surreal_volume_names(),
        };
        graph_up(&["surreal-graph", "surreal-search-0", "surreal-search-1"]);
        profile
    }
}

impl Drop for GraphProfile {
    fn drop(&mut self) {
        let _ = docker(&[
            "compose",
            "--profile",
            "graph",
            "rm",
            "-sfv",
            "surreal-graph",
            "surreal-search-0",
            "surreal-search-1",
        ]);
        for volume in &self.volumes {
            let _ = docker(&["volume", "rm", volume]);
        }
    }
}

fn container_id(service: &str) -> String {
    compose(&["--profile", "graph", "ps", "-q", service])
        .trim()
        .to_string()
}

fn mount_volume(service: &str) -> String {
    let id = container_id(service);
    let out = docker(&[
        "inspect",
        "-f",
        "{{range .Mounts}}{{.Destination}} {{.Name}}\n{{end}}",
        &id,
    ]);
    assert!(out.status.success(), "inspect mounts {service}");
    let text = String::from_utf8_lossy(&out.stdout);
    let data = text
        .lines()
        .find(|line| line.starts_with("/data "))
        .unwrap_or_else(|| panic!("{service} has no /data mount: {text}"));
    data.trim_start_matches("/data ").trim().to_string()
}

fn memory_bytes(service: &str) -> String {
    let id = container_id(service);
    let out = docker(&["inspect", "-f", "{{.HostConfig.Memory}}", &id]);
    assert!(out.status.success(), "inspect memory {service}");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn health_ok(port: u16) {
    let url = format!("http://127.0.0.1:{port}/health");
    let out = Command::new("curl")
        .args(["-sf", "--max-time", "5", &url])
        .output()
        .unwrap_or_else(|err| panic!("curl {url}: {err}"));
    assert!(
        out.status.success(),
        "{url} not healthy: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// :28730, :28731, and :28732 are healthy, 1g each.
#[test]
fn three_processes() {
    let _guard = live_lock();
    let _profile = GraphProfile::start();
    for (service, port) in [
        ("surreal-graph", 28730u16),
        ("surreal-search-0", 28731),
        ("surreal-search-1", 28732),
    ] {
        let status = compose(&[
            "--profile",
            "graph",
            "ps",
            "--format",
            "{{.Status}}",
            service,
        ]);
        assert!(status.contains("healthy"), "{service} status: {status}");
        assert_eq!(memory_bytes(service), ONE_GIB, "{service} mem_limit");
        health_ok(port);
    }
}

fn sql(port: u16, body: &str) -> String {
    let url = format!("http://127.0.0.1:{port}/sql");
    let out = Command::new("curl")
        .args([
            "-sS",
            "--max-time",
            "10",
            "-u",
            "venus:venus",
            "-H",
            "Accept: application/json",
            "-H",
            "Content-Type: text/plain",
            "--data-binary",
            body,
            &url,
        ])
        .output()
        .unwrap_or_else(|err| panic!("curl {url}: {err}"));
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "sql :{port} failed: {}\n{text}",
        String::from_utf8_lossy(&out.stderr)
    );
    text
}

/// A row written on :28731 is still there after :28732 is recreated, and :28732 never had it.
#[test]
fn independent_disk() {
    let _guard = live_lock();
    let profile = GraphProfile::start();
    assert_ne!(
        mount_volume("surreal-search-0"),
        mount_volume("surreal-search-1"),
        "search nodes must use different volumes: {:?}",
        profile.volumes
    );

    let write = "USE NS probe DB disk; UPSERT probe:keep SET ok = true;";
    let on_primary = sql(28731, write);
    assert!(
        on_primary.contains("\"status\":\"OK\"") && on_primary.contains("probe:keep"),
        "row missing on :28731: {on_primary}"
    );

    let on_other = sql(28732, "USE NS probe DB disk; SELECT * FROM probe:keep;");
    assert!(
        !on_other.contains("probe:keep"),
        ":28732 must not see :28731's row: {on_other}"
    );

    compose(&[
        "--profile",
        "graph",
        "up",
        "-d",
        "--wait",
        "--wait-timeout",
        "180",
        "--no-deps",
        "--force-recreate",
        "surreal-search-1",
    ]);

    let after = sql(28731, "USE NS probe DB disk; SELECT * FROM probe:keep;");
    assert!(
        after.contains("\"status\":\"OK\"") && after.contains("probe:keep"),
        "row on :28731 did not survive recreate of :28732: {after}"
    );
}
