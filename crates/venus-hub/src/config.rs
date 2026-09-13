//! Database URL and listen address from the environment.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::env;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

/// Product store is Postgres. SQLite / omitting the DSN is a bug.
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    /// Effective `sslmode` for logs (S8). Never log [`Self::database_url`] — it carries the password.
    /// Parts path: `POSTGRES_SSLMODE` (default [`DEFAULT_PG_SSLMODE`]). `DATABASE_URL` path: the
    /// query value, or `unset` when the DSN has none (the URL is not rewritten).
    pub pg_sslmode: String,
    pub listen: SocketAddr,
    pub owner: String,
    pub persist_interval: Duration,
    pub lease_ttl: Duration,
    /// Process-wide lease refresh. Derived as `lease_ttl / 3` unless `HUB_HEARTBEAT_INTERVAL_SECS` is set.
    pub heartbeat_interval: Duration,
    pub compact_after: i64,
    /// sqlx pool. Required at start (P17). Tests that skip `from_env` use `db::connect`.
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub db_acquire_timeout: Duration,
    /// Session `work_mem` for hub connections (P5). A cap-sized flush
    /// materialises an 8 MiB `bytea[]` parameter and spills to a temp file at
    /// the 4 MB server default. Per node per sort/hash — size it against
    /// [`Self::db_max_connections`].
    pub db_work_mem: String,
    /// CORS allow list (S9). Unset env → [`DEFAULT_CORS_ORIGINS`]. Empty env → no CORS (same-origin nginx).
    pub cors_origins: Vec<String>,
}

/// Compose default. Private-network Postgres; operators targeting managed Postgres set `require`.
pub const DEFAULT_PG_SSLMODE: &str = "disable";

/// libpq tokens. Anything else is rejected so a typo cannot inject extra query params.
const PG_SSLMODES: &[&str] = &[
    "disable",
    "allow",
    "prefer",
    "require",
    "verify-ca",
    "verify-full",
];

impl Config {
    pub fn from_env() -> Result<Self> {
        let listen = env::var("HUB_LISTEN")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "0.0.0.0:3000".into())
            .parse()
            .context("HUB_LISTEN")?;
        let owner = env::var("HUB_OWNER")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(default_owner);
        let lease_ttl = duration_secs_env("HUB_LEASE_TTL_SECS", 20)?;
        let heartbeat_interval = match optional_secs_env("HUB_HEARTBEAT_INTERVAL_SECS")? {
            Some(interval) => interval,
            None => heartbeat_interval_from_ttl(lease_ttl),
        };
        validate_lease_ratio(lease_ttl, heartbeat_interval)?;
        let persist_interval = persist_interval_from_env()?;
        let compact_after = compact_after_from_env()?;
        let db_max_connections = db_max_connections_from_env()?;
        let db_min_connections = db_min_connections_from_env()?;
        let db_acquire_timeout = db_acquire_timeout_from_env()?;
        let db_work_mem = db_work_mem_from_env()?;
        validate_pool_sizes(db_max_connections, db_min_connections)?;
        let cors_origins = cors_origins_from_env()?;
        let (database_url, pg_sslmode) = dsn_from_env()?;
        Ok(Self {
            database_url,
            pg_sslmode,
            listen,
            owner,
            persist_interval,
            lease_ttl,
            heartbeat_interval,
            compact_after,
            db_max_connections,
            db_min_connections,
            db_acquire_timeout,
            db_work_mem,
            cors_origins,
        })
    }
}

/// Vite `:5173`/`:5174` and Compose `:8080`, both `localhost` and `127.0.0.1`.
pub const DEFAULT_CORS_ORIGINS: [&str; 6] = [
    "http://localhost:5173",
    "http://127.0.0.1:5173",
    "http://localhost:5174",
    "http://127.0.0.1:5174",
    "http://localhost:8080",
    "http://127.0.0.1:8080",
];

pub fn default_cors_origins() -> Vec<String> {
    DEFAULT_CORS_ORIGINS
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

fn cors_origins_from_env() -> Result<Vec<String>> {
    match env::var("HUB_CORS_ORIGINS") {
        Err(_) => Ok(default_cors_origins()),
        Ok(s) => parse_cors_origins(&s),
    }
}

/// Comma-separated `http(s)://host[:port]`. Empty / whitespace → no origins.
/// `*` is rejected (that would echo the request origin).
pub fn parse_cors_origins(raw: &str) -> Result<Vec<String>> {
    let s = raw.trim();
    if s.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let origin = parse_cors_origin(part)?;
        if !out.iter().any(|o| o == &origin) {
            out.push(origin);
        }
    }
    Ok(out)
}

pub fn parse_cors_origin(raw: &str) -> Result<String> {
    if raw == "*" {
        bail!("HUB_CORS_ORIGINS must not be *");
    }
    let rest = match raw.strip_prefix("http://") {
        Some(r) => r,
        None => match raw.strip_prefix("https://") {
            Some(r) => r,
            None => bail!("HUB_CORS_ORIGINS entries must be http(s) origins with no path"),
        },
    };
    if rest.is_empty()
        || rest.contains('/')
        || rest.contains('?')
        || rest.contains('#')
        || rest.contains('@')
        || rest.contains(' ')
    {
        bail!("HUB_CORS_ORIGINS entries must be http(s) origins with no path");
    }
    let host = match rest.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => {
            h
        }
        _ => rest,
    };
    if host.is_empty() {
        bail!("HUB_CORS_ORIGINS entries must be http(s) origins with no path");
    }
    let host_ok = host == "localhost"
        || host.split('.').all(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        });
    if !host_ok {
        bail!("HUB_CORS_ORIGINS entries must be http(s) origins with no path");
    }
    Ok(raw.to_string())
}

/// Whole seconds: `ttl / 3` so at least three beats fit in one TTL (L11).
pub fn heartbeat_interval_from_ttl(lease_ttl: Duration) -> Duration {
    Duration::from_secs(lease_ttl.as_secs() / 3)
}

/// Reject ratios where one missed beat makes the row stealable.
pub fn validate_lease_ratio(lease_ttl: Duration, heartbeat_interval: Duration) -> Result<()> {
    let ttl = lease_ttl.as_secs();
    let hb = heartbeat_interval.as_secs();
    if hb == 0 {
        bail!("heartbeat_interval must be at least 1s");
    }
    if ttl < 3 * hb {
        bail!(
            "lease_ttl ({ttl}s) must be >= 3 × heartbeat_interval ({hb}s); \
             a single missed beat must not make the row stealable"
        );
    }
    Ok(())
}

/// `0` made `trail_len >= 0` always true and compacted every persist tick (P3).
pub fn validate_compact_after(n: i64) -> Result<i64> {
    if n < 1 {
        bail!("HUB_COMPACT_AFTER must be >= 1");
    }
    Ok(n)
}

/// `min > max` is a sqlx panic at connect. Fail at config instead.
pub fn validate_pool_sizes(max: u32, min: u32) -> Result<()> {
    if min > max {
        bail!("HUB_DB_MIN_CONNECTIONS must be <= HUB_DB_MAX_CONNECTIONS");
    }
    Ok(())
}

fn persist_interval_from_env() -> Result<Duration> {
    let n = parse_u64_ge1(
        "HUB_PERSIST_INTERVAL_MS",
        &required_env("HUB_PERSIST_INTERVAL_MS")?,
    )?;
    Ok(Duration::from_millis(n))
}

fn compact_after_from_env() -> Result<i64> {
    let n = parse_i64("HUB_COMPACT_AFTER", &required_env("HUB_COMPACT_AFTER")?)?;
    validate_compact_after(n)
}

fn db_max_connections_from_env() -> Result<u32> {
    parse_u32_ge1(
        "HUB_DB_MAX_CONNECTIONS",
        &required_env("HUB_DB_MAX_CONNECTIONS")?,
    )
}

fn db_min_connections_from_env() -> Result<u32> {
    parse_u32(
        "HUB_DB_MIN_CONNECTIONS",
        &required_env("HUB_DB_MIN_CONNECTIONS")?,
    )
}

fn db_work_mem_from_env() -> Result<String> {
    parse_work_mem("HUB_DB_WORK_MEM", &required_env("HUB_DB_WORK_MEM")?)
}

/// Normalize a Postgres memory size for `work_mem` (P5). Validated at start so
/// a typo names the var instead of failing every pool connection. The value is
/// still passed to `set_config` as a **bind parameter**, never interpolated —
/// `SET` takes no parameters, so this would otherwise be env into SQL.
///
/// A bare integer is rejected on purpose: Postgres would read `16` as 16 kB.
pub fn parse_work_mem(name: &str, raw: &str) -> Result<String> {
    let s = raw.trim();
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        bail!("{name} must be a size like 16MB; got {raw:?}");
    }
    let n: u64 = digits
        .parse()
        .with_context(|| format!("{name} must be a size like 16MB; got {raw:?}"))?;
    if n < 1 {
        bail!("{name} must be >= 1");
    }
    let unit = match s[digits.len()..].trim().to_ascii_lowercase().as_str() {
        "kb" => "kB",
        "mb" => "MB",
        "gb" => "GB",
        "" => bail!("{name} needs an explicit unit (kB, MB, GB); got {raw:?}"),
        other => bail!("{name} unit must be kB, MB or GB; got {other:?}"),
    };
    Ok(format!("{n}{unit}"))
}

fn db_acquire_timeout_from_env() -> Result<Duration> {
    let n = parse_u64_ge1(
        "HUB_DB_ACQUIRE_TIMEOUT_SECS",
        &required_env("HUB_DB_ACQUIRE_TIMEOUT_SECS")?,
    )?;
    Ok(Duration::from_secs(n))
}

/// Missing or blank is a startup error. Names the var. Does not dump other env.
pub fn required_env(name: &str) -> Result<String> {
    match env::var(name) {
        Err(_) => bail!("{name} is required"),
        Ok(s) => {
            let s = s.trim();
            if s.is_empty() {
                bail!("{name} is required");
            }
            Ok(s.to_string())
        }
    }
}

pub fn parse_u32(name: &str, s: &str) -> Result<u32> {
    s.parse().with_context(|| {
        let mut msg = String::from(name);
        msg.push_str(" must be a non-negative integer");
        msg
    })
}

pub fn parse_u32_ge1(name: &str, s: &str) -> Result<u32> {
    let n = parse_u32(name, s)?;
    if n < 1 {
        bail!("{name} must be >= 1");
    }
    Ok(n)
}

pub fn parse_u64_ge1(name: &str, s: &str) -> Result<u64> {
    let n: u64 = s.parse().with_context(|| {
        let mut msg = String::from(name);
        msg.push_str(" must be an integer >= 1");
        msg
    })?;
    if n < 1 {
        bail!("{name} must be >= 1");
    }
    Ok(n)
}

pub fn parse_i64(name: &str, s: &str) -> Result<i64> {
    s.parse().with_context(|| {
        let mut msg = String::from(name);
        msg.push_str(" must be an integer");
        msg
    })
}

fn default_owner() -> String {
    let host = env::var("HOSTNAME").unwrap_or_else(|_| "hub".into());
    host + "-" + &std::process::id().to_string()
}

/// `DATABASE_URL` wins if set and non-empty. Otherwise build from
/// `POSTGRES_HOST`, `POSTGRES_USER`, `POSTGRES_PASSWORD` (required),
/// plus optional `POSTGRES_PORT` (5432), `POSTGRES_DB` (`venus`), and
/// `POSTGRES_SSLMODE` ([`DEFAULT_PG_SSLMODE`]).
pub fn database_url_from_env() -> Result<String> {
    Ok(dsn_from_env()?.0)
}

fn dsn_from_env() -> Result<(String, String)> {
    if let Ok(url) = env::var("DATABASE_URL") {
        let url = url.trim().to_string();
        if !url.is_empty() {
            deny_sqlite(&url)?;
            let sslmode = sslmode_from_dsn(&url).unwrap_or("unset").to_string();
            return Ok((url, sslmode));
        }
    }
    let host = required("POSTGRES_HOST")?;
    let user = required("POSTGRES_USER")?;
    let password = required("POSTGRES_PASSWORD")?;
    let db = env::var("POSTGRES_DB")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "venus".into());
    let port = env::var("POSTGRES_PORT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "5432".into());
    let sslmode = sslmode_from_parts_env()?;
    let url = build_postgres_url(&user, &password, &host, &port, &db, sslmode);
    deny_sqlite(&url)?;
    Ok((url, sslmode.to_string()))
}

fn sslmode_from_parts_env() -> Result<&'static str> {
    match env::var("POSTGRES_SSLMODE") {
        Err(_) => Ok(DEFAULT_PG_SSLMODE),
        Ok(s) => {
            let s = s.trim();
            if s.is_empty() {
                Ok(DEFAULT_PG_SSLMODE)
            } else {
                parse_pg_sslmode(s)
            }
        }
    }
}

/// Accept a libpq `sslmode` token. Rejects unknown values and query injection.
pub fn parse_pg_sslmode(raw: &str) -> Result<&'static str> {
    let lower = raw.trim().to_ascii_lowercase();
    for mode in PG_SSLMODES {
        if *mode == lower {
            return Ok(*mode);
        }
    }
    bail!(
        "POSTGRES_SSLMODE must be one of {}; got {raw:?}",
        PG_SSLMODES.join(", ")
    )
}

/// Query `sslmode` from a DSN. Not a rewrite — `DATABASE_URL` stays as given.
pub fn sslmode_from_dsn(url: &str) -> Option<&str> {
    let query = url.rsplit_once('?')?.1;
    for pair in query.split('&') {
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        if k.eq_ignore_ascii_case("sslmode") && !v.is_empty() {
            return Some(v);
        }
    }
    None
}

fn build_postgres_url(
    user: &str,
    password: &str,
    host: &str,
    port: &str,
    db: &str,
    sslmode: &str,
) -> String {
    let user_enc = utf8_percent_encode(user, NON_ALPHANUMERIC).to_string();
    let pass_enc = utf8_percent_encode(password, NON_ALPHANUMERIC).to_string();
    let mut url = String::from("postgres://");
    url.push_str(&user_enc);
    url.push(':');
    url.push_str(&pass_enc);
    url.push('@');
    url.push_str(host);
    url.push(':');
    url.push_str(port);
    url.push('/');
    url.push_str(db);
    url.push_str("?sslmode=");
    url.push_str(sslmode);
    url
}

fn required(name: &str) -> Result<String> {
    let v = env::var(name).with_context(|| {
        format!("{name} is required when DATABASE_URL is unset (no SQLite fallback)")
    })?;
    let v = v.trim().to_string();
    if v.is_empty() {
        bail!("{name} is empty");
    }
    Ok(v)
}

fn deny_sqlite(url: &str) -> Result<()> {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("sqlite") || lower.contains("://sqlite") {
        bail!("SQLite is not a product store; set Postgres POSTGRES_* or DATABASE_URL");
    }
    Ok(())
}

fn duration_secs_env(name: &str, default: u64) -> Result<Duration> {
    match optional_secs_env(name)? {
        Some(d) => Ok(d),
        None => Ok(Duration::from_secs(default)),
    }
}

fn optional_secs_env(name: &str) -> Result<Option<Duration>> {
    match env::var(name) {
        Err(_) => Ok(None),
        Ok(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Ok(None);
            }
            let n: u64 = s
                .parse()
                .with_context(|| format!("{name} must be a positive integer (seconds)"))?;
            if n == 0 {
                bail!("{name} must be > 0");
            }
            Ok(Some(Duration::from_secs(n)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_sqlite() {
        assert!(deny_sqlite("sqlite://foo").is_err());
        assert!(deny_sqlite("postgres://venus:venus@postgres:5432/venus").is_ok());
    }

    #[test]
    fn builds_dsn_from_parts() {
        let url = build_postgres_url(
            "u/ser",
            "p@ss:word",
            "db.example",
            "5432",
            "venus",
            DEFAULT_PG_SSLMODE,
        );
        assert!(url.contains("u%2Fser"));
        assert!(url.contains("p%40ss%3Aword"));
        assert!(url.starts_with("postgres://"));
        assert!(
            url.ends_with("?sslmode=disable"),
            "Compose default must stay disable: {url}"
        );
    }

    #[test]
    fn parse_pg_sslmode_allowlist() {
        assert_eq!(parse_pg_sslmode("require").unwrap(), "require");
        assert_eq!(parse_pg_sslmode("VERIFY-FULL").unwrap(), "verify-full");
        assert_eq!(parse_pg_sslmode(" disable ").unwrap(), "disable");
        assert!(parse_pg_sslmode("").is_err());
        assert!(parse_pg_sslmode("nope").is_err());
        assert!(
            parse_pg_sslmode("disable&host=evil").is_err(),
            "must not inject extra query params"
        );
    }

    #[test]
    fn build_postgres_url_uses_sslmode() {
        let url = build_postgres_url("venus", "venus", "postgres", "5432", "venus", "require");
        assert!(url.ends_with("?sslmode=require"), "{url}");
        assert!(!url.contains("sslmode=disable"));
    }

    #[test]
    fn sslmode_from_dsn_reads_query_and_leaves_url_alone() {
        assert_eq!(
            sslmode_from_dsn("postgres://venus:venus@postgres:5432/venus?sslmode=require"),
            Some("require")
        );
        assert_eq!(
            sslmode_from_dsn("postgres://venus:venus@postgres:5432/venus"),
            None
        );
        assert_eq!(
            sslmode_from_dsn("postgres://venus:venus@postgres:5432/venus?connect_timeout=5"),
            None
        );
    }

    #[test]
    fn heartbeat_interval_is_ttl_divided_by_three() {
        assert_eq!(
            heartbeat_interval_from_ttl(Duration::from_secs(20)),
            Duration::from_secs(6)
        );
        assert_eq!(
            heartbeat_interval_from_ttl(Duration::from_secs(30)),
            Duration::from_secs(10)
        );
    }

    #[test]
    fn lease_ratio_requires_three_beats_per_ttl() {
        validate_lease_ratio(Duration::from_secs(20), Duration::from_secs(6)).unwrap();
        validate_lease_ratio(Duration::from_secs(30), Duration::from_secs(10)).unwrap();
        assert!(validate_lease_ratio(Duration::from_secs(20), Duration::from_secs(10)).is_err());
        assert!(validate_lease_ratio(Duration::from_secs(20), Duration::from_secs(0)).is_err());
        assert!(validate_lease_ratio(Duration::from_secs(2), Duration::from_secs(1)).is_err());
    }

    #[test]
    fn compact_after_rejects_below_one() {
        assert_eq!(validate_compact_after(32).unwrap(), 32);
        assert_eq!(validate_compact_after(1).unwrap(), 1);
        assert!(validate_compact_after(0).is_err());
        assert!(validate_compact_after(-1).is_err());
    }

    #[test]
    fn pool_and_timer_env_parse() {
        assert_eq!(parse_u32_ge1("HUB_DB_MAX_CONNECTIONS", "32").unwrap(), 32);
        assert_eq!(parse_u32("HUB_DB_MIN_CONNECTIONS", "0").unwrap(), 0);
        assert_eq!(parse_u32("HUB_DB_MIN_CONNECTIONS", "4").unwrap(), 4);
        assert_eq!(
            parse_u64_ge1("HUB_DB_ACQUIRE_TIMEOUT_SECS", "10").unwrap(),
            10
        );
        assert_eq!(
            parse_u64_ge1("HUB_PERSIST_INTERVAL_MS", "1000").unwrap(),
            1000
        );
        assert_eq!(parse_i64("HUB_COMPACT_AFTER", "32").unwrap(), 32);
        validate_pool_sizes(32, 4).unwrap();
        validate_pool_sizes(32, 32).unwrap();
        validate_pool_sizes(32, 0).unwrap();
    }

    #[test]
    fn pool_and_timer_env_reject_zero_and_garbage() {
        let max0 = parse_u32_ge1("HUB_DB_MAX_CONNECTIONS", "0")
            .unwrap_err()
            .to_string();
        assert!(max0.contains("HUB_DB_MAX_CONNECTIONS"));
        assert!(max0.contains(">= 1"));
        let max_bad = parse_u32_ge1("HUB_DB_MAX_CONNECTIONS", "nope")
            .unwrap_err()
            .to_string();
        assert!(max_bad.contains("HUB_DB_MAX_CONNECTIONS"));
        assert!(parse_u64_ge1("HUB_DB_ACQUIRE_TIMEOUT_SECS", "0").is_err());
        assert!(parse_u64_ge1("HUB_PERSIST_INTERVAL_MS", "0").is_err());
        let persist_bad = parse_u64_ge1("HUB_PERSIST_INTERVAL_MS", "x")
            .unwrap_err()
            .to_string();
        assert!(persist_bad.contains("HUB_PERSIST_INTERVAL_MS"));
        assert!(validate_pool_sizes(4, 5).is_err());
        let sizes = validate_pool_sizes(4, 5).unwrap_err().to_string();
        assert!(sizes.contains("HUB_DB_MIN_CONNECTIONS"));
        assert!(sizes.contains("HUB_DB_MAX_CONNECTIONS"));
    }

    #[test]
    fn work_mem_normalizes_units_and_rejects_junk() {
        assert_eq!(parse_work_mem("HUB_DB_WORK_MEM", "16MB").unwrap(), "16MB");
        assert_eq!(parse_work_mem("HUB_DB_WORK_MEM", " 64mb ").unwrap(), "64MB");
        assert_eq!(
            parse_work_mem("HUB_DB_WORK_MEM", "4096kB").unwrap(),
            "4096kB"
        );
        assert_eq!(parse_work_mem("HUB_DB_WORK_MEM", "1GB").unwrap(), "1GB");
        // Postgres would read a bare integer as kB; make the operator say it.
        let bare = parse_work_mem("HUB_DB_WORK_MEM", "16")
            .unwrap_err()
            .to_string();
        assert!(bare.contains("HUB_DB_WORK_MEM"));
        assert!(bare.contains("unit"));
        assert!(parse_work_mem("HUB_DB_WORK_MEM", "").is_err());
        assert!(parse_work_mem("HUB_DB_WORK_MEM", "0MB").is_err());
        assert!(parse_work_mem("HUB_DB_WORK_MEM", "lots").is_err());
        assert!(parse_work_mem("HUB_DB_WORK_MEM", "16TB").is_err());
        // Bound, not interpolated — but a token like this must never parse.
        assert!(parse_work_mem("HUB_DB_WORK_MEM", "16MB'; DROP TABLE dirty --").is_err());
    }

    #[test]
    fn required_env_names_the_var() {
        match required_env("VENUS_HUB_TEST_UNSET_P17_VAR") {
            Ok(v) => panic!("unset var must fail, got {v}"),
            Err(e) => {
                let msg = e.to_string();
                assert!(msg.contains("VENUS_HUB_TEST_UNSET_P17_VAR"));
                assert!(msg.contains("required"));
            }
        }
    }

    #[test]
    fn cors_origins_empty_star_path_and_default() {
        assert!(parse_cors_origins("").unwrap().is_empty());
        assert!(parse_cors_origins("   ").unwrap().is_empty());
        let six = parse_cors_origins(
            "http://localhost:5173, http://127.0.0.1:5173, http://localhost:5174, http://127.0.0.1:5174, http://localhost:8080, http://127.0.0.1:8080",
        )
        .unwrap();
        assert_eq!(six, default_cors_origins());
        assert_eq!(
            parse_cors_origins("http://localhost:5173,http://localhost:5173").unwrap(),
            vec!["http://localhost:5173".to_string()]
        );
        let star = parse_cors_origins("*").unwrap_err().to_string();
        assert!(star.contains("HUB_CORS_ORIGINS"));
        assert!(parse_cors_origin("*").is_err());
        assert!(parse_cors_origin("http://localhost:5173/").is_err());
        assert!(parse_cors_origin("http://localhost:5173/api").is_err());
        assert!(parse_cors_origin("ftp://localhost:5173").is_err());
        assert!(parse_cors_origin("http://user@localhost").is_err());
        assert!(parse_cors_origin("http://localhost:5173?x=1").is_err());
        assert_eq!(
            parse_cors_origin("https://example.com").unwrap(),
            "https://example.com"
        );
    }
}
