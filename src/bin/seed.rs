//! Engine-portable dev-data seeder for the Sema package registry.
//!
//! Replaces the old `seed.sh` (slow, API-driven, SQLite-adjacent) and
//! `seed_stress.py` (SQLite-only, produced un-loginable users). This tool talks
//! to the database through the app's own SeaORM entities, so it works against
//! SQLite, PostgreSQL, or MySQL from one `DATABASE_URL`, inserts in batched
//! transactions, and produces data that actually works: real Argon2 password
//! hashes (every seeded user logs in with the dev password), a real printed API
//! token, and real content-addressed blobs for the featured packages.
//!
//! Usage:
//!   cargo run --features seed --bin seed              # small, realistic dev set
//!   cargo run --features seed --bin seed -- --large   # bulk stress data
//!   cargo run --features seed --bin seed -- --fresh   # wipe + recreate schema first
//!
//! Configuration is read from the environment exactly like the server
//! (`DATABASE_URL`, `BLOB_DIR`, `BLOB_S3_*`). Point it at a **dev** database.

use std::collections::HashSet;

use fake::faker::internet::en::{SafeEmail, Username};
use fake::faker::lorem::en::{Sentence, Word};
use fake::Fake;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
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

// Insert batch size. Small enough to stay under every engine's bound-parameter
// limit (SQLite ~32k, Postgres 65k) at ~10 columns per row.
const CHUNK: usize = 500;

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

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let large = args.iter().any(|a| a == "--large" || a == "--scale=large");
    let fresh = args.iter().any(|a| a == "--fresh");

    let (bulk_users, bulk_packages, num_downloads, num_reports) = if large {
        (1000usize, 500usize, 20_000usize, 200usize)
    } else {
        (40, 60, 4_000, 30)
    };

    let config = Config::from_env();
    println!("=== Sema Registry Seeder ===");
    println!("Database: {}", redact(&config.database_url));
    println!(
        "Scale:    {}",
        if large {
            "large (stress)"
        } else {
            "small (dev)"
        }
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

    let mut rng = StdRng::seed_from_u64(42);
    let t0 = OffsetDateTime::now_utc();
    let dev_hash = hash_password(DEV_PASSWORD); // hash once, reuse everywhere

    // ── Users ────────────────────────────────────────────────────────────
    // Featured accounts drive the stable demo narrative.
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

    // Bulk users: realistic fake identities. ~15% are GitHub-only (no password).
    let mut github_id_seq = 100_000i64;
    while users.len() < featured_users.len() + bulk_users {
        let username: String = Username().fake_with_rng(&mut rng);
        let username = sanitize_username(&username);
        if username.len() < 2 || !seen_names.insert(username.clone()) {
            continue;
        }
        let email: String = SafeEmail().fake_with_rng(&mut rng);
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
    insert_chunked::<user::Entity, _>(&db, users).await;

    // Map username -> id (portable; avoids relying on returned ids).
    let user_ids: Vec<(i64, String, bool)> = user::Entity::find()
        .all(&db)
        .await
        .expect("load users")
        .into_iter()
        .map(|u| (u.id, u.username, u.password_hash.is_some()))
        .collect();
    let helge_id = user_ids
        .iter()
        .find(|(_, n, _)| n == "helge")
        .map(|(id, ..)| *id)
        .unwrap();
    let all_ids: Vec<i64> = user_ids.iter().map(|(id, ..)| *id).collect();
    // Users who can own/publish (have a login or are GitHub-linked — anyone, really).
    let owner_pool: Vec<i64> = all_ids.clone();
    println!("Users:     {user_count}");

    // ── API token for helge (real, printed so you can curl the API) ──────
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

    // ── Packages ─────────────────────────────────────────────────────────
    let mut packages: Vec<package::ActiveModel> = Vec::new();
    let mut pkg_specs: Vec<(String, i64, bool)> = Vec::new(); // (name, owner_id, is_featured)
    let mut seen_pkg: HashSet<String> = HashSet::new();

    for f in FEATURED {
        seen_pkg.insert(f.name.to_string());
        let owner_id = if f.name == "sema-csv" {
            user_ids
                .iter()
                .find(|(_, n, _)| n == "kari")
                .map(|(id, ..)| *id)
                .unwrap()
        } else {
            helge_id
        };
        let readme = featured_readme(f);
        let created = ts_days_ago(&mut rng, 180, 120);
        packages.push(package::ActiveModel {
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
            created_at: Set(created),
            ..Default::default()
        });
        pkg_specs.push((f.name.to_string(), owner_id, true));
    }

    // Bulk packages with realistic names + descriptions.
    let mut spam_pkgs: Vec<String> = Vec::new();
    while pkg_specs.len() < FEATURED.len() + bulk_packages {
        let name = gen_pkg_name(&mut rng);
        if !seen_pkg.insert(name.clone()) {
            continue;
        }
        let owner_id = *owner_pool.choose(&mut rng).unwrap();
        let github = rng.gen_bool(0.2);
        let topic = name
            .trim_start_matches("sema-")
            .split('-')
            .next_back()
            .unwrap_or("core");
        let desc = format!("{} library for Sema — {}", capitalize(topic), {
            let s: String = Sentence(4..12).fake_with_rng(&mut rng);
            lower_first(&s)
        });
        packages.push(package::ActiveModel {
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
        if rng.gen_bool(0.03) {
            spam_pkgs.push(name.clone());
        }
        pkg_specs.push((name, owner_id, false));
    }
    let pkg_count = pkg_specs.len();
    insert_chunked::<package::Entity, _>(&db, packages).await;

    let pkg_id: std::collections::HashMap<String, i64> = package::Entity::find()
        .all(&db)
        .await
        .expect("load packages")
        .into_iter()
        .map(|p| (p.name, p.id))
        .collect();
    println!("Packages:  {pkg_count}");

    // ── Owners ───────────────────────────────────────────────────────────
    let mut owners: HashSet<(i64, i64)> = HashSet::new();
    for (name, owner_id, _) in &pkg_specs {
        let pid = pkg_id[name];
        owners.insert((pid, *owner_id));
        // ~25% of packages have a co-owner.
        if rng.gen_bool(0.25) {
            let co = *owner_pool.choose(&mut rng).unwrap();
            if co != *owner_id {
                owners.insert((pid, co));
            }
        }
    }
    let owner_models: Vec<owner::ActiveModel> = owners
        .into_iter()
        .map(|(pid, uid)| owner::ActiveModel {
            package_id: Set(pid),
            user_id: Set(uid),
        })
        .collect();
    insert_chunked::<owner::Entity, _>(&db, owner_models).await;

    // ── Versions (real blobs for featured, synthetic keys for bulk) ──────
    let mut versions: Vec<package_version::ActiveModel> = Vec::new();
    let mut version_index: Vec<(String, String, bool, i64)> = Vec::new(); // name, ver, yanked, owner
    for (name, owner_id, featured) in &pkg_specs {
        let pid = pkg_id[name];
        let n = if *featured {
            rng.gen_range(2..=6)
        } else {
            rng.gen_range(1..=8)
        };
        let mut vers: Vec<(u32, u32, u32)> = Vec::new();
        while vers.len() < n {
            let v = (
                rng.gen_range(0..=3),
                rng.gen_range(0..=15),
                rng.gen_range(0..=20),
            );
            if !vers.contains(&v) {
                vers.push(v);
            }
        }
        vers.sort_unstable();
        for (i, (maj, min, pat)) in vers.iter().enumerate() {
            let ver = format!("{maj}.{min}.{pat}");
            let (blob_key, checksum, size) = if *featured {
                let bytes = format!("sema package {name} v{ver}\n").into_bytes();
                blobs.store(&bytes).await.expect("store blob")
            } else {
                synthetic_key(name, &ver)
            };
            let yanked = rng.gen_bool(0.08);
            // Later versions published later.
            let span = 150 - (i as i64 * 150 / n.max(1) as i64);
            versions.push(package_version::ActiveModel {
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
            });
            version_index.push((name.clone(), ver, yanked, *owner_id));
        }
    }
    let version_count = versions.len();
    insert_chunked::<package_version::Entity, _>(&db, versions).await;
    println!("Versions:  {version_count}");

    // ── Downloads (Zipf-ish: a few packages dominate) ────────────────────
    let pkg_names: Vec<String> = pkg_specs.iter().map(|(n, ..)| n.clone()).collect();
    let mut ver_by_pkg: std::collections::HashMap<&str, Vec<&str>> =
        std::collections::HashMap::new();
    for (name, ver, ..) in &version_index {
        ver_by_pkg
            .entry(name.as_str())
            .or_default()
            .push(ver.as_str());
    }
    let popular_n = (pkg_names.len() / 10).max(1);
    let mut dl_counts: std::collections::HashMap<(String, String, String), i32> =
        std::collections::HashMap::new();
    for _ in 0..num_downloads {
        let name = if rng.gen_bool(0.7) {
            &pkg_names[rng.gen_range(0..popular_n)]
        } else {
            &pkg_names[rng.gen_range(0..pkg_names.len())]
        };
        let Some(vers) = ver_by_pkg.get(name.as_str()) else {
            continue;
        };
        let ver = vers[rng.gen_range(0..vers.len())];
        let date = date_days_ago(&mut rng, 60, 0);
        *dl_counts
            .entry((name.clone(), ver.to_string(), date))
            .or_insert(0) += 1;
    }
    let downloads: Vec<download_daily::ActiveModel> = dl_counts
        .into_iter()
        .map(|((name, ver, date), count)| download_daily::ActiveModel {
            package_name: Set(name),
            version: Set(ver),
            download_date: Set(date),
            count: Set(count),
        })
        .collect();
    let dl_rows = downloads.len();
    insert_chunked::<download_daily::Entity, _>(&db, downloads).await;
    println!("Downloads: {dl_rows} daily rows ({num_downloads} events)");

    // ── Reports ──────────────────────────────────────────────────────────
    let usernames: Vec<String> = user_ids.iter().map(|(_, n, _)| n.clone()).collect();
    let mut reports: Vec<report::ActiveModel> = Vec::new();
    // Ensure spammy bulk packages + spambot get reported for a believable queue.
    for target in spam_pkgs.iter().take(6) {
        reports.push(make_report(&mut rng, &all_ids, "package", target, "open"));
    }
    reports.push(make_report(&mut rng, &all_ids, "user", "spambot", "open"));
    while reports.len() < num_reports {
        let (ttype, tname) = if rng.gen_bool(0.8) {
            (
                "package",
                pkg_names[rng.gen_range(0..pkg_names.len())].clone(),
            )
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
    insert_chunked::<report::Entity, _>(&db, reports).await;
    println!("Reports:   {report_count}");

    // ── Audit log ────────────────────────────────────────────────────────
    let uname_by_id: std::collections::HashMap<i64, String> =
        user_ids.iter().map(|(id, n, _)| (*id, n.clone())).collect();
    let mut audit: Vec<audit_log::ActiveModel> = Vec::new();
    for (_, name, _) in &user_ids {
        audit.push(audit_entry(
            name,
            "register",
            "user",
            name,
            None,
            ts_days_ago(&mut rng, 200, 1),
        ));
    }
    for (name, ver, yanked, owner_id) in &version_index {
        let actor = uname_by_id
            .get(owner_id)
            .cloned()
            .unwrap_or_else(|| "unknown".into());
        audit.push(audit_entry(
            &actor,
            "publish",
            "package_version",
            &format!("{name}@{ver}"),
            None,
            ts_days_ago(&mut rng, 150, 1),
        ));
        if *yanked {
            audit.push(audit_entry(
                &actor,
                "yank",
                "package_version",
                &format!("{name}@{ver}"),
                None,
                ts_days_ago(&mut rng, 60, 1),
            ));
        }
    }
    let audit_count = audit.len();
    insert_chunked::<audit_log::Entity, _>(&db, audit).await;
    println!("Audit log: {audit_count}");

    let secs = (OffsetDateTime::now_utc() - t0).as_seconds_f64();
    println!();
    println!("=== Seed complete in {secs:.1}s ===");
    println!();
    println!("Admin login:  helge / {DEV_PASSWORD}");
    println!("API token:    {raw_token}");
    println!(
        "  e.g. curl -H \"Authorization: Bearer {raw_token}\" {}/api/v1/search?q=http",
        config.base_url
    );
}

// ── Insert helper ─────────────────────────────────────────────────────────

/// Insert active models in chunks within one transaction per chunk. Skips empty
/// input (SeaORM's `insert_many` errors on an empty set).
async fn insert_chunked<E, A>(db: &sema_pkg::db::Db, models: Vec<A>)
where
    E: EntityTrait,
    A: sea_orm::ActiveModelTrait<Entity = E> + Send,
{
    for chunk in models.chunks(CHUNK) {
        if chunk.is_empty() {
            continue;
        }
        E::insert_many(chunk.to_vec())
            .exec(db)
            .await
            .expect("insert_many failed");
    }
}

// ── Data helpers ──────────────────────────────────────────────────────────

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
