//! Engine-portable dev-data seeder for the Sema package registry.
//!
//! Writes through the app's own SeaORM entities, so it works against SQLite,
//! PostgreSQL, or MySQL from one `DATABASE_URL`. Seeded data is fully usable:
//! real Argon2 password hashes (every user logs in with the dev password), a
//! real printed API token, and real content-addressed blobs for the featured
//! packages.
//!
//! Bulk data is generated and inserted in streaming per-batch transactions, so
//! memory stays bounded at `--huge` scale (1M packages) — rows are never all
//! materialized at once.
//!
//! Usage:
//!   cargo run --features seed --bin seed              # small, realistic dev set
//!   cargo run --features seed --bin seed -- --large   # bulk stress data
//!   cargo run --features seed --bin seed -- --huge    # 20k users, 1M packages
//!   cargo run --features seed --bin seed -- --fresh   # wipe + recreate schema first
//!
//! Counts can be overridden per-run:
//!   SEED_USERS=50000 SEED_PACKAGES=2000000 cargo run --features seed --bin seed -- --fresh
//!
//! Configuration is read from the environment exactly like the server
//! (`DATABASE_URL`, `BLOB_DIR`, `BLOB_S3_*`). Point it at a **dev** database.

use std::collections::{HashMap, HashSet};

use fake::faker::internet::en::{SafeEmail, Username};
use fake::faker::lorem::en::{Sentence, Word};
use fake::Fake;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set, TransactionTrait,
};
use time::{Duration, OffsetDateTime};

use sema_pkg::auth::{generate_token, hash_password, hash_token};
use sema_pkg::blob::BlobStore;
use sema_pkg::config::Config;
use sema_pkg::entity::{
    api_token, audit_log, download_daily, owner, package, package_version, report, user,
};

// The dev password every seeded user shares — hashed once (Argon2 is slow) and
// reused, so all seeded accounts log in with it.
const DEV_PASSWORD: &str = "123123123";

// Insert batch size for a single insert_many. Stays under every engine's
// bound-parameter limit (SQLite ~32k, Postgres 65k) at ~10 columns per row.
const CHUNK: usize = 500;

// Packages generated per streaming transaction. Larger = fewer commits/fsyncs
// (faster) but more transient memory; ~4k keeps memory small and throughput high.
const PKG_BATCH: usize = 4000;

// Cap on packages retained in memory for the download/report pool. Bounds memory
// at huge scale; downloads concentrate on this (popular) subset, which is
// realistic anyway.
const POOL_CAP: usize = 5000;

/// A featured package: a stable, hand-written demo entry with a real README.
struct Featured {
    name: &'static str,
    topic: &'static str,
    description: &'static str,
    github: bool,
}

const FEATURED: &[Featured] = &[
    Featured {
        name: "sema-http",
        topic: "HTTP",
        description: "Ergonomic HTTP client and server with async streaming bodies",
        github: true,
    },
    Featured {
        name: "sema-json",
        topic: "JSON",
        description: "Fast, allocation-light JSON parser and serializer",
        github: true,
    },
    Featured {
        name: "sema-router",
        topic: "routing",
        description: "Composable, type-safe URL routing",
        github: false,
    },
    Featured {
        name: "sema-sql",
        topic: "SQL",
        description: "SQL query builder with a pooled async driver",
        github: true,
    },
    Featured {
        name: "sema-cli",
        topic: "CLI",
        description: "Declarative command-line argument parsing",
        github: false,
    },
    Featured {
        name: "sema-async",
        topic: "async",
        description: "Lightweight async runtime and task scheduler",
        github: true,
    },
    Featured {
        name: "sema-test",
        topic: "testing",
        description: "Property-based testing and rich assertions",
        github: false,
    },
    Featured {
        name: "sema-crypto",
        topic: "cryptography",
        description: "Hashing, HMAC, and AEAD primitives",
        github: true,
    },
    Featured {
        name: "sema-csv",
        topic: "CSV",
        description: "Streaming CSV reader and writer",
        github: false,
    },
    Featured {
        name: "sema-log",
        topic: "logging",
        description: "Structured, leveled, low-overhead logging",
        github: false,
    },
];

// Realistic library topics; bulk package names are drawn from these (plus fake
// words for the long tail) rather than a tiny combinatorial set.
const TOPICS: &[&str] = &[
    "http",
    "json",
    "toml",
    "yaml",
    "xml",
    "csv",
    "sql",
    "redis",
    "postgres",
    "sqlite",
    "mongo",
    "cache",
    "queue",
    "kafka",
    "grpc",
    "graphql",
    "websocket",
    "oauth",
    "jwt",
    "crypto",
    "hash",
    "uuid",
    "regex",
    "glob",
    "path",
    "fs",
    "net",
    "dns",
    "tls",
    "smtp",
    "ssh",
    "cli",
    "args",
    "config",
    "env",
    "log",
    "trace",
    "metrics",
    "retry",
    "backoff",
    "ratelimit",
    "pool",
    "actor",
    "stream",
    "channel",
    "time",
    "date",
    "money",
    "i18n",
    "unicode",
    "base64",
    "hex",
    "gzip",
    "zip",
    "tar",
    "image",
    "svg",
    "pdf",
    "markdown",
    "template",
    "html",
    "color",
    "math",
    "stats",
    "random",
    "geo",
    "graph",
    "tree",
    "trie",
    "bloom",
    "lru",
    "diff",
    "fuzzy",
    "search",
    "index",
    "parser",
    "lexer",
    "codec",
    "proto",
];

const QUALIFIERS: &[&str] = &[
    "fast", "tiny", "async", "mini", "turbo", "lite", "simple", "core", "pure", "zero",
];

const REPORT_TYPES: &[&str] = &["spam", "malware", "abuse", "other"];

const REPORT_REASONS: &[&str] = &[
    "Contains obfuscated code that phones home to an unknown server",
    "Typosquats a popular package name to trick installers",
    "Description is SEO spam with advertising links",
    "Bundles proprietary code in violation of its license",
    "Placeholder package reserving a name with no real code",
    "Installs a cryptocurrency miner on first run",
    "README contains phishing links",
    "Maintainer account appears compromised — sudden malicious release",
    "Ships a known-vulnerable dependency without disclosure",
    "Offensive package name and description",
];

struct Counts {
    users: usize,
    packages: usize,
    downloads: usize,
    reports: usize,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fresh = args.iter().any(|a| a == "--fresh");
    let has = |name: &str| args.iter().any(|a| a == name);

    let (label, mut counts) = if has("--huge") {
        (
            "huge (10M packages)",
            Counts {
                users: 50_000,
                packages: 10_000_000,
                downloads: 500_000_000,
                reports: 2_000,
            },
        )
    } else if has("--large") || has("--scale=large") {
        (
            "large (stress)",
            Counts {
                users: 1_000,
                packages: 500,
                downloads: 20_000,
                reports: 200,
            },
        )
    } else {
        (
            "small (dev)",
            Counts {
                users: 40,
                packages: 60,
                downloads: 4_000,
                reports: 30,
            },
        )
    };
    // Per-run overrides for probing where things break.
    env_override("SEED_USERS", &mut counts.users);
    env_override("SEED_PACKAGES", &mut counts.packages);
    env_override("SEED_DOWNLOADS", &mut counts.downloads);
    env_override("SEED_REPORTS", &mut counts.reports);

    let config = Config::from_env();
    println!("=== Sema Registry Seeder ===");
    println!("Database: {}", redact(&config.database_url));
    println!("Scale:    {label}");
    println!(
        "Target:   {} users, {} packages, {} downloads, {} reports",
        counts.users, counts.packages, counts.downloads, counts.reports
    );
    println!();

    // `db::connect` runs migrations; `--fresh` drops and recreates first.
    let db = sema_pkg::db::connect(&config.database_url).await;
    if fresh {
        use sea_orm_migration::MigratorTrait;
        println!("Wiping and recreating schema (--fresh)...");
        sema_pkg::migration::Migrator::fresh(&db)
            .await
            .expect("failed to reset schema");
    }
    let blobs = BlobStore::from_config(&config).expect("failed to init blob store");

    // For a large fresh load, drop the secondary indexes and the SQLite FTS
    // index and rebuild them once at the end — far cheaper than maintaining them
    // across every insert.
    let defer_indexes = fresh && counts.packages > 100_000;
    if defer_indexes {
        println!("Dropping secondary + search indexes for bulk load (rebuilt at the end)...");
        perf_indexes(&db, false).await;
        defer_fts(&db, false).await;
    }

    let mut rng = StdRng::seed_from_u64(42);
    let t0 = OffsetDateTime::now_utc();
    let dev_hash = hash_password(DEV_PASSWORD); // hash once, reuse everywhere

    // ── Users ────────────────────────────────────────────────────────────
    let featured_users: &[(&str, &str, bool, bool)] = &[
        // (username, email, is_admin, banned)
        ("helge", "helge@sema-lang.com", true, false),
        ("kari", "kari@example.com", false, false),
        ("magnus", "magnus@dev.no", false, false),
        ("ingrid", "ingrid@example.com", false, false),
        ("olav", "olav@example.com", false, false),
        ("spambot", "spam@bad.example", false, true),
    ];

    let mut users: Vec<user::ActiveModel> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut seen_emails: HashSet<String> = HashSet::new();

    for &(name, email, is_admin, banned) in featured_users {
        seen_names.insert(name.to_string());
        seen_emails.insert(email.to_string());
        users.push(user::ActiveModel {
            username: Set(name.into()),
            email: Set(email.into()),
            password_hash: Set(Some(dev_hash.clone())),
            github_id: Set(None),
            homepage: Set(None),
            is_admin: Set(i32::from(is_admin)),
            banned_at: Set(banned.then(|| ts_days_ago(&mut rng, 90, 30))),
            created_at: Set(ts_days_ago(&mut rng, 200, 150)),
            ..Default::default()
        });
    }

    let mut github_id_seq = 100_000i64;
    let mut user_seq: u64 = 0;
    while users.len() < counts.users.max(featured_users.len()) {
        // fake's Username/email namespace is finite once lowercased; append a
        // strictly-increasing suffix on collision so we can reach any count.
        let base: String = Username().fake_with_rng(&mut rng);
        let base = sanitize_username(&base);
        if base.len() < 2 {
            continue;
        }
        let username = if seen_names.contains(&base) {
            user_seq += 1;
            format!("{base}{user_seq}")
        } else {
            base
        };
        if !seen_names.insert(username.clone()) {
            continue;
        }
        let raw_email: String = SafeEmail().fake_with_rng(&mut rng);
        let email = if seen_emails.contains(&raw_email) {
            format!("{username}@seed.example")
        } else {
            raw_email
        };
        if !seen_emails.insert(email.clone()) {
            continue;
        }
        let github_only = rng.gen_bool(0.15);
        let banned = rng.gen_bool(0.02);
        let (password_hash, github_id) = if github_only {
            github_id_seq += 1;
            (None, Some(github_id_seq))
        } else {
            (Some(dev_hash.clone()), None)
        };
        users.push(user::ActiveModel {
            username: Set(username),
            email: Set(email),
            password_hash: Set(password_hash),
            github_id: Set(github_id),
            homepage: Set(None),
            is_admin: Set(0),
            banned_at: Set(banned.then(|| ts_days_ago(&mut rng, 120, 1))),
            created_at: Set(ts_days_ago(&mut rng, 200, 1)),
            ..Default::default()
        });
    }
    let user_count = users.len();
    insert_chunked::<user::Entity, _, _>(&db, users).await;
    use std::io::Write as _;

    let loaded: Vec<user::Model> = user::Entity::find().all(&db).await.expect("load users");
    let helge_id = loaded
        .iter()
        .find(|u| u.username == "helge")
        .map(|u| u.id)
        .unwrap();
    let all_ids: Vec<i64> = loaded.iter().map(|u| u.id).collect();
    let usernames: Vec<String> = loaded.iter().map(|u| u.username.clone()).collect();
    let uname_by_id: HashMap<i64, String> =
        loaded.iter().map(|u| (u.id, u.username.clone())).collect();
    println!("Users:     {user_count}  [{:.1}s]", elapsed(t0));
    std::io::stdout().flush().ok();

    // Register audit for every user.
    let register_audit: Vec<audit_log::ActiveModel> = usernames
        .iter()
        .map(|n| {
            audit_entry(
                n,
                "register",
                "user",
                n,
                None,
                ts_days_ago(&mut rng, 200, 1),
            )
        })
        .collect();
    insert_chunked::<audit_log::Entity, _, _>(&db, register_audit).await;

    // API token for helge (real, printed so you can curl the API).
    let raw_token = generate_token();
    api_token::ActiveModel {
        user_id: Set(helge_id),
        name: Set("dev-seed-token".into()),
        token_hash: Set(hash_token(&raw_token)),
        scopes: Set("publish".into()),
        created_at: Set(ts_days_ago(&mut rng, 150, 100)),
        ..Default::default()
    }
    .insert(&db)
    .await
    .expect("insert token");

    // ── Featured packages (real blobs, READMEs) ──────────────────────────
    // Retained pool: (package_name, [(version, yanked)]) used later to generate
    // realistic downloads/reports without holding all 1M packages in memory.
    let mut pool: Vec<(String, Vec<(String, bool)>)> = Vec::new();
    let mut publish_audit: Vec<audit_log::ActiveModel> = Vec::new();

    for f in FEATURED {
        let owner_id = if f.name == "sema-csv" {
            loaded
                .iter()
                .find(|u| u.username == "kari")
                .map(|u| u.id)
                .unwrap()
        } else {
            helge_id
        };
        let readme = featured_readme(f);
        let pkg = package::ActiveModel {
            name: Set(f.name.into()),
            description: Set(f.description.into()),
            repository_url: Set(f
                .github
                .then(|| format!("https://github.com/sema-lang/{}", f.name))),
            source: Set(if f.github {
                "github".into()
            } else {
                "upload".into()
            }),
            github_repo: Set(f.github.then(|| format!("sema-lang/{}", f.name))),
            webhook_secret: Set(None),
            readme_raw: Set(Some(readme.clone())),
            readme_html: Set(Some(comrak::markdown_to_html(
                &readme,
                &comrak::Options::default(),
            ))),
            created_at: Set(ts_days_ago(&mut rng, 180, 120)),
            ..Default::default()
        };
        let pid = pkg.insert(&db).await.expect("insert featured").id;
        owner::ActiveModel {
            package_id: Set(pid),
            user_id: Set(owner_id),
        }
        .insert(&db)
        .await
        .expect("insert featured owner");

        let n = rng.gen_range(2..=6);
        let mut vers = unique_versions(&mut rng, n);
        vers.sort_unstable();
        let mut retained = Vec::new();
        for (i, (maj, min, pat)) in vers.iter().enumerate() {
            let ver = format!("{maj}.{min}.{pat}");
            let bytes = format!("sema package {} v{ver}\n", f.name).into_bytes();
            let (blob_key, checksum, size) = blobs.store(&bytes).await.expect("store blob");
            let yanked = rng.gen_bool(0.08);
            let span = 150 - (i as i64 * 150 / n as i64);
            package_version::ActiveModel {
                package_id: Set(pid),
                version: Set(ver.clone()),
                checksum_sha256: Set(checksum),
                blob_key: Set(blob_key),
                size_bytes: Set(size as i64),
                yanked: Set(i32::from(yanked)),
                sema_version_req: Set(Some(format!(
                    ">={}",
                    ["0.6", "0.7", "0.8", "0.9", "1.0"]
                        .choose(&mut rng)
                        .unwrap()
                ))),
                tarball_url: Set(None),
                published_at: Set(ts_days_ago(&mut rng, span.max(1), (span - 20).max(0))),
                ..Default::default()
            }
            .insert(&db)
            .await
            .expect("insert featured version");
            publish_audit.push(audit_entry(
                &uname_by_id[&owner_id],
                "publish",
                "package_version",
                &format!("{}@{ver}", f.name),
                None,
                ts_days_ago(&mut rng, 150, 1),
            ));
            retained.push((ver, yanked));
        }
        pool.push((f.name.to_string(), retained));
    }

    // ── Bulk packages (streamed in per-batch transactions) ───────────────
    let bulk_target = counts.packages.saturating_sub(FEATURED.len());
    // The realistic name space is finite (~topics × qualifiers); a
    // strictly-increasing suffix keeps names unique past it. Below a few million
    // packages we track seen names for prettier (unsuffixed) names where
    // possible; beyond that we always suffix and skip the set so its memory
    // stays flat at scale.
    let track_names = counts.packages <= 2_000_000;
    let mut seen_pkg: HashSet<String> = if track_names {
        FEATURED.iter().map(|f| f.name.to_string()).collect()
    } else {
        HashSet::new()
    };
    let mut spam_pkgs: Vec<String> = Vec::new();
    let mut total_versions: u64 = 0;
    let mut total_dl: u64 = 0;
    let mut total_dl_rows: u64 = 0;
    let mut made = 0usize;
    let mut last_report = 0usize;
    let mut name_seq: u64 = 0;

    // Download distribution: concentrate on a fraction of packages at huge scale
    // (most real packages get ~none), spreading `counts.downloads` total across
    // them; give every package downloads at small scale for a fuller demo.
    let dl_prob = if counts.packages > 100_000 { 0.08 } else { 1.0 };
    let dl_mean = counts.downloads as f64 / (counts.packages as f64 * dl_prob).max(1.0);

    while made < bulk_target {
        let batch_n = PKG_BATCH.min(bulk_target - made);
        let txn = db.begin().await.expect("begin batch txn");

        // 1. Package rows for this batch.
        let mut names: Vec<String> = Vec::with_capacity(batch_n);
        let mut pkgs: Vec<package::ActiveModel> = Vec::with_capacity(batch_n);
        let mut owner_of: HashMap<String, i64> = HashMap::with_capacity(batch_n);
        while names.len() < batch_n {
            let base = gen_pkg_name(&mut rng);
            let name = if !track_names || seen_pkg.contains(&base) {
                name_seq += 1;
                format!("{base}-{name_seq}")
            } else {
                base
            };
            if track_names && !seen_pkg.insert(name.clone()) {
                continue;
            }
            let owner_id = *all_ids.choose(&mut rng).unwrap();
            let github = rng.gen_bool(0.2);
            let topic = name
                .trim_start_matches("sema-")
                .split('-')
                .next_back()
                .unwrap_or("core");
            let sentence: String = Sentence(4..12).fake_with_rng(&mut rng);
            let desc = format!(
                "{} library for Sema — {}",
                capitalize(topic),
                lower_first(&sentence)
            );
            pkgs.push(package::ActiveModel {
                name: Set(name.clone()),
                description: Set(desc),
                repository_url: Set(github.then(|| format!("https://github.com/sema-lang/{name}"))),
                source: Set(if github {
                    "github".into()
                } else {
                    "upload".into()
                }),
                github_repo: Set(github.then(|| format!("sema-lang/{name}"))),
                webhook_secret: Set(None),
                readme_raw: Set(None),
                readme_html: Set(None),
                created_at: Set(ts_days_ago(&mut rng, 170, 20)),
                ..Default::default()
            });
            if rng.gen_bool(0.02) {
                spam_pkgs.push(name.clone());
            }
            owner_of.insert(name.clone(), owner_id);
            names.push(name);
        }
        insert_chunked::<package::Entity, _, _>(&txn, pkgs).await;

        // 2. Fetch ids for this batch (reads its own writes inside the txn).
        let id_of: HashMap<String, i64> = package::Entity::find()
            .filter(package::Column::Name.is_in(names.iter().cloned()))
            .all(&txn)
            .await
            .expect("load batch ids")
            .into_iter()
            .map(|p| (p.name, p.id))
            .collect();

        // 3. Versions + owners + downloads for the batch.
        let mut versions: Vec<package_version::ActiveModel> = Vec::new();
        let mut owners: Vec<owner::ActiveModel> = Vec::new();
        let mut downloads: Vec<download_daily::ActiveModel> = Vec::new();
        for name in &names {
            let pid = id_of[name];
            let owner_id = owner_of[name];
            owners.push(owner::ActiveModel {
                package_id: Set(pid),
                user_id: Set(owner_id),
            });
            if rng.gen_bool(0.25) {
                let co = *all_ids.choose(&mut rng).unwrap();
                if co != owner_id {
                    owners.push(owner::ActiveModel {
                        package_id: Set(pid),
                        user_id: Set(co),
                    });
                }
            }
            let n = rng.gen_range(1..=8);
            let mut vers = unique_versions(&mut rng, n);
            vers.sort_unstable();
            let mut ver_strings: Vec<String> = Vec::with_capacity(n);
            let mut retained = Vec::new();
            for (i, (maj, min, pat)) in vers.iter().enumerate() {
                let ver = format!("{maj}.{min}.{pat}");
                let (blob_key, checksum, _) = synthetic_key(name, &ver);
                let yanked = rng.gen_bool(0.08);
                let span = 150 - (i as i64 * 150 / n as i64);
                versions.push(package_version::ActiveModel {
                    package_id: Set(pid),
                    version: Set(ver.clone()),
                    checksum_sha256: Set(checksum),
                    blob_key: Set(blob_key),
                    size_bytes: Set(rng.gen_range(1_000..500_000)),
                    yanked: Set(i32::from(yanked)),
                    sema_version_req: Set(Some(format!(
                        ">={}",
                        ["0.6", "0.7", "0.8", "0.9", "1.0"]
                            .choose(&mut rng)
                            .unwrap()
                    ))),
                    tarball_url: Set(None),
                    published_at: Set(ts_days_ago(&mut rng, span.max(1), (span - 20).max(0))),
                    ..Default::default()
                });
                if pool.len() < POOL_CAP {
                    retained.push((ver.clone(), yanked));
                }
                ver_strings.push(ver);
            }
            total_versions += vers.len() as u64;
            if pool.len() < POOL_CAP {
                pool.push((name.clone(), retained));
            }

            // Downloads for this package (aggregate rows, count column sums to
            // roughly the target across all packages).
            if rng.gen_bool(dl_prob) {
                let total = (exp_sample(&mut rng, dl_mean).round() as i64).max(1);
                // Spread across a few distinct (version, day) cells.
                let want_cells = rng.gen_range(1..=8).min(total as usize).max(1);
                let mut cells: HashMap<(String, String), i64> = HashMap::new();
                for _ in 0..want_cells * 3 {
                    if cells.len() >= want_cells {
                        break;
                    }
                    let ver = ver_strings[rng.gen_range(0..ver_strings.len())].clone();
                    let date = date_days_ago(&mut rng, 90, 0);
                    cells.entry((ver, date)).or_insert(0);
                }
                let ncells = cells.len().max(1) as i64;
                let base = total / ncells;
                let mut rem = total % ncells;
                for (ver, date) in cells.into_keys() {
                    let mut c = base;
                    if rem > 0 {
                        c += 1;
                        rem -= 1;
                    }
                    downloads.push(download_daily::ActiveModel {
                        package_name: Set(name.clone()),
                        version: Set(ver),
                        download_date: Set(date),
                        count: Set(c as i32),
                    });
                    total_dl += c as u64;
                    total_dl_rows += 1;
                }
            }
        }
        insert_chunked::<package_version::Entity, _, _>(&txn, versions).await;
        insert_chunked::<owner::Entity, _, _>(&txn, owners).await;
        insert_chunked::<download_daily::Entity, _, _>(&txn, downloads).await;

        txn.commit().await.expect("commit batch");
        made += batch_n;

        if made - last_report >= 250_000 || made == bulk_target {
            last_report = made;
            let secs = elapsed(t0);
            let rate = made as f64 / secs.max(0.001);
            println!(
                "  … {made}/{bulk_target} pkgs, {total_versions} versions, {total_dl} downloads  [{secs:.0}s, {rate:.0} pkg/s]"
            );
            std::io::stdout().flush().ok();
        }
    }
    println!(
        "Packages:  {}  ({} versions)  [{:.1}s]",
        made + FEATURED.len(),
        total_versions,
        elapsed(t0)
    );

    println!(
        "Downloads: {total_dl} across {total_dl_rows} daily rows  [{:.1}s]",
        elapsed(t0)
    );

    // ── Reports (against pooled packages + users) ────────────────────────
    let mut reports: Vec<report::ActiveModel> = Vec::new();
    for target in spam_pkgs.iter().take(6) {
        reports.push(make_report(&mut rng, &all_ids, "package", target, "open"));
    }
    reports.push(make_report(&mut rng, &all_ids, "user", "spambot", "open"));
    while reports.len() < counts.reports {
        let (ttype, tname) = if rng.gen_bool(0.8) {
            ("package", pool[rng.gen_range(0..pool.len())].0.clone())
        } else {
            ("user", usernames[rng.gen_range(0..usernames.len())].clone())
        };
        let status = match rng.gen_range(0..10) {
            0 => "actioned",
            1 => "dismissed",
            _ => "open",
        };
        reports.push(make_report(&mut rng, &all_ids, ttype, &tname, status));
    }
    let report_count = reports.len();
    insert_chunked::<report::Entity, _, _>(&db, reports).await;
    println!("Reports:   {report_count}  [{:.1}s]", elapsed(t0));

    // ── Audit log: publish events for the pool + yanks ───────────────────
    for (name, vers) in &pool {
        for (ver, yanked) in vers {
            if *yanked {
                publish_audit.push(audit_entry(
                    "moderation",
                    "yank",
                    "package_version",
                    &format!("{name}@{ver}"),
                    None,
                    ts_days_ago(&mut rng, 60, 1),
                ));
            }
        }
    }
    let audit_count = publish_audit.len();
    insert_chunked::<audit_log::Entity, _, _>(&db, publish_audit).await;
    println!(
        "Audit log: {} register + {audit_count} publish/yank  [{:.1}s]",
        user_count,
        elapsed(t0)
    );

    if defer_indexes {
        println!("Rebuilding secondary + search indexes...");
        std::io::stdout().flush().ok();
        perf_indexes(&db, true).await;
        defer_fts(&db, true).await;
        println!("Indexes rebuilt  [{:.1}s]", elapsed(t0));
    }

    println!();
    println!("=== Seed complete in {:.1}s ===", elapsed(t0));
    println!();
    println!("Admin login:  helge / {DEV_PASSWORD}");
    println!("API token:    {raw_token}");
    println!(
        "  e.g. curl -H \"Authorization: Bearer {raw_token}\" {}/api/v1/search?q=http",
        config.base_url
    );
}

// ── Insert helper ─────────────────────────────────────────────────────────

// Secondary indexes (mirrors migrations m007 + m009) dropped before a bulk load
// and rebuilt after — a single sorted build is far cheaper than maintaining them
// incrementally across tens of millions of inserts.
const PERF_INDEXES: &[(&str, &str, &[&str])] = &[
    ("idx_owners_user", "owners", &["user_id"]),
    ("idx_users_created", "users", &["created_at"]),
    ("idx_packages_created", "packages", &["created_at"]),
    (
        "idx_versions_published",
        "package_versions",
        &["published_at"],
    ),
    (
        "idx_download_daily_date_count",
        "download_daily",
        &["download_date", "count"],
    ),
];

/// Drop (`create = false`) or rebuild (`create = true`) the perf indexes, using
/// the schema manager so the DDL is portable across engines.
async fn perf_indexes(db: &sema_pkg::db::Db, create: bool) {
    use sea_orm_migration::prelude::*;
    let manager = SchemaManager::new(db);
    for (name, table, cols) in PERF_INDEXES {
        if create {
            let mut idx = Index::create();
            idx.if_not_exists().name(*name).table(Alias::new(*table));
            for col in *cols {
                idx.col(Alias::new(*col));
            }
            let _ = manager.create_index(idx.to_owned()).await;
        } else {
            let _ = manager
                .drop_index(
                    Index::drop()
                        .if_exists()
                        .name(*name)
                        .table(Alias::new(*table))
                        .to_owned(),
                )
                .await;
        }
    }
}

/// Drop (`rebuild = false`) or rebuild (`rebuild = true`) the SQLite FTS index
/// by running migration m008 down/up. No-op on other backends. Rebuilding
/// repopulates the FTS index from `packages` in one bulk pass, far cheaper than
/// the per-row triggers firing across a bulk load.
async fn defer_fts(db: &sema_pkg::db::Db, rebuild: bool) {
    use sea_orm::ConnectionTrait;
    if db.get_database_backend() != sea_orm::DatabaseBackend::Sqlite {
        return;
    }
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    let manager = SchemaManager::new(db);
    let migration = sema_pkg::migration::m008_search::Migration;
    let _ = if rebuild {
        migration.up(&manager).await
    } else {
        migration.down(&manager).await
    };
}

/// Insert active models in chunks. Works against a connection or a transaction.
/// Skips empty input (SeaORM's `insert_many` errors on an empty set).
async fn insert_chunked<E, A, C>(conn: &C, models: Vec<A>)
where
    E: EntityTrait,
    A: ActiveModelTrait<Entity = E> + Send,
    C: ConnectionTrait,
{
    for chunk in models.chunks(CHUNK) {
        if chunk.is_empty() {
            continue;
        }
        E::insert_many(chunk.to_vec())
            .exec(conn)
            .await
            .expect("insert_many failed");
    }
}

// ── Data helpers ──────────────────────────────────────────────────────────

fn env_override(key: &str, slot: &mut usize) {
    if let Ok(v) = std::env::var(key) {
        if let Ok(n) = v.parse::<usize>() {
            *slot = n;
        }
    }
}

/// Sample from an exponential distribution with the given mean — a light,
/// heavy-tailed spread so a few packages are far more popular than the rest.
fn exp_sample(rng: &mut StdRng, mean: f64) -> f64 {
    let u: f64 = rng.gen_range(f64::MIN_POSITIVE..1.0);
    -mean * u.ln()
}

fn unique_versions(rng: &mut StdRng, n: usize) -> Vec<(u32, u32, u32)> {
    let mut vers: Vec<(u32, u32, u32)> = Vec::with_capacity(n);
    let mut guard = 0;
    while vers.len() < n && guard < n * 10 {
        let v = (
            rng.gen_range(0..=3),
            rng.gen_range(0..=15),
            rng.gen_range(0..=20),
        );
        if !vers.contains(&v) {
            vers.push(v);
        }
        guard += 1;
    }
    vers
}

fn make_report(
    rng: &mut StdRng,
    user_ids: &[i64],
    target_type: &str,
    target_name: &str,
    status: &str,
) -> report::ActiveModel {
    let resolved = status != "open";
    report::ActiveModel {
        reporter_id: Set(*user_ids.choose(rng).unwrap()),
        target_type: Set(target_type.into()),
        target_name: Set(target_name.into()),
        report_type: Set((*REPORT_TYPES.choose(rng).unwrap()).into()),
        reason: Set((*REPORT_REASONS.choose(rng).unwrap()).into()),
        status: Set(status.into()),
        resolved_by: Set(resolved.then(|| *user_ids.choose(rng).unwrap())),
        resolved_at: Set(resolved.then(|| ts_days_ago(rng, 30, 0))),
        created_at: Set(ts_days_ago(rng, 90, 0)),
        ..Default::default()
    }
}

fn audit_entry(
    actor: &str,
    action: &str,
    target_type: &str,
    target_name: &str,
    detail: Option<String>,
    created_at: String,
) -> audit_log::ActiveModel {
    audit_log::ActiveModel {
        actor: Set(actor.into()),
        action: Set(action.into()),
        target_type: Set(Some(target_type.into())),
        target_name: Set(Some(target_name.into())),
        detail: Set(detail),
        created_at: Set(created_at),
        ..Default::default()
    }
}

/// Content-addressed key without writing a blob — for bulk packages whose
/// tarballs are never actually downloaded in dev.
fn synthetic_key(name: &str, ver: &str) -> (String, String, usize) {
    use sha2::{Digest, Sha256};
    let hex = format!("{:x}", Sha256::digest(format!("{name}@{ver}").as_bytes()));
    (format!("{hex}.tar.gz"), hex, 0)
}

fn gen_pkg_name(rng: &mut StdRng) -> String {
    match rng.gen_range(0..10) {
        0..=5 => {
            let t = TOPICS.choose(rng).unwrap();
            format!("sema-{t}")
        }
        6..=8 => {
            let q = QUALIFIERS.choose(rng).unwrap();
            let t = TOPICS.choose(rng).unwrap();
            format!("sema-{q}-{t}")
        }
        _ => {
            let w: String = Word().fake_with_rng(rng);
            let t = TOPICS.choose(rng).unwrap();
            format!("sema-{w}-{t}")
        }
    }
}

fn featured_readme(f: &Featured) -> String {
    format!(
        "# {name}\n\n{desc}.\n\n## Install\n\n```\nsema add {name}\n```\n\n## Usage\n\n```sema\n(import \"{name}\")\n```\n\n{name} provides {topic} building blocks for Sema applications.\n",
        name = f.name,
        desc = f.description,
        topic = f.topic,
    )
}

// ── String / time utilities ────────────────────────────────────────────────

fn sanitize_username(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>()
        .to_lowercase()
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn redact(url: &str) -> String {
    // Hide credentials in a postgres://user:pass@host URL when printing.
    if let Some(at) = url.find('@') {
        if let Some(scheme_end) = url.find("://") {
            return format!("{}://***@{}", &url[..scheme_end], &url[at + 1..]);
        }
    }
    url.to_string()
}

fn elapsed(t0: OffsetDateTime) -> f64 {
    (OffsetDateTime::now_utc() - t0).as_seconds_f64()
}

fn ts_days_ago(rng: &mut StdRng, max_days: i64, min_days: i64) -> String {
    let secs = rng.gen_range(min_days.max(0) * 86400..=max_days.max(min_days + 1) * 86400);
    let t = OffsetDateTime::now_utc() - Duration::seconds(secs);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        t.year(),
        t.month() as u8,
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    )
}

fn date_days_ago(rng: &mut StdRng, max_days: i64, min_days: i64) -> String {
    let days = rng.gen_range(min_days..=max_days.max(min_days + 1));
    let t = OffsetDateTime::now_utc() - Duration::days(days);
    format!("{:04}-{:02}-{:02}", t.year(), t.month() as u8, t.day())
}
