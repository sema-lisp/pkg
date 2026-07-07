//! Concurrent endpoint latency load test.
//!
//! Exercises every HTTP endpoint — web pages, read APIs, admin views, and every
//! mutating action — against a running registry, reporting per-endpoint latency
//! percentiles. Reads are hammered concurrently to surface behaviour under load;
//! actions run a full create→mutate→delete lifecycle per worker on isolated
//! entities.
//!
//! The target server must run against a seeded database (the admin `helge` and
//! the dev password are expected). Point it with env:
//!
//!   BASE_URL=http://localhost:3000 \
//!   LOADTEST_CONCURRENCY=50 LOADTEST_DURATION=10 \
//!   LOADTEST_ACTION_WORKERS=20 LOADTEST_ACTION_ITERS=5 \
//!   cargo run --release --features loadtest --bin loadtest

use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use reqwest::{Client, StatusCode};
use serde_json::{json, Value};

const DEV_PASSWORD: &str = "123123123";

type Rec = HashMap<String, Vec<f64>>;

#[tokio::main]
async fn main() {
    let base = env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3000".into());
    let concurrency = env_usize("LOADTEST_CONCURRENCY", 50);
    let duration = env_usize("LOADTEST_DURATION", 10) as u64;
    let action_workers = env_usize("LOADTEST_ACTION_WORKERS", 20);
    let action_iters = env_usize("LOADTEST_ACTION_ITERS", 5);
    let run = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let client = Client::builder()
        .pool_max_idle_per_host(concurrency + action_workers)
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap();

    println!("=== sema-pkg load test ===");
    println!("Target:   {base}");
    println!("Reads:    {concurrency} workers × {duration}s");
    println!("Actions:  {action_workers} workers × {action_iters} iters");
    println!();

    // ── Fixtures ─────────────────────────────────────────────────────────
    let admin = login(&client, &base, "helge", DEV_PASSWORD)
        .await
        .expect("admin login failed — seed the DB (jake seed) and start the server first");
    let user = register(&client, &base, &format!("lt-read-{run}"))
        .await
        .expect("register read user");
    let token = create_token(&client, &base, &user)
        .await
        .expect("create read token")
        .0;
    let target = format!("ltread{run}");
    publish(&client, &base, &token, &target, "1.0.0").await;
    publish(&client, &base, &token, &target, "1.1.0").await;

    // ── Phase 1: concurrent read load ────────────────────────────────────
    let endpoints: Arc<Vec<(String, String, Option<String>)>> = Arc::new(vec![
        ep("GET /readyz", "/readyz", None),
        ep("GET /healthz", "/healthz", None),
        ep("GET / (home)", "/", None),
        ep("GET /search (web)", "/search?q=lib", None),
        ep(
            "GET /packages/{n} (web)",
            &format!("/packages/{target}"),
            None,
        ),
        ep("GET /login", "/login", None),
        ep("GET /account", "/account", Some(user.clone())),
        ep("GET /admin (web)", "/admin", Some(admin.clone())),
        ep(
            "GET api package",
            &format!("/api/v1/packages/{target}"),
            None,
        ),
        ep(
            "GET api downloads",
            &format!("/api/v1/packages/{target}/downloads"),
            None,
        ),
        ep(
            "GET api owners",
            &format!("/api/v1/packages/{target}/owners"),
            None,
        ),
        ep("GET api search", "/api/v1/search?q=lib", None),
        ep("GET api tokens", "/api/v1/tokens", Some(user.clone())),
        ep(
            "GET admin/stats",
            "/api/v1/admin/stats",
            Some(admin.clone()),
        ),
        ep(
            "GET admin/users",
            "/api/v1/admin/users",
            Some(admin.clone()),
        ),
        ep(
            "GET admin/packages",
            "/api/v1/admin/packages",
            Some(admin.clone()),
        ),
        ep(
            "GET admin/audit",
            "/api/v1/admin/audit",
            Some(admin.clone()),
        ),
        ep(
            "GET admin/reports",
            "/api/v1/admin/reports",
            Some(admin.clone()),
        ),
    ]);

    println!("Phase 1: read load ({} endpoints)…", endpoints.len());
    let deadline = Instant::now() + Duration::from_secs(duration);
    let mut handles = Vec::new();
    for w in 0..concurrency {
        let client = client.clone();
        let base = base.clone();
        let eps = endpoints.clone();
        handles.push(tokio::spawn(async move {
            let mut local = Rec::new();
            let mut errs: HashMap<String, usize> = HashMap::new();
            let mut i = w;
            while Instant::now() < deadline {
                let (label, url, cookie) = &eps[i % eps.len()];
                i += 1;
                let mut req = client.get(format!("{base}{url}"));
                if let Some(ck) = cookie {
                    req = req.header("cookie", format!("session={ck}"));
                }
                let t = Instant::now();
                let ok = match req.send().await {
                    Ok(r) => {
                        let s = r.status();
                        let _ = r.bytes().await;
                        s.is_success()
                    }
                    Err(_) => false,
                };
                local
                    .entry(label.clone())
                    .or_default()
                    .push(t.elapsed().as_secs_f64() * 1000.0);
                if !ok {
                    *errs.entry(label.clone()).or_default() += 1;
                }
            }
            (local, errs)
        }));
    }
    let mut reads = Rec::new();
    let mut read_errs: HashMap<String, usize> = HashMap::new();
    for h in handles {
        let (l, e) = h.await.unwrap();
        merge(&mut reads, l);
        for (k, v) in e {
            *read_errs.entry(k).or_default() += v;
        }
    }
    report("Read endpoints (concurrent)", &reads, &read_errs, duration);

    // ── Phase 2: action lifecycle latency ────────────────────────────────
    println!("\nPhase 2: action lifecycle ({action_workers} workers)…");
    let mut handles = Vec::new();
    for w in 0..action_workers {
        let client = client.clone();
        let base = base.clone();
        let admin = admin.clone();
        handles.push(tokio::spawn(async move {
            let mut rec = Rec::new();
            let mut errs: HashMap<String, usize> = HashMap::new();
            for it in 0..action_iters {
                action_cycle(
                    &client,
                    &base,
                    &admin,
                    &format!("{run}-{w}-{it}"),
                    &mut rec,
                    &mut errs,
                )
                .await;
            }
            (rec, errs)
        }));
    }
    let mut actions = Rec::new();
    let mut action_errs: HashMap<String, usize> = HashMap::new();
    for h in handles {
        let (l, e) = h.await.unwrap();
        merge(&mut actions, l);
        for (k, v) in e {
            *action_errs.entry(k).or_default() += v;
        }
    }
    report(
        "Action endpoints (concurrent lifecycle)",
        &actions,
        &action_errs,
        0,
    );
}

/// One full create→mutate→delete lifecycle, timing every mutating endpoint.
#[allow(clippy::too_many_arguments)]
async fn action_cycle(
    c: &Client,
    base: &str,
    admin: &str,
    nonce: &str,
    rec: &mut Rec,
    errs: &mut HashMap<String, usize>,
) {
    let uname = format!("lta-{nonce}");
    let pkg = format!("ltapkg{nonce}");

    // Account lifecycle.
    let sess = match timed_session(c, rec, errs, "POST register", "POST",
        &format!("{base}/api/v1/auth/register"), None, None,
        Some(json!({"username": uname, "email": format!("{uname}@ex.com"), "password": DEV_PASSWORD}))).await {
        Some(s) => s,
        None => return,
    };
    let _ = timed_session(
        c,
        rec,
        errs,
        "POST login",
        "POST",
        &format!("{base}/api/v1/auth/login"),
        None,
        None,
        Some(json!({"username": uname, "password": DEV_PASSWORD})),
    )
    .await;

    // Token create + revoke.
    let (token, token_id) = match token_create(c, rec, errs, base, &sess).await {
        Some(t) => t,
        None => return,
    };

    // Publish two versions.
    let (s, ms) = publish(c, base, &token, &pkg, "1.0.0").await;
    record(rec, errs, "PUT publish", ms, s.is_success());
    let (s, ms) = publish(c, base, &token, &pkg, "2.0.0").await;
    record(rec, errs, "PUT publish", ms, s.is_success());

    // Ownership: add helge as co-owner, then remove (bearer-token auth).
    call(
        c,
        rec,
        errs,
        "PUT owners.add",
        "PUT",
        &format!("{base}/api/v1/packages/{pkg}/owners"),
        None,
        Some(&token),
        Some(json!({"username": "helge"})),
    )
    .await;
    call(
        c,
        rec,
        errs,
        "DELETE owners.remove",
        "DELETE",
        &format!("{base}/api/v1/packages/{pkg}/owners"),
        None,
        Some(&token),
        Some(json!({"username": "helge"})),
    )
    .await;

    // Yank a version.
    call(
        c,
        rec,
        errs,
        "POST yank",
        "POST",
        &format!("{base}/api/v1/packages/{pkg}/1.0.0/yank"),
        None,
        Some(&token),
        None,
    )
    .await;

    // Submit a report on the package.
    call(c, rec, errs, "POST reports.submit", "POST", &format!("{base}/api/v1/reports"),
        Some(&sess), None, Some(json!({"target_type":"package","target_name":pkg,"report_type":"spam","reason":"loadtest report reason text"}))).await;

    // Revoke the token.
    call(
        c,
        rec,
        errs,
        "DELETE tokens.revoke",
        "DELETE",
        &format!("{base}/api/v1/tokens/{token_id}"),
        Some(&sess),
        None,
        None,
    )
    .await;

    // Admin actions on the created user.
    if let Some(uid) = find_user_id(c, base, admin, &uname).await {
        call(
            c,
            rec,
            errs,
            "PUT admin.set_role",
            "PUT",
            &format!("{base}/api/v1/admin/users/{uid}/role"),
            Some(admin),
            None,
            Some(json!({"is_admin": false})),
        )
        .await;
        call(
            c,
            rec,
            errs,
            "POST admin.ban",
            "POST",
            &format!("{base}/api/v1/admin/users/{uid}/ban"),
            Some(admin),
            None,
            Some(json!({"reason":"loadtest"})),
        )
        .await;
        call(
            c,
            rec,
            errs,
            "POST admin.unban",
            "POST",
            &format!("{base}/api/v1/admin/users/{uid}/unban"),
            Some(admin),
            None,
            None,
        )
        .await;
        call(
            c,
            rec,
            errs,
            "POST admin.revoke_tokens",
            "POST",
            &format!("{base}/api/v1/admin/users/{uid}/revoke-tokens"),
            Some(admin),
            None,
            None,
        )
        .await;
    }

    // Admin package actions.
    call(
        c,
        rec,
        errs,
        "POST admin.yank_all",
        "POST",
        &format!("{base}/api/v1/admin/packages/{pkg}/yank-all"),
        Some(admin),
        None,
        None,
    )
    .await;
    call(
        c,
        rec,
        errs,
        "POST admin.transfer",
        "POST",
        &format!("{base}/api/v1/admin/packages/{pkg}/transfer"),
        Some(admin),
        None,
        Some(json!({"to_username":"helge"})),
    )
    .await;

    // Resolve a report, then delete the package.
    if let Some(rid) = first_open_report(c, base, admin).await {
        call(
            c,
            rec,
            errs,
            "POST admin.action_report",
            "POST",
            &format!("{base}/api/v1/admin/reports/{rid}/action"),
            Some(admin),
            None,
            None,
        )
        .await;
    }
    call(
        c,
        rec,
        errs,
        "DELETE admin.remove_package",
        "DELETE",
        &format!("{base}/api/v1/admin/packages/{pkg}"),
        Some(admin),
        None,
        None,
    )
    .await;

    // Log out.
    call(
        c,
        rec,
        errs,
        "POST logout",
        "POST",
        &format!("{base}/api/v1/auth/logout"),
        Some(&sess),
        None,
        None,
    )
    .await;
}

// ── HTTP helpers ────────────────────────────────────────────────────────────

/// Timed JSON call; records latency + error. Returns the status.
#[allow(clippy::too_many_arguments)]
async fn call(
    c: &Client,
    rec: &mut Rec,
    errs: &mut HashMap<String, usize>,
    label: &str,
    method: &str,
    url: &str,
    cookie: Option<&str>,
    bearer: Option<&str>,
    body: Option<Value>,
) -> StatusCode {
    let mut req = match method {
        "POST" => c.post(url),
        "PUT" => c.put(url),
        "DELETE" => c.delete(url),
        _ => c.get(url),
    };
    if let Some(ck) = cookie {
        req = req.header("cookie", format!("session={ck}"));
    }
    if let Some(b) = bearer {
        req = req.header("authorization", format!("Bearer {b}"));
    }
    if let Some(j) = body {
        req = req.json(&j);
    }
    let t = Instant::now();
    let status = match req.send().await {
        Ok(r) => {
            let s = r.status();
            let _ = r.bytes().await;
            s
        }
        Err(_) => StatusCode::REQUEST_TIMEOUT,
    };
    record(
        rec,
        errs,
        label,
        t.elapsed().as_secs_f64() * 1000.0,
        status.is_success(),
    );
    status
}

/// Like [`call`] but returns the `session=` cookie from the response.
#[allow(clippy::too_many_arguments)]
async fn timed_session(
    c: &Client,
    rec: &mut Rec,
    errs: &mut HashMap<String, usize>,
    label: &str,
    method: &str,
    url: &str,
    cookie: Option<&str>,
    bearer: Option<&str>,
    body: Option<Value>,
) -> Option<String> {
    let mut req = match method {
        "POST" => c.post(url),
        _ => c.get(url),
    };
    if let Some(ck) = cookie {
        req = req.header("cookie", format!("session={ck}"));
    }
    if let Some(b) = bearer {
        req = req.header("authorization", format!("Bearer {b}"));
    }
    if let Some(j) = body {
        req = req.json(&j);
    }
    let t = Instant::now();
    let (status, session) = match req.send().await {
        Ok(r) => {
            let s = r.status();
            let sess = session_cookie(&r);
            let _ = r.bytes().await;
            (s, sess)
        }
        Err(_) => (StatusCode::REQUEST_TIMEOUT, None),
    };
    record(
        rec,
        errs,
        label,
        t.elapsed().as_secs_f64() * 1000.0,
        status.is_success(),
    );
    session
}

async fn token_create(
    c: &Client,
    rec: &mut Rec,
    errs: &mut HashMap<String, usize>,
    base: &str,
    sess: &str,
) -> Option<(String, i64)> {
    let t = Instant::now();
    let resp = c
        .post(format!("{base}/api/v1/tokens"))
        .header("cookie", format!("session={sess}"))
        .json(&json!({"name": "loadtest"}))
        .send()
        .await
        .ok()?;
    let ok = resp.status().is_success();
    let body: Value = resp.json().await.ok()?;
    record(
        rec,
        errs,
        "POST tokens.create",
        t.elapsed().as_secs_f64() * 1000.0,
        ok,
    );
    Some((
        body["token"].as_str()?.to_string(),
        body["id"].as_i64().unwrap_or(0),
    ))
}

async fn login(c: &Client, base: &str, u: &str, p: &str) -> Option<String> {
    let r = c
        .post(format!("{base}/api/v1/auth/login"))
        .json(&json!({"username": u, "password": p}))
        .send()
        .await
        .ok()?;
    if !r.status().is_success() {
        return None;
    }
    session_cookie(&r)
}

async fn register(c: &Client, base: &str, uname: &str) -> Option<String> {
    let r = c
        .post(format!("{base}/api/v1/auth/register"))
        .json(&json!({"username": uname, "email": format!("{uname}@ex.com"), "password": DEV_PASSWORD}))
        .send()
        .await
        .ok()?;
    if !r.status().is_success() {
        return None;
    }
    session_cookie(&r)
}

async fn create_token(c: &Client, base: &str, sess: &str) -> Option<(String, i64)> {
    let body: Value = c
        .post(format!("{base}/api/v1/tokens"))
        .header("cookie", format!("session={sess}"))
        .json(&json!({"name": "loadtest"}))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    Some((
        body["token"].as_str()?.to_string(),
        body["id"].as_i64().unwrap_or(0),
    ))
}

async fn publish(c: &Client, base: &str, token: &str, pkg: &str, ver: &str) -> (StatusCode, f64) {
    let boundary = "ltboundary";
    let meta = json!({"description": "loadtest package"}).to_string();
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"metadata\"\r\n\r\n{meta}\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"tarball\"; filename=\"p.tar.gz\"\r\nContent-Type: application/gzip\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(&gzip(format!("{pkg}-{ver}").as_bytes()));
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let t = Instant::now();
    let status = match c
        .put(format!("{base}/api/v1/packages/{pkg}/{ver}"))
        .header("authorization", format!("Bearer {token}"))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body)
        .send()
        .await
    {
        Ok(r) => {
            let s = r.status();
            let _ = r.bytes().await;
            s
        }
        Err(_) => StatusCode::REQUEST_TIMEOUT,
    };
    (status, t.elapsed().as_secs_f64() * 1000.0)
}

async fn find_user_id(c: &Client, base: &str, admin: &str, uname: &str) -> Option<i64> {
    let body: Value = c
        .get(format!("{base}/api/v1/admin/users?q={uname}"))
        .header("cookie", format!("session={admin}"))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    body["users"].as_array()?.first()?["id"].as_i64()
}

async fn first_open_report(c: &Client, base: &str, admin: &str) -> Option<i64> {
    let body: Value = c
        .get(format!("{base}/api/v1/admin/reports?status=open"))
        .header("cookie", format!("session={admin}"))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    body["reports"].as_array()?.first()?["id"].as_i64()
}

// ── Utilities ───────────────────────────────────────────────────────────────

fn record(rec: &mut Rec, errs: &mut HashMap<String, usize>, label: &str, ms: f64, ok: bool) {
    rec.entry(label.to_string()).or_default().push(ms);
    if !ok {
        *errs.entry(label.to_string()).or_default() += 1;
    }
}

fn ep(label: &str, url: &str, cookie: Option<String>) -> (String, String, Option<String>) {
    (label.to_string(), url.to_string(), cookie)
}

fn session_cookie(r: &reqwest::Response) -> Option<String> {
    r.headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|h| h.to_str().ok())
        .find_map(|c| {
            c.split(';')
                .next()
                .and_then(|kv| kv.trim().strip_prefix("session="))
                .map(|s| s.to_string())
        })
}

fn env_usize(k: &str, d: usize) -> usize {
    env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

fn merge(into: &mut Rec, from: Rec) {
    for (k, mut v) in from {
        into.entry(k).or_default().append(&mut v);
    }
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx]
}

fn report(title: &str, rec: &Rec, errs: &HashMap<String, usize>, duration: u64) {
    println!("\n── {title} ──");
    println!(
        "{:<28} {:>7} {:>8} {:>8} {:>8} {:>8} {:>8} {:>6}",
        "endpoint", "count", "p50ms", "p95ms", "p99ms", "maxms", "meanms", "err"
    );
    let mut rows: Vec<(&String, &Vec<f64>)> = rec.iter().collect();
    rows.sort_by(|a, b| {
        let mut a2 = a.1.clone();
        let mut b2 = b.1.clone();
        a2.sort_by(|x, y| x.partial_cmp(y).unwrap());
        b2.sort_by(|x, y| x.partial_cmp(y).unwrap());
        pct(&b2, 0.95).partial_cmp(&pct(&a2, 0.95)).unwrap()
    });
    let mut total = 0usize;
    for (label, vals) in rows {
        let mut s = vals.clone();
        s.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let mean = s.iter().sum::<f64>() / s.len().max(1) as f64;
        total += s.len();
        println!(
            "{:<28} {:>7} {:>8.1} {:>8.1} {:>8.1} {:>8.1} {:>8.1} {:>6}",
            label,
            s.len(),
            pct(&s, 0.50),
            pct(&s, 0.95),
            pct(&s, 0.99),
            s.last().copied().unwrap_or(0.0),
            mean,
            errs.get(label).copied().unwrap_or(0),
        );
    }
    if duration > 0 {
        println!(
            "Throughput: {:.0} req/s total",
            total as f64 / duration as f64
        );
    }
    let errsum: usize = errs.values().sum();
    println!("Total requests: {total}, errors: {errsum}");
}

/// Minimal valid gzip (stored deflate blocks) so publish's magic-byte check
/// passes and the blob is a real stream.
fn gzip(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff];
    let mut chunks = data.chunks(0xFFFF).peekable();
    loop {
        let chunk: &[u8] = chunks.next().unwrap_or(&[]);
        let is_last = chunks.peek().is_none();
        out.push(if is_last { 0x01 } else { 0x00 });
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
        if is_last {
            break;
        }
    }
    out.extend_from_slice(&crc32(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}
