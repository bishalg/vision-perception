//! owner: WP-B.2 (bootstrap follow-up)
//!
//! Operator commands for a fresh deployment, run as `asd-backend admin ...`:
//!
//! - `create-org --name <name> --kind home|clinic|research [--retention-months N] [--force]`
//! - `create-user --email <email> --role owner|superadmin [--org <uuid>] [--force]`
//! - `reset-totp --user <uuid>` (WP-B.14, AUTH-9): clears the user's TOTP seed, spent step
//!   and any pending enrolment and revokes their refresh tokens
//!   ([`crate::auth::totp::reset_for_user`], the function the org-admin route of WP-B.17
//!   uses too, through [`crate::auth::totp::reset`]) with audit `user.totp_reset`
//!   (`actor: operator`), so a user who lost the authenticator signs in with the password
//!   alone and enrols again. No KEK needed. Prints the user id. For a superadmin, or when
//!   no owner/admin of the org can sign in; otherwise `POST /v1/orgs/{id}/users/{uid}/
//!   totp/reset`.
//! - `reset-password --user <uuid>` (WP-B.24, 164.312(a)(2)(ii) emergency access): server
//!   shell only. For an **active** user sets a new Argon2id hash, revokes every refresh
//!   token (same transaction) and audits `user.password_reset` (`actor: operator`,
//!   `generated`; never the password or hash). TOTP is not touched (`reset-totp` is
//!   separate). The password is a generated temporary one printed once on stdout, or the
//!   value of `ASD_ADMIN_PASSWORD` (then not echoed). There is no forced change at the
//!   next sign-in (no mechanism exists yet).
//! - `rotate-kek [--batch N] [--dry-run]` (WP-B.15, KEY-9): re-wraps every org data key
//!   and re-seals every user secret still under a KEK from `ASD_DATA_KEK_PREVIOUS` with
//!   the active `ASD_DATA_KEK` ([`crate::crypto::rotate`]). Prints
//!   `{"rewrapped_deks","resealed_users","remaining","dry_run"}` as JSON; exits 3 (after
//!   printing) while rows remain under a KEK that is not configured. Runbook:
//!   `rust/README.md`.
//! - `erasure-ledger` (WP-B.22, DPL-6): prints one JSON line per `erasure_requests` row,
//!   `{participant_id, org_id, requested_at, status}` (oldest first; ids only, no tokens),
//!   which `backup.sh` appends to `erasure-ledger.jsonl` outside the database dump.
//! - `requeue-erasure --ledger <file>` (WP-B.22, DPL-7): after a restore, every ledger
//!   participant whose restored row is not erased gets an erasure job
//!   ([`crate::jobs::erasure::requeue`], audit `participant.erasure_requeued`, `via: cli`);
//!   erased ones are skipped, unknown ids are reported (stderr and the report), not fatal.
//!   Prints `{"entries","participants","requeued","already_erased","in_progress","held",
//!   "unknown"}`. The worker does the erasing (trap 21).
//! - `verify` (WP-B.22, DPL-8): `SELECT 1` from every table and unwraps every
//!   `org_data_keys` row with `ASD_DATA_KEK` (plus `ASD_DATA_KEK_PREVIOUS`); prints
//!   `{"tables","data_keys","kek_id"}` (a fingerprint, never key material) or exits 3 when
//!   a data key cannot be unwrapped (the key card does not match the database).
//!
//! `encrypt-review-notes` (WP-C.9, KEY-7) was removed with migration 0022, which drops
//! `review_items.note` (WP-B.14, KEY-11): a deployment that still has plaintext notes runs
//! it with the previous release before upgrading (the 0022 guard refuses otherwise).
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
use crate::config::Config;
use crate::crypto::rotate::{self, RotateOptions, RotateReport};
use crate::crypto::Keyring;
use crate::routes::auth::normalize_email;
use crate::routes::users::valid_email;

/// Environment variable read for the new user's password before falling back to stdin.
pub const PASSWORD_ENV: &str = "ASD_ADMIN_PASSWORD";

pub const USAGE: &str = "usage: asd-backend admin <command> [options]\n\
\n\
  create-org  --name <name> --kind home|clinic|research [--retention-months N] [--force]\n\
  create-user --email <email> --role owner|superadmin [--org <org-id>] [--force]\n\
  reset-totp  --user <user-id>\n\
  reset-password --user <user-id>\n\
  rotate-kek [--batch N] [--dry-run]\n\
  erasure-ledger\n\
  requeue-erasure --ledger <file>\n\
  verify\n\
\n\
create-user reads the password from ASD_ADMIN_PASSWORD, or else the first line of stdin;\n\
it is never accepted as an argument. owner needs --org; superadmin has no org.\n\
--force allows a second superadmin, or a second org with the same name.\n\
create-org and create-user print the new id on stdout and write an audit row.\n\
reset-totp clears the user's TOTP seed and any pending enrolment (they sign in with the\n\
password and enrol again), writes an audit row and prints the user id.\n\
reset-password (active users only; server shell, emergency access) sets a new password and\n\
revokes the user's refresh tokens; TOTP is untouched. It prints a generated temporary password\n\
once on stdout, or, if ASD_ADMIN_PASSWORD is set, uses that and prints the user id. Hand the\n\
password over in person. Access tokens already issued stay valid up to 15 minutes; there is\n\
no forced change at next sign-in. Writes audit user.password_reset (no secret).\n\
rotate-kek (needs ASD_DATA_KEK=new, ASD_DATA_KEK_PREVIOUS=old) moves every wrapped data key\n\
and sealed user secret to the new KEK, prints a JSON report and exits 3 while rows remain\n\
under an unconfigured KEK; running it again is safe. Runbook: rust/README.md.\n\
erasure-ledger prints one JSON line per erasure request (for backups); requeue-erasure replays\n\
such a file after a restore (queues an erasure job for every listed participant that is not\n\
erased) and prints a JSON report; verify checks every table and unwraps every org data key\n\
with ASD_DATA_KEK, exiting 3 when the KEK does not match.\n\
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
    /// AUTH-9 operator TOTP reset.
    ResetTotp {
        user_id: Uuid,
    },
    /// WP-B.24 operator password reset (emergency access).
    ResetPassword {
        user_id: Uuid,
    },
    /// KEY-9 KEK rotation.
    RotateKek {
        batch: u32,
        dry_run: bool,
    },
    /// DPL-6 erasure ledger (WP-B.22).
    ErasureLedger,
    /// DPL-7 ledger replay after a restore (WP-B.22).
    RequeueErasure {
        ledger: std::path::PathBuf,
    },
    /// DPL-8 restore check (WP-B.22).
    Verify,
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
    #[error("user {0} does not exist; nothing was changed")]
    UserNotFound(Uuid),
    /// `rotate-kek` finished but rows remain under a KEK that is not configured (or could
    /// not be opened, or were locked). The report is still printed on stdout.
    #[error(
        "{} row(s) are still not under the active ASD_DATA_KEK: they are sealed under a KEK \
         that is not configured, could not be opened, or were locked; add the missing key to \
         ASD_DATA_KEK_PREVIOUS and run rotate-kek again",
        .0.remaining
    )]
    RotationIncomplete(RotateReport),
    /// `verify`: the database is unreadable with the configured KEK (exit 3).
    #[error("verify failed: {0}")]
    VerifyFailed(String),
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Internal(String),
}

impl AdminError {
    /// Process exit code for this error (see the module docs).
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) | Self::Invalid(_) | Self::OrgNotFound(_) | Self::UserNotFound(_) => 2,
            Self::EmailTaken { .. }
            | Self::SuperadminExists
            | Self::OrgNameTaken { .. }
            | Self::RotationIncomplete(_)
            | Self::VerifyFailed(_) => 3,
            Self::Database(_) | Self::Internal(_) => 1,
        }
    }
}

fn usage(msg: impl Into<String>) -> AdminError {
    AdminError::Usage(msg.into())
}

/// Splits `--flag value` / `--flag=value` / bare `flags` (e.g. `--force`) options. `args`
/// excludes `admin` and the command name.
fn options(
    args: &[String],
    valued: &[&str],
    flags: &[&str],
) -> Result<Vec<(String, Option<String>)>, AdminError> {
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
        if flags.contains(&name) {
            if inline.is_some() {
                return Err(usage(format!("{name} takes no value")));
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
            let opts = options(
                rest,
                &["--name", "--kind", "--retention-months"],
                &["--force"],
            )?;
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
            let opts = options(rest, &["--email", "--role", "--org"], &["--force"])?;
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
        "reset-totp" => {
            let opts = options(rest, &["--user"], &[])?;
            let user_id = Uuid::parse_str(required(&opts, "--user")?.trim())
                .map_err(|_| AdminError::Invalid("--user must be a user id (UUID)".into()))?;
            Ok(AdminCommand::ResetTotp { user_id })
        }
        "reset-password" => {
            let opts = options(rest, &["--user"], &[])?;
            let user_id = Uuid::parse_str(required(&opts, "--user")?.trim())
                .map_err(|_| AdminError::Invalid("--user must be a user id (UUID)".into()))?;
            Ok(AdminCommand::ResetPassword { user_id })
        }
        "rotate-kek" => {
            let opts = options(rest, &["--batch"], &["--dry-run"])?;
            let batch = match take(&opts, "--batch") {
                None => rotate::DEFAULT_BATCH,
                Some(v) => match v.parse::<u32>() {
                    Ok(n) if (1..=rotate::MAX_BATCH).contains(&n) => n,
                    _ => {
                        return Err(AdminError::Invalid(format!(
                            "--batch must be an integer from 1 to {}",
                            rotate::MAX_BATCH
                        )))
                    }
                },
            };
            Ok(AdminCommand::RotateKek {
                batch,
                dry_run: take_flag(&opts, "--dry-run"),
            })
        }
        "encrypt-review-notes" => Err(usage(
            "encrypt-review-notes was removed with migration 0022 (review_items.note is dropped); \
             run it with the previous release before upgrading, see rust/README.md",
        )),
        "erasure-ledger" => {
            options(rest, &[], &[])?;
            Ok(AdminCommand::ErasureLedger)
        }
        "requeue-erasure" => {
            let opts = options(rest, &["--ledger"], &[])?;
            let ledger = required(&opts, "--ledger")?;
            if ledger.trim().is_empty() {
                return Err(AdminError::Invalid("--ledger must name a file".into()));
            }
            Ok(AdminCommand::RequeueErasure {
                ledger: ledger.into(),
            })
        }
        "verify" => {
            options(rest, &[], &[])?;
            Ok(AdminCommand::Verify)
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

/// `reset-totp` (AUTH-9): clears every TOTP column of `user_id` and revokes its refresh
/// tokens ([`crate::auth::totp::reset_for_user`]) with audit `user.totp_reset`
/// (`actor: operator`); [`AdminError::UserNotFound`] (exit 2) changes nothing.
pub async fn reset_totp(pool: &PgPool, user_id: Uuid) -> Result<Uuid, AdminError> {
    let found = crate::auth::totp::reset(pool, user_id)
        .await
        .map_err(|e| AdminError::Internal(e.to_string()))?;
    if found {
        Ok(user_id)
    } else {
        Err(AdminError::UserNotFound(user_id))
    }
}

/// Length of a generated temporary password.
pub const TEMP_PASSWORD_LEN: usize = 20;

/// A temporary password: [`TEMP_PASSWORD_LEN`] characters of
/// `asd_contract::emr::SHARE_PASSCODE_ALPHABET` (32 symbols, so `byte & 31` is unbiased).
pub fn generate_temp_password() -> Result<String, AdminError> {
    let alphabet = asd_contract::emr::SHARE_PASSCODE_ALPHABET;
    let mut bytes = [0u8; TEMP_PASSWORD_LEN];
    getrandom::fill(&mut bytes).map_err(|e| AdminError::Internal(format!("randomness: {e}")))?;
    Ok(bytes
        .iter()
        .map(|b| char::from(alphabet[usize::from(b & 31)]))
        .collect())
}

/// `reset-password` (WP-B.24): sets a new Argon2id hash for an active user, revokes every
/// refresh token and audits `user.password_reset` (no secret), in one transaction. TOTP is
/// untouched. `supplied` (from `ASD_ADMIN_PASSWORD`) is checked with the API policy;
/// otherwise a temporary password is generated. Returns the password now in effect (the
/// caller prints it only when it was generated). Unknown, `disabled` or `invited` users
/// are exit 2 and nothing changes.
pub async fn reset_password(
    pool: &PgPool,
    user_id: Uuid,
    supplied: Option<String>,
) -> Result<String, AdminError> {
    let generated = supplied.is_none();
    let new_password = match supplied {
        Some(p) => {
            password::validate_new(&p)
                .map_err(|e| AdminError::Invalid(format!("password {}", e.reason)))?;
            p
        }
        None => generate_temp_password()?,
    };
    let pw_hash = password::hash_blocking(new_password.clone())
        .await
        .map_err(|e| AdminError::Internal(e.to_string()))?;

    let mut tx = pool.begin().await?;
    let row = sqlx::query!(
        "SELECT org_id, role, status FROM users WHERE id = $1 FOR UPDATE",
        user_id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AdminError::UserNotFound(user_id))?;
    if row.status != "active" {
        return Err(AdminError::Invalid(format!(
            "user {user_id} is {}, not active; nothing was changed",
            row.status
        )));
    }
    sqlx::query!(
        "UPDATE users SET pw_hash = $2 WHERE id = $1",
        user_id,
        pw_hash
    )
    .execute(&mut *tx)
    .await?;
    crate::auth::refresh::revoke_user(&mut tx, user_id)
        .await
        .map_err(|e| AdminError::Internal(e.to_string()))?;
    let mut event = AuditEvent::system("user.password_reset", "user", user_id).after(json!({
        "actor": "operator",
        "via": "cli",
        "role": row.role,
        "generated": generated,
    }));
    if let Some(org) = row.org_id {
        event = event.org(org);
    }
    event
        .record(&mut tx)
        .await
        .map_err(|e| AdminError::Internal(e.to_string()))?;
    tx.commit().await?;
    Ok(new_password)
}

/// `rotate-kek` from the CLI (KEY-9): builds the keyring from `ASD_DATA_KEK` (active) and
/// `ASD_DATA_KEK_PREVIOUS`, runs [`rotate::rotate_kek`] and returns the JSON report.
/// [`AdminError::RotationIncomplete`] (exit 3) carries the report when rows remain.
pub async fn rotate_kek_cli(
    pool: &PgPool,
    config: &Config,
    batch: u32,
    dry_run: bool,
) -> Result<String, AdminError> {
    let kek = config.data_kek.clone().ok_or_else(|| {
        AdminError::Invalid(
            "ASD_DATA_KEK is not set; rotate-kek needs the new KEK in ASD_DATA_KEK and the old \
             one in ASD_DATA_KEK_PREVIOUS"
                .into(),
        )
    })?;
    let keyring = Keyring::with_previous(kek, config.data_kek_previous.clone(), pool.clone());
    let report = rotate::rotate_kek(pool, &keyring, RotateOptions { batch, dry_run })
        .await
        .map_err(|e| match e {
            crate::crypto::CryptoError::Database(e) => AdminError::Database(e),
            other => AdminError::Internal(other.to_string()),
        })?;
    if report.remaining > 0 {
        return Err(AdminError::RotationIncomplete(report));
    }
    report_json(&report)
}

/// The `rotate-kek` report as one JSON line.
pub fn report_json(report: &RotateReport) -> Result<String, AdminError> {
    serde_json::to_string(report).map_err(|e| AdminError::Internal(e.to_string()))
}

// ---- restore helpers (WP-B.22, DPL-6..8) -------------------------------------------------

/// One line of the erasure ledger (`erasure-ledger`, read back by `requeue-erasure`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LedgerEntry {
    pub participant_id: Uuid,
    pub org_id: Uuid,
    /// RFC 3339.
    pub requested_at: String,
    /// `pending`, `completed` or `held` at the time of the backup.
    pub status: String,
}

/// `erasure-ledger`: one JSON line per `erasure_requests` row, oldest first (empty string
/// when there is none).
pub async fn erasure_ledger(pool: &PgPool) -> Result<String, AdminError> {
    let rows = sqlx::query!(
        "SELECT participant_id, org_id, requested_at, status
         FROM erasure_requests ORDER BY requested_at, id"
    )
    .fetch_all(pool)
    .await?;
    let mut lines = Vec::with_capacity(rows.len());
    for r in rows {
        let entry = LedgerEntry {
            participant_id: r.participant_id,
            org_id: r.org_id,
            requested_at: r
                .requested_at
                .to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
            status: r.status,
        };
        lines.push(serde_json::to_string(&entry).map_err(|e| AdminError::Internal(e.to_string()))?);
    }
    Ok(lines.join("\n"))
}

/// Parses a ledger file: one [`LedgerEntry`] per non-blank line. A malformed line is an
/// error (exit 2) before anything is changed.
pub fn parse_ledger(text: &str) -> Result<Vec<LedgerEntry>, AdminError> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| {
            serde_json::from_str::<LedgerEntry>(l).map_err(|e| {
                AdminError::Invalid(format!("ledger line {} is not a ledger entry: {e}", i + 1))
            })
        })
        .collect()
}

/// What `requeue-erasure` did (printed as one JSON line).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct RequeueReport {
    /// Ledger lines read.
    pub entries: usize,
    /// Distinct `(org, participant)` pairs among them.
    pub participants: usize,
    pub requeued: usize,
    pub already_erased: usize,
    pub in_progress: usize,
    pub held: usize,
    /// Participant ids not in this database (or not in the ledger's org).
    pub unknown: Vec<Uuid>,
}

/// `requeue-erasure --ledger <file>`: [`crate::jobs::erasure::requeue`] for every distinct
/// participant of the ledger (a nightly ledger repeats rows). Unknown ids are reported on
/// stderr and in the report, not fatal.
pub async fn requeue_erasure(
    pool: &PgPool,
    ledger: &std::path::Path,
) -> Result<RequeueReport, AdminError> {
    use crate::jobs::erasure::{requeue, Requeued};

    let text = std::fs::read_to_string(ledger).map_err(|e| {
        AdminError::Invalid(format!("cannot read ledger {}: {e}", ledger.display()))
    })?;
    let entries = parse_ledger(&text)?;
    let mut seen = std::collections::BTreeSet::new();
    let mut report = RequeueReport {
        entries: entries.len(),
        ..RequeueReport::default()
    };
    for e in &entries {
        if !seen.insert((e.org_id, e.participant_id)) {
            continue;
        }
        let outcome = requeue(pool, e.org_id, e.participant_id)
            .await
            .map_err(|err| AdminError::Internal(err.to_string()))?;
        match outcome {
            Requeued::Queued { .. } => report.requeued += 1,
            Requeued::AlreadyErased => report.already_erased += 1,
            Requeued::InProgress { .. } => report.in_progress += 1,
            Requeued::Held { .. } => report.held += 1,
            Requeued::Unknown => {
                eprintln!(
                    "asd-backend admin: participant {} of org {} is not in this database; skipped",
                    e.participant_id, e.org_id
                );
                report.unknown.push(e.participant_id);
            }
        }
    }
    report.participants = seen.len();
    Ok(report)
}

/// `requeue-erasure` from the CLI: the report as one JSON line.
pub async fn requeue_erasure_cli(
    pool: &PgPool,
    ledger: &std::path::Path,
) -> Result<String, AdminError> {
    let report = requeue_erasure(pool, ledger).await?;
    serde_json::to_string(&report).map_err(|e| AdminError::Internal(e.to_string()))
}

/// `verify`: every table answers `SELECT 1`, and every `org_data_keys` row unwraps with
/// the configured KEK(s). Prints counts and the active KEK fingerprint, never key material.
/// [`AdminError::VerifyFailed`] (exit 3) when a data key does not unwrap.
pub async fn verify(pool: &PgPool, config: &Config) -> Result<String, AdminError> {
    use sqlx::Row as _;

    let kek = config.data_kek.clone().ok_or_else(|| {
        AdminError::Invalid(
            "ASD_DATA_KEK is not set; verify checks that the key card's KEK opens the data keys"
                .into(),
        )
    })?;
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT quote_ident(table_name)::text FROM information_schema.tables
         WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'
         ORDER BY table_name",
    )
    .fetch_all(pool)
    .await?;
    for table in &tables {
        // `table` comes from `quote_ident`, so it is a single quoted identifier.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT 1 FROM {table} LIMIT 1"
        )))
        .fetch_optional(pool)
        .await?;
    }
    let keyring = Keyring::with_previous(kek, config.data_kek_previous.clone(), pool.clone());
    let keys = sqlx::query("SELECT key_id, org_id, wrapped_dek, kek_id FROM org_data_keys")
        .fetch_all(pool)
        .await?;
    let mut failed = 0usize;
    for row in &keys {
        let key_id: Uuid = row.try_get("key_id")?;
        let org: Uuid = row.try_get("org_id")?;
        let wrapped: Vec<u8> = row.try_get("wrapped_dek")?;
        let kek_id: String = row.try_get("kek_id")?;
        // Re-wrapping in memory proves the unwrap; nothing is written.
        if keyring.rewrap_dek(org, key_id, &wrapped, &kek_id).is_err() {
            failed += 1;
        }
    }
    if failed > 0 {
        return Err(AdminError::VerifyFailed(format!(
            "{failed} of {} org data key(s) cannot be unwrapped with ASD_DATA_KEK \
             (fingerprint {}) or ASD_DATA_KEK_PREVIOUS: the key card does not match this \
             database",
            keys.len(),
            keyring.kek_id()
        )));
    }
    serde_json::to_string(&json!({
        "tables": tables.len(),
        "data_keys": keys.len(),
        "kek_id": keyring.kek_id(),
    }))
    .map_err(|e| AdminError::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &[&str]) -> Vec<String> {
        s.iter().map(|a| (*a).to_owned()).collect()
    }

    #[test]
    fn parses_reset_password() {
        let id = Uuid::new_v4();
        assert_eq!(
            parse(&args(&["reset-password", "--user", &id.to_string()])).unwrap(),
            AdminCommand::ResetPassword { user_id: id }
        );
        assert!(parse(&args(&["reset-password"])).is_err());
        assert!(parse(&args(&["reset-password", "--user", "nope"])).is_err());
        assert!(parse(&args(&[
            "reset-password",
            "--user",
            &id.to_string(),
            "--password",
            "x"
        ]))
        .is_err());
    }

    #[test]
    fn temp_password_has_length_and_alphabet() {
        let a = generate_temp_password().unwrap();
        let b = generate_temp_password().unwrap();
        assert_eq!(a.len(), TEMP_PASSWORD_LEN);
        assert_ne!(a, b);
        assert!(a
            .bytes()
            .all(|c| asd_contract::emr::SHARE_PASSCODE_ALPHABET.contains(&c)));
        assert!(password::validate_new(&a).is_ok());
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
    fn parses_restore_helpers() {
        assert_eq!(
            parse(&args(&["erasure-ledger"])).unwrap(),
            AdminCommand::ErasureLedger
        );
        assert_eq!(parse(&args(&["verify"])).unwrap(), AdminCommand::Verify);
        assert_eq!(
            parse(&args(&["requeue-erasure", "--ledger", "/tmp/l.jsonl"])).unwrap(),
            AdminCommand::RequeueErasure {
                ledger: "/tmp/l.jsonl".into()
            }
        );
        for bad in [
            &["requeue-erasure"][..],
            &["requeue-erasure", "--ledger"],
            &["requeue-erasure", "--ledger", " "],
            &["erasure-ledger", "--force"],
            &["verify", "--batch", "3"],
        ] {
            assert_eq!(parse(&args(bad)).unwrap_err().exit_code(), 2, "{bad:?}");
        }
        assert_eq!(AdminError::VerifyFailed("x".into()).exit_code(), 3);
    }

    #[test]
    fn ledger_lines_round_trip_and_bad_lines_are_refused() {
        let entry = LedgerEntry {
            participant_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            requested_at: "2026-11-02T10:00:00.000000Z".into(),
            status: "completed".into(),
        };
        let line = serde_json::to_string(&entry).unwrap();
        let text = format!("{line}\n\n{line}\n");
        assert_eq!(parse_ledger(&text).unwrap(), vec![entry.clone(), entry]);
        let err = parse_ledger("{\"participant_id\":\"nope\"}").unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("line 1"), "{err}");
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
