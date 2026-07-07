//! `sema-pkg <command>` — offline operator tooling against `DATABASE_URL`.
//!
//! Runs management commands without the HTTP server or hand-editing the
//! database, and works on any backend. Invoked as a subcommand of the server
//! binary (see `main`).
//!
//!   admin <cmd>      user + token administration
//!   package <cmd>    package moderation
//!   stats            registry summary
//!   doctor           check DB, blob store, and config

use sea_orm::{ColumnTrait, ConnectionTrait, Database, EntityTrait, QueryFilter, QueryOrder};

use crate::blob::BlobStore;
use crate::entity::user;
use crate::{auth, config::Config, dal, db};

const TOP_USAGE: &str = "\
Usage: sema-pkg <command>

  admin <cmd>      user + token administration (try: sema-pkg admin)
  package <cmd>    package moderation (try: sema-pkg package)
  stats            registry summary
  doctor           check DB, blob store, and config";

const ADMIN_USAGE: &str = "\
Usage: sema-pkg admin <command>

  create  <username> <email> <pw>    create a new admin user
  promote <username>                 grant the admin role
  demote  <username>                 revoke the admin role
  ban     <username>                 ban a user
  unban   <username>                 lift a ban
  reset-password <username> <pw>     set a user's password
  revoke-tokens  <username>          revoke all of a user's API tokens
  list                               list admins";

const PACKAGE_USAGE: &str = "\
Usage: sema-pkg package <command>

  yank   <name> <version>            mark a version yanked
  remove <name>                      delete a package and all its versions";

/// Dispatch a CLI command and exit the process.
pub async fn run(args: &[String]) -> ! {
    let result = match args.first().map(String::as_str) {
        Some("admin") => run_admin(&args[1..]).await,
        Some("package") => run_package(&args[1..]).await,
        Some("stats") => run_stats().await,
        Some("doctor") => run_doctor().await,
        _ => Err(TOP_USAGE.to_string()),
    };
    match result {
        Ok(msg) => {
            println!("{msg}");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

async fn connect() -> db::Db {
    db::connect(&Config::from_env().database_url).await
}

async fn user_id(db: &db::Db, username: &str) -> Result<i64, String> {
    dal::users::find_by_username(db, username)
        .await
        .ok()
        .flatten()
        .map(|u| u.id)
        .ok_or_else(|| format!("No such user: {username}"))
}

// ── admin ───────────────────────────────────────────────────────────────────

async fn run_admin(args: &[String]) -> Result<String, String> {
    let sub = args.first().map(String::as_str);
    if sub.is_none() {
        return Err(ADMIN_USAGE.to_string());
    }
    let db = connect().await;
    match sub {
        Some("create") => create_admin(&db, args.get(1), args.get(2), args.get(3)).await,
        Some("promote") => set_role(&db, args.get(1), true).await,
        Some("demote") => set_role(&db, args.get(1), false).await,
        Some("ban") => set_ban(&db, args.get(1), true).await,
        Some("unban") => set_ban(&db, args.get(1), false).await,
        Some("reset-password") => reset_password(&db, args.get(1), args.get(2)).await,
        Some("revoke-tokens") => revoke_tokens(&db, args.get(1)).await,
        Some("list") => list_admins(&db).await,
        _ => Err(ADMIN_USAGE.to_string()),
    }
}

async fn set_role(db: &db::Db, username: Option<&String>, admin: bool) -> Result<String, String> {
    let name = username.ok_or("usage: sema-pkg admin promote|demote <username>")?;
    let id = user_id(db, name).await?;
    dal::users::set_admin(db, id, admin)
        .await
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "{name} is {} an admin",
        if admin { "now" } else { "no longer" }
    ))
}

async fn set_ban(db: &db::Db, username: Option<&String>, banned: bool) -> Result<String, String> {
    let name = username.ok_or("usage: sema-pkg admin ban|unban <username>")?;
    let id = user_id(db, name).await?;
    dal::users::set_banned(db, id, banned)
        .await
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "{name} is {}",
        if banned { "banned" } else { "unbanned" }
    ))
}

async fn reset_password(
    db: &db::Db,
    username: Option<&String>,
    password: Option<&String>,
) -> Result<String, String> {
    let (name, pw) = match (username, password) {
        (Some(n), Some(p)) => (n, p),
        _ => return Err("usage: sema-pkg admin reset-password <username> <password>".into()),
    };
    auth::validate_password(pw)?;
    let id = user_id(db, name).await?;
    dal::users::set_password(db, id, &auth::hash_password(pw))
        .await
        .map_err(|e| e.to_string())?;
    Ok(format!("Password reset for {name}"))
}

async fn revoke_tokens(db: &db::Db, username: Option<&String>) -> Result<String, String> {
    let name = username.ok_or("usage: sema-pkg admin revoke-tokens <username>")?;
    let id = user_id(db, name).await?;
    let n = dal::tokens::revoke_all_for_user(db, id).await;
    Ok(format!("Revoked {n} token(s) for {name}"))
}

async fn create_admin(
    db: &db::Db,
    username: Option<&String>,
    email: Option<&String>,
    password: Option<&String>,
) -> Result<String, String> {
    let (username, email, password) = match (username, email, password) {
        (Some(u), Some(e), Some(p)) => (u, e, p),
        _ => return Err("usage: sema-pkg admin create <username> <email> <password>".into()),
    };
    auth::validate_username(username)?;
    auth::validate_email(email)?;
    auth::validate_password(password)?;

    let hash = auth::hash_password(password);
    let model = dal::users::create(db, &username.to_lowercase(), &email.to_lowercase(), &hash)
        .await
        .map_err(|_| format!("Could not create user (username or email '{username}' taken?)"))?;
    dal::users::set_admin(db, model.id, true)
        .await
        .map_err(|e| e.to_string())?;
    Ok(format!("Created admin user '{}'", model.username))
}

async fn list_admins(db: &db::Db) -> Result<String, String> {
    let admins = user::Entity::find()
        .filter(user::Column::IsAdmin.eq(1))
        .order_by_asc(user::Column::Username)
        .all(db)
        .await
        .map_err(|e| e.to_string())?;
    if admins.is_empty() {
        return Ok("No admins.".into());
    }
    let mut out = format!("{} admin(s):", admins.len());
    for a in admins {
        out.push_str(&format!("\n  {} <{}>", a.username, a.email));
    }
    Ok(out)
}

// ── package ─────────────────────────────────────────────────────────────────

async fn run_package(args: &[String]) -> Result<String, String> {
    let sub = args.first().map(String::as_str);
    if sub.is_none() {
        return Err(PACKAGE_USAGE.to_string());
    }
    let db = connect().await;
    match sub {
        Some("yank") => yank(&db, args.get(1), args.get(2)).await,
        Some("remove") => remove(&db, args.get(1)).await,
        _ => Err(PACKAGE_USAGE.to_string()),
    }
}

async fn yank(
    db: &db::Db,
    name: Option<&String>,
    version: Option<&String>,
) -> Result<String, String> {
    let (name, version) = match (name, version) {
        (Some(n), Some(v)) => (n, v),
        _ => return Err("usage: sema-pkg package yank <name> <version>".into()),
    };
    let pkg = dal::packages::find_by_name(db, name)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("No such package: {name}"))?;
    match dal::versions::yank(db, pkg.id, version).await {
        Ok(0) => Err(format!("No such version: {name}@{version}")),
        Ok(_) => Ok(format!("Yanked {name}@{version}")),
        Err(e) => Err(e.to_string()),
    }
}

async fn remove(db: &db::Db, name: Option<&String>) -> Result<String, String> {
    let name = name.ok_or("usage: sema-pkg package remove <name>")?;
    let Some(orphaned) = dal::packages::delete_by_name(db, name).await else {
        return Err(format!("No such package: {name}"));
    };
    let blobs = BlobStore::from_config(&Config::from_env())?;
    let mut reclaimed = 0;
    for key in &orphaned {
        if blobs.delete(key).await.is_ok() {
            reclaimed += 1;
        }
    }
    Ok(format!(
        "Removed package {name} and all its versions ({reclaimed} blob(s) reclaimed)"
    ))
}

// ── stats / doctor ──────────────────────────────────────────────────────────

async fn run_stats() -> Result<String, String> {
    let db = connect().await;
    let s = dal::admin::stats(&db).await;
    Ok(format!(
        "Users:            {:>12}\nPackages:         {:>12}\n  banned users:   {:>12}\n  open reports:   {:>12}\nDownloads (30d):  {:>12}",
        s.total_users, s.total_packages, s.banned_users, s.open_reports, s.total_downloads
    ))
}

/// Verify the deployment can reach its dependencies. `Err` (non-zero exit) on
/// any failed check, so it works as a `fly ssh console -C "sema-pkg doctor"`
/// smoke test.
async fn run_doctor() -> Result<String, String> {
    let config = Config::from_env();
    let mut out = String::from("Config\n");
    out += &format!("  database:  {}\n", redact(&config.database_url));
    out += &format!("  base_url:  {}\n", config.base_url);
    out += &format!(
        "  blobs:     {}\n",
        config
            .blob_s3_bucket
            .as_deref()
            .map(|b| format!("s3://{b}"))
            .unwrap_or_else(|| config.blob_dir.clone())
    );
    out += &format!(
        "  github:    {}\n",
        if config.github_enabled() {
            "enabled"
        } else {
            "disabled"
        }
    );

    let checks: [(&str, Result<String, String>); 3] = [
        (
            "database",
            match Database::connect(&config.database_url).await {
                Ok(conn) => match conn.ping().await {
                    Ok(()) => Ok(format!("reachable ({:?})", conn.get_database_backend())),
                    Err(e) => Err(format!("ping failed: {e}")),
                },
                Err(e) => Err(format!("connect failed: {e}")),
            },
        ),
        (
            "blob store",
            match BlobStore::from_config(&config) {
                Ok(store) if config.blob_s3_bucket.is_some() => Ok(store.describe()),
                Ok(store) => match std::fs::create_dir_all(&config.blob_dir) {
                    Ok(()) => Ok(store.describe()),
                    Err(e) => Err(format!("blob dir not writable: {e}")),
                },
                Err(e) => Err(e),
            },
        ),
        (
            "secrets",
            config.check_production_secrets().map(|()| "ok".to_string()),
        ),
    ];

    out += "\nChecks";
    let mut healthy = true;
    for (name, result) in &checks {
        match result {
            Ok(detail) => out += &format!("\n  ✓ {name}: {detail}"),
            Err(detail) => {
                out += &format!("\n  ✗ {name}: {detail}");
                healthy = false;
            }
        }
    }
    if healthy {
        Ok(out)
    } else {
        Err(out)
    }
}

fn redact(url: &str) -> String {
    if let (Some(at), Some(scheme)) = (url.find('@'), url.find("://")) {
        return format!("{}://***@{}", &url[..scheme], &url[at + 1..]);
    }
    url.to_string()
}
