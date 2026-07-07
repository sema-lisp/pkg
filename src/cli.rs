//! `sema-pkg admin …` — offline user administration against `DATABASE_URL`.
//!
//! Lets an operator create the first admin and manage roles without hand-editing
//! the database, and works on any backend. Invoked as a subcommand of the server
//! binary, before the HTTP server starts.

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::entity::user;
use crate::{auth, config::Config, dal, db};

const USAGE: &str = "\
Usage: sema-pkg admin <command>

  promote <username>                 grant the admin role
  demote  <username>                 revoke the admin role
  ban     <username>                 ban a user
  unban   <username>                 lift a ban
  create  <username> <email> <pw>    create a new admin user
  list                               list admins";

/// Run an `admin` subcommand and exit the process.
pub async fn run_admin(args: &[String]) -> ! {
    let config = Config::from_env();
    let db = db::connect(&config.database_url).await;

    let result = match args.first().map(String::as_str) {
        Some("promote") => set_role(&db, args.get(1), true).await,
        Some("demote") => set_role(&db, args.get(1), false).await,
        Some("ban") => set_ban(&db, args.get(1), true).await,
        Some("unban") => set_ban(&db, args.get(1), false).await,
        Some("create") => create_admin(&db, args.get(1), args.get(2), args.get(3)).await,
        Some("list") => list_admins(&db).await,
        _ => Err(USAGE.to_string()),
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

async fn user_id(db: &db::Db, username: &str) -> Result<i64, String> {
    dal::users::find_by_username(db, username)
        .await
        .ok()
        .flatten()
        .map(|u| u.id)
        .ok_or_else(|| format!("No such user: {username}"))
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
