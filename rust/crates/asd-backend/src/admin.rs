//! owner: WP-B.2 (bootstrap follow-up)
//!
//! Operator commands for a fresh deployment, run as `asd-backend admin ...`:
//!
//! - `create-org --name <name> --kind home|clinic|research [--retention-months N] [--force]`
//! - `create-user --email <email> --role owner|superadmin [--org <uuid>] [--force]`
//!
//! Each prints only the new id on stdout, so scripts can capture it. The password for
//! `create-user` comes from [`PASSWORD_ENV`] or the first line of stdin, never from argv
//! (argv is visible in `ps` and shell history). Hashing is Argon2id via
//! [`crate::auth::password`], with the API's password policy.
//!
//! Safety rules: every write appends one `audit_log` row (actor `system`, `via: cli`) in
//! the same transaction; an existing e-mail is an error and changes nothing (also with
//! `--force`); a second superadmin, or a second org with the same name, needs `--force`.
//! Other org roles are created by an owner through `POST /v1/orgs/{id}/users`.
//!
//! Exit codes: 0 success, 2 usage or invalid input, 3 conflict with existing data,
//! 1 anything else (database).

use std::io::BufRead;

use asd_contract::api_v1::{OrgKind, Role};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::audit::AuditEvent;
use crate::auth::{password, rbac};
use crate::routes::auth::normalize_email;
use crate::routes::users::valid_email;

/// Environment variable read for the new user's password before falling back to stdin.
pub const PASSWORD_ENV: &str = "ASD_ADMIN_PASSWORD";

pub const USAGE: &str = "usage: asd-backend admin <command> [options]\n\
\n\
  create-org  --name <name> --kind home|clinic|research [--retention-months N] [--force]\n\
  create-user --email <email> --role owner|superadmin [--org <org-id>] [--force]\n\
\n\
create-user reads the password from ASD_ADMIN_PASSWORD, or else the first line of stdin;\n\
it is never accepted as an argument. owner needs --org; superadmin has no org.\n\
--force allows a second superadmin, or a second org with the same name.\n\
Both commands print the new id on stdout and write an audit row.\n\
Exit codes: 0 ok, 2 usage/invalid input, 3 conflicts with existing data, 1 other errors.";

/// A new organisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOrg {
    pub name: String,
    pub kind: OrgKind,
    pub retention_months: Option<u32>,
    /// Create even when an org with the same name (case-insensitive) exists.
    pub force: bool,
}

/// A new `owner` (with `org_id`) or `superadmin` (without).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUser {
    pub email: String,
    pub role: Role,
    pub org_id: Option<Uuid>,
    /// Create a superadmin even when one exists. Never overrides an existing e-mail.
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminCommand {
    CreateOrg(NewOrg),
    CreateUser(NewUser),
    Help,
}

#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    #[error("{0}\n\n{USAGE}")]
    Usage(String),
    #[error("{0}")]
    Invalid(String),
    #[error("a user with e-mail {email} already exists (id {id}); nothing was changed")]
    EmailTaken { email: String, id: Uuid },
    #[error("a superadmin already exists; pass --force to create another")]
    SuperadminExists,
    #[error(
        "an organisation named {name:?} already exists (id {id}); pass --force to create another"
    )]
    OrgNameTaken { name: String, id: Uuid },
    #[error("organisation {0} does not exist")]
    OrgNotFound(Uuid),
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Internal(String),
}

impl AdminError {
    /// Process exit code for this error (see the module docs).
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) | Self::Invalid(_) | Self::OrgNotFound(_) => 2,
            Self::EmailTaken { .. } | Self::SuperadminExists | Self::OrgNameTaken { .. } => 3,
            Self::Database(_) | Self::Internal(_) => 1,
        }
    }
}

fn usage(msg: impl Into<String>) -> AdminError {
    AdminError::Usage(msg.into())
}

/// Splits `--flag value` / `--flag=value` / `--force` options. `args` excludes `admin`
/// and the command name.
fn options(args: &[String], valued: &[&str]) -> Result<Vec<(String, Option<String>)>, AdminError> {
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) => (n, Some(v.to_owned())),
            None => (arg.as_str(), None),
        };
        if name == "--password" || name.starts_with("--password") {
            return Err(usage(format!(
                "the password is never accepted as an argument; set {PASSWORD_ENV} or pipe it on stdin"
            )));
        }
        if name == "--force" {
            if inline.is_some() {
                return Err(usage("--force takes no value"));
            }
            out.push((name.to_owned(), None));
        } else if valued.contains(&name) {
            let value = match inline {
                Some(v) => v,
                None => it
                    .next()
                    .cloned()
                    .ok_or_else(|| usage(format!("{name} needs a value")))?,
            };
            if out.iter().any(|(n, _)| n == name) {
                return Err(usage(format!("{name} given twice")));
            }
            out.push((name.to_owned(), Some(value)));
        } else {
            return Err(usage(format!("unknown option {name}")));
        }
    }
    Ok(out)
}

fn take(opts: &[(String, Option<String>)], name: &str) -> Option<String> {
    opts.iter()
        .find(|(n, _)| n == name)
        .and_then(|(_, v)| v.clone())
}

fn required(opts: &[(String, Option<String>)], name: &str) -> Result<String, AdminError> {
    take(opts, name).ok_or_else(|| usage(format!("{name} is required")))
}

/// Parses the arguments after `asd-backend admin`.
pub fn parse(args: &[String]) -> Result<AdminCommand, AdminError> {
    let Some((command, rest)) = args.split_first() else {
        return Err(usage("missing command"));
    };
    match command.as_str() {
        "-h" | "--help" | "help" => Ok(AdminCommand::Help),
        "create-org" => {
            let opts = options(rest, &["--name", "--kind", "--retention-months"])?;
            let name = required(&opts, "--name")?.trim().to_owned();
            if name.is_empty() {
                return Err(AdminError::Invalid("--name must not be empty".into()));
            }
            let kind = match required(&opts, "--kind")?.as_str() {
                "home" => OrgKind::Home,
                "clinic" => OrgKind::Clinic,
                "research" => OrgKind::Research,
                other => {
                    return Err(AdminError::Invalid(format!(
                        "--kind {other:?} is not one of home, clinic, research"
                    )))
                }
            };
            let retention_months = take(&opts, "--retention-months")
                .map(|v| match v.parse::<u32>() {
                    Ok(n) if (1..=i32::MAX as u32).contains(&n) => Ok(n),
                    _ => Err(AdminError::Invalid(
                        "--retention-months must be a positive integer".into(),
                    )),
                })
                .transpose()?;
            Ok(AdminCommand::CreateOrg(NewOrg {
                name,
                kind,
                retention_months,
                force: take_flag(&opts, "--force"),
            }))
        }
        "create-user" => {
            let opts = options(rest, &["--email", "--role", "--org"])?;
            let email = required(&opts, "--email")?;
            let role = match required(&opts, "--role")?.as_str() {
                "owner" => Role::Owner,
                "superadmin" => Role::Superadmin,
                other => {
                    return Err(AdminError::Invalid(format!(
                        "--role {other:?}: only owner or superadmin are created here; \
                         an owner adds other roles through the API"
                    )))
                }
            };
            let org_id = take(&opts, "--org")
                .map(|v| {
                    Uuid::parse_str(v.trim())
                        .map_err(|_| AdminError::Invalid("--org must be an org id (UUID)".into()))
                })
                .transpose()?;
            Ok(AdminCommand::CreateUser(NewUser {
                email,
                role,
                org_id,
                force: take_flag(&opts, "--force"),
            }))
        }
        other => Err(usage(format!("unknown command {other:?}"))),
    }
}

fn take_flag(opts: &[(String, Option<String>)], name: &str) -> bool {
    opts.iter().any(|(n, _)| n == name)
}

/// The password from `env` (the value of [`PASSWORD_ENV`]) or else the first line of
/// `input`; only the line terminator is stripped. Checked against the API policy.
pub fn read_password(env: Option<String>, input: &mut dyn BufRead) -> Result<String, AdminError> {
    let raw = match env {
        Some(v) => v,
        None => {
            let mut line = String::new();
            input
                .read_line(&mut line)
                .map_err(|e| AdminError::Internal(format!("read password from stdin: {e}")))?;
            line
        }
    };
    let pw = raw
        .strip_suffix('\n')
        .map(|s| s.strip_suffix('\r').unwrap_or(s))
        .unwrap_or(&raw)
        .to_owned();
    if pw.is_empty() {
        return Err(AdminError::Invalid(format!(
            "no password: set {PASSWORD_ENV} or pipe it on stdin"
        )));
    }
    password::validate_new(&pw)
        .map_err(|e| AdminError::Invalid(format!("password {}", e.reason)))?;
    Ok(pw)
}

fn kind_str(kind: OrgKind) -> &'static str {
    match kind {
        OrgKind::Home => "home",
        OrgKind::Clinic => "clinic",
        OrgKind::Research => "research",
    }
}

/// Inserts an organisation and its audit row; returns its id.
pub async fn create_org(pool: &PgPool, org: &NewOrg) -> Result<Uuid, AdminError> {
    let name = org.name.trim();
    if name.is_empty() {
        return Err(AdminError::Invalid("--name must not be empty".into()));
    }
    let retention = org
        .retention_months
        .map(|m| i32::try_from(m).map_err(|_| AdminError::Invalid("retention too large".into())))
        .transpose()?;
    let kind = kind_str(org.kind);
    let mut tx = pool.begin().await?;
    // Serialises concurrent bootstrap runs (the name check below is not a constraint).
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtext('asd-admin-create-org'))")
        .execute(&mut *tx)
        .await?;
    if !org.force {
        if let Some(id) = sqlx::query_scalar!(
            "SELECT id FROM organizations WHERE lower(name) = lower($1) ORDER BY created_at LIMIT 1",
            name
        )
        .fetch_optional(&mut *tx)
        .await?
        {
            return Err(AdminError::OrgNameTaken {
                name: name.to_owned(),
                id,
            });
        }
    }
    let row = sqlx::query!(
        "INSERT INTO organizations (name, kind, retention_months)
         VALUES ($1, $2, COALESCE($3, 24))
         RETURNING id, retention_months",
        name,
        kind,
        retention
    )
    .fetch_one(&mut *tx)
    .await?;
    AuditEvent::system("org.create", "organization", row.id)
        .org(row.id)
        .after(json!({
            "name": name,
            "kind": kind,
            "retention_months": row.retention_months,
            "via": "cli",
            "forced": org.force,
        }))
        .record(&mut tx)
        .await
        .map_err(|e| AdminError::Internal(e.to_string()))?;
    tx.commit().await?;
    Ok(row.id)
}

/// Inserts an active `owner` or `superadmin` with an Argon2id hash of `password` and its
/// audit row; returns the user id.
pub async fn create_user(
    pool: &PgPool,
    user: &NewUser,
    password: String,
) -> Result<Uuid, AdminError> {
    let email = normalize_email(&user.email);
    if !valid_email(&email) {
        return Err(AdminError::Invalid(format!(
            "--email {email:?} is not an e-mail address"
        )));
    }
    match (user.role, user.org_id) {
        (Role::Superadmin, Some(_)) => {
            return Err(AdminError::Invalid(
                "a superadmin belongs to no org; drop --org".into(),
            ))
        }
        (Role::Owner, None) => return Err(AdminError::Invalid("an owner needs --org".into())),
        (Role::Owner | Role::Superadmin, _) => {}
        (other, _) => {
            return Err(AdminError::Invalid(format!(
                "role {} is created through the API, not here",
                rbac::role_as_str(other)
            )))
        }
    }
    password::validate_new(&password)
        .map_err(|e| AdminError::Invalid(format!("password {}", e.reason)))?;
    let pw_hash = password::hash_blocking(password)
        .await
        .map_err(|e| AdminError::Internal(e.to_string()))?;
    let role = rbac::role_as_str(user.role);

    let mut tx = pool.begin().await?;
    // Serialises concurrent runs so the superadmin check holds.
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtext('asd-admin-create-user'))")
        .execute(&mut *tx)
        .await?;
    if let Some(id) = sqlx::query_scalar!("SELECT id FROM users WHERE lower(email) = $1", email)
        .fetch_optional(&mut *tx)
        .await?
    {
        return Err(AdminError::EmailTaken { email, id });
    }
    if let Some(org) = user.org_id {
        sqlx::query_scalar!("SELECT id FROM organizations WHERE id = $1 FOR SHARE", org)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AdminError::OrgNotFound(org))?;
    }
    if user.role == Role::Superadmin && !user.force {
        let existing =
            sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM users WHERE role = 'superadmin'"#)
                .fetch_one(&mut *tx)
                .await?;
        if existing > 0 {
            return Err(AdminError::SuperadminExists);
        }
    }
    let id = sqlx::query_scalar!(
        "INSERT INTO users (org_id, email, pw_hash, role, status)
         VALUES ($1, $2, $3, $4, 'active')
         ON CONFLICT ((lower(email))) DO NOTHING
         RETURNING id",
        user.org_id,
        email,
        pw_hash,
        role
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AdminError::Internal("e-mail taken concurrently; re-run".into()))?;
    let mut event = AuditEvent::system("user.create", "user", id).after(json!({
        "email": email,
        "role": role,
        "status": "active",
        "via": "cli",
        "forced": user.force,
    }));
    if let Some(org) = user.org_id {
        event = event.org(org);
    }
    event
        .record(&mut tx)
        .await
        .map_err(|e| AdminError::Internal(e.to_string()))?;
    tx.commit().await?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &[&str]) -> Vec<String> {
        s.iter().map(|a| (*a).to_owned()).collect()
    }

    #[test]
    fn parses_create_org() {
        let cmd = parse(&args(&[
            "create-org",
            "--name",
            " First Clinic ",
            "--kind=research",
            "--retention-months",
            "36",
        ]))
        .unwrap();
        assert_eq!(
            cmd,
            AdminCommand::CreateOrg(NewOrg {
                name: "First Clinic".into(),
                kind: OrgKind::Research,
                retention_months: Some(36),
                force: false,
            })
        );
        assert!(matches!(
            parse(&args(&[
                "create-org",
                "--name",
                "x",
                "--kind",
                "clinic",
                "--force"
            ])),
            Ok(AdminCommand::CreateOrg(NewOrg { force: true, .. }))
        ));
    }

    #[test]
    fn parses_create_user() {
        let org = Uuid::new_v4();
        let cmd = parse(&args(&[
            "create-user",
            "--email",
            "a@b.org",
            "--role",
            "owner",
            "--org",
            &org.to_string(),
        ]))
        .unwrap();
        assert_eq!(
            cmd,
            AdminCommand::CreateUser(NewUser {
                email: "a@b.org".into(),
                role: Role::Owner,
                org_id: Some(org),
                force: false,
            })
        );
    }

    #[test]
    fn rejects_bad_input_with_usage_exit_code() {
        for bad in [
            &[][..],
            &["nope"],
            &["create-org", "--kind", "clinic"],
            &["create-org", "--name", "x", "--kind", "hospital"],
            &["create-org", "--name", "  ", "--kind", "clinic"],
            &[
                "create-org",
                "--name",
                "x",
                "--kind",
                "clinic",
                "--retention-months",
                "0",
            ],
            &[
                "create-org",
                "--name",
                "x",
                "--name",
                "y",
                "--kind",
                "clinic",
            ],
            &["create-user", "--email", "a@b.org", "--role", "admin"],
            &[
                "create-user",
                "--email",
                "a@b.org",
                "--role",
                "owner",
                "--org",
                "nope",
            ],
            &["create-user", "--email", "a@b.org"],
            &["create-user", "--email"],
            &[
                "create-user",
                "--email",
                "a@b.org",
                "--role",
                "owner",
                "--bogus",
            ],
        ] {
            let err = parse(&args(bad)).unwrap_err();
            assert_eq!(err.exit_code(), 2, "{bad:?}: {err}");
        }
        assert_eq!(parse(&args(&["--help"])).unwrap(), AdminCommand::Help);
    }

    #[test]
    fn never_accepts_a_password_argument_and_never_echoes_it() {
        for bad in [
            &[
                "create-user",
                "--password",
                "hunter2hunter2",
                "--email",
                "a@b.org",
            ][..],
            &["create-user", "--password=hunter2hunter2"],
        ] {
            let err = parse(&args(bad)).unwrap_err();
            assert_eq!(err.exit_code(), 2);
            let msg = err.to_string();
            assert!(msg.contains(PASSWORD_ENV), "{msg}");
            assert!(!msg.contains("hunter2"), "{msg}");
        }
    }

    #[test]
    fn password_from_env_wins_over_stdin_and_only_the_newline_is_stripped() {
        let mut stdin: &[u8] = b"from stdin password\n";
        assert_eq!(
            read_password(Some("from env password".into()), &mut stdin).unwrap(),
            "from env password"
        );
        let mut stdin: &[u8] = b" spaced  password \r\nsecond line";
        assert_eq!(
            read_password(None, &mut stdin).unwrap(),
            " spaced  password "
        );
        let mut empty: &[u8] = b"";
        assert_eq!(read_password(None, &mut empty).unwrap_err().exit_code(), 2);
        let mut short: &[u8] = b"short\n";
        let err = read_password(None, &mut short).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(!err.to_string().contains("short\n"));
    }

    #[test]
    fn exit_codes() {
        assert_eq!(AdminError::SuperadminExists.exit_code(), 3);
        assert_eq!(
            AdminError::EmailTaken {
                email: "a@b.org".into(),
                id: Uuid::nil()
            }
            .exit_code(),
            3
        );
        assert_eq!(AdminError::Internal("x".into()).exit_code(), 1);
    }
}
