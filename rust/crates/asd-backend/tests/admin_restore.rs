//! owner: WP-B.22
//!
//! Restore helpers (DPL-6..8, acceptance 21, 164.310(d)), through the real binary:
//! `admin erasure-ledger` prints one JSON line per erasure request; `admin requeue-erasure
//! --ledger` queues an erasure job (audited, `via: cli`) for every listed participant that
//! the restored database has not erased, skips erased ones, reports unknown ids without
//! failing, and the worker then erases; `admin verify` exits 0 with the KEK and 3 with
//! another one, never printing key material. Needs `DATABASE_URL`; skipped otherwise.

mod common;
mod participant_support;

use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use asd_backend::crypto::Aad;
use asd_backend::jobs::{runner, JobContext};
use asd_contract::api_v1::Role;
use http::{Method, StatusCode};
use participant_support::Ctx;
use serde_json::Value;
use uuid::Uuid;

use crate::common::*;

/// `DATABASE_URL` with its database name replaced by the test database's.
fn test_db_url(db: &TestDb) -> String {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let (base, query) = match url.split_once('?') {
        Some((b, q)) => (b.to_owned(), format!("?{q}")),
        None => (url.clone(), String::new()),
    };
    let (server, _) = base.rsplit_once('/').expect("URL has a database path");
    format!("{server}/{}{query}", db.name)
}

/// Runs `asd-backend admin <args>` with only `DATABASE_URL` and `extra` set.
fn run_admin(db: &TestDb, args: &[&str], extra: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_asd-backend"));
    cmd.env_clear()
        .env("DATABASE_URL", test_db_url(db))
        .arg("admin")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("spawn asd-backend");
    drop(child.stdin.take());
    child.wait_with_output().expect("asd-backend exits")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

async fn erase(c: &Ctx, bearer: &str, participant: Uuid) {
    let (status, body) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/participants/{participant}/erasure", c.org),
            None,
            bearer,
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
}

async fn run_jobs(c: &Ctx) -> usize {
    let ctx = JobContext::from_state(&c.state).with_heartbeat_every(Duration::from_millis(50));
    runner::run_pending_in(&ctx, "test-worker").await.unwrap()
}

async fn erased(c: &Ctx, participant: Uuid) -> bool {
    sqlx::query_scalar("SELECT erased_at IS NOT NULL FROM participants WHERE id = $1")
        .bind(participant)
        .fetch_one(&c.db.pool)
        .await
        .unwrap()
}

async fn erasure_jobs(c: &Ctx) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'erasure'")
        .fetch_one(&c.db.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn ledger_lists_every_request_and_replay_requeues_only_what_the_restore_brought_back() {
    let Some(c) = Ctx::new().await else { return };
    let (_, owner) = c.token(Role::Owner).await;
    let gone = c.participant(c.org, "TOKEN-GONE").await;
    let back = c.participant(c.org, "TOKEN-BACK").await;
    erase(&c, &owner, gone).await;
    erase(&c, &owner, back).await;
    assert_eq!(run_jobs(&c).await, 2);
    assert!(erased(&c, gone).await && erased(&c, back).await);

    // The ledger: one line per request, ids and status only.
    let out = run_admin(&c.db, &["erasure-ledger"], &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    let lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2, "{text}");
    for line in &lines {
        let mut keys: Vec<&str> = line
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["org_id", "participant_id", "requested_at", "status"]);
        assert_eq!(line["status"], "completed");
        assert_eq!(line["org_id"], c.org.to_string());
        chrono::DateTime::parse_from_rfc3339(line["requested_at"].as_str().unwrap()).unwrap();
    }
    assert!(
        !text.contains("TOKEN-"),
        "no participant tokens in the ledger"
    );

    // "Restore" a backup from before `back` was erased: its row is live again and its
    // request is gone; `gone` stays erased. A nightly ledger repeats lines, and lists an
    // id this database never had.
    sqlx::query("DELETE FROM erasure_requests WHERE participant_id = $1")
        .bind(back)
        .execute(&c.db.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE participants SET erased_at = NULL, withdrawn_at = NULL, language = 'en'
         WHERE id = $1",
    )
    .bind(back)
    .execute(&c.db.pool)
    .await
    .unwrap();
    let unknown = Uuid::new_v4();
    let ledger_path = std::env::temp_dir().join(format!("ledger-{}.jsonl", Uuid::new_v4()));
    {
        let mut f = std::fs::File::create(&ledger_path).unwrap();
        writeln!(f, "{text}\n\n{text}").unwrap();
        writeln!(
            f,
            r#"{{"participant_id":"{unknown}","org_id":"{}","requested_at":"2026-01-01T00:00:00Z","status":"completed"}}"#,
            c.org
        )
        .unwrap();
    }
    let jobs_before = erasure_jobs(&c).await;
    let out = run_admin(
        &c.db,
        &["requeue-erasure", "--ledger", ledger_path.to_str().unwrap()],
        &[],
    );
    assert!(
        out.status.success(),
        "unknown ids are not fatal: {}",
        stderr(&out)
    );
    let report: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(report["entries"], 5);
    assert_eq!(report["participants"], 3);
    assert_eq!(report["requeued"], 1);
    assert_eq!(report["already_erased"], 1);
    assert_eq!(report["unknown"], serde_json::json!([unknown]));
    assert!(
        stderr(&out).contains(&unknown.to_string()),
        "{}",
        stderr(&out)
    );
    assert_eq!(erasure_jobs(&c).await, jobs_before + 1, "exactly one job");
    assert!(
        !erased(&c, back).await,
        "the CLI only queues; the worker erases"
    );

    let audits = c.audit_rows("participant.erasure_requeued").await;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0]["actor_type"], "system");
    assert_eq!(audits[0]["target_id"], back.to_string());
    assert_eq!(audits[0]["after"]["via"], "cli");
    assert_eq!(audits[0]["after"]["new_request"], true);
    let requested_by: String = sqlx::query_scalar(
        "SELECT requested_by_type FROM erasure_requests WHERE participant_id = $1",
    )
    .bind(back)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(requested_by, "system");

    // A second replay while the job is queued adds nothing.
    let out = run_admin(
        &c.db,
        &["requeue-erasure", "--ledger", ledger_path.to_str().unwrap()],
        &[],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let report: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(report["requeued"], 0);
    assert_eq!(report["in_progress"], 1);
    assert_eq!(erasure_jobs(&c).await, jobs_before + 1);

    // The worker erases it again: a tombstone, as before the restore.
    assert_eq!(run_jobs(&c).await, 1);
    assert!(erased(&c, back).await);
    let out = run_admin(
        &c.db,
        &["requeue-erasure", "--ledger", ledger_path.to_str().unwrap()],
        &[],
    );
    let report: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(report["requeued"], 0);
    assert_eq!(report["already_erased"], 2);
    assert_eq!(c.audit_rows("participant.erasure_requeued").await.len(), 1);
    std::fs::remove_file(&ledger_path).ok();
    c.db.cleanup().await;
}

#[tokio::test]
async fn replay_reuses_a_pending_request_without_a_job_and_skips_held_ones() {
    let Some(c) = Ctx::new().await else { return };
    let pending = c.participant(c.org, "TOKEN-PENDING").await;
    let held = c.participant(c.org, "TOKEN-HELD").await;
    // Restored mid-way: a pending request whose job row did not survive, and a held one.
    let request: Uuid = sqlx::query_scalar(
        "INSERT INTO erasure_requests (org_id, participant_id, requested_by_type)
         VALUES ($1, $2, 'device') RETURNING id",
    )
    .bind(c.org)
    .bind(pending)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO erasure_requests (org_id, participant_id, requested_by_type, status,
                                       hold_reason)
         VALUES ($1, $2, 'user', 'held', 'clinical_record')",
    )
    .bind(c.org)
    .bind(held)
    .execute(&c.db.pool)
    .await
    .unwrap();
    let ledger_path = std::env::temp_dir().join(format!("ledger-{}.jsonl", Uuid::new_v4()));
    std::fs::write(
        &ledger_path,
        format!(
            "{{\"participant_id\":\"{pending}\",\"org_id\":\"{org}\",\"requested_at\":\"2026-01-01T00:00:00Z\",\"status\":\"pending\"}}\n\
             {{\"participant_id\":\"{held}\",\"org_id\":\"{org}\",\"requested_at\":\"2026-01-01T00:00:00Z\",\"status\":\"held\"}}\n",
            org = c.org
        ),
    )
    .unwrap();
    let out = run_admin(
        &c.db,
        &["requeue-erasure", "--ledger", ledger_path.to_str().unwrap()],
        &[],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let report: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(report["requeued"], 1, "{report}");
    assert_eq!(report["held"], 1, "{report}");
    let linked: Option<Uuid> =
        sqlx::query_scalar("SELECT job_id FROM erasure_requests WHERE id = $1")
            .bind(request)
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert!(linked.is_some(), "the open request got the job");
    assert_eq!(erasure_jobs(&c).await, 1);
    let audits = c.audit_rows("participant.erasure_requeued").await;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0]["after"]["new_request"], false);
    assert_eq!(
        audits[0]["after"]["erasure_request_id"],
        request.to_string()
    );

    // A malformed ledger changes nothing and is a usage error.
    std::fs::write(&ledger_path, "not json\n").unwrap();
    let out = run_admin(
        &c.db,
        &["requeue-erasure", "--ledger", ledger_path.to_str().unwrap()],
        &[],
    );
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert_eq!(erasure_jobs(&c).await, 1);
    std::fs::remove_file(&ledger_path).ok();
    c.db.cleanup().await;
}

#[tokio::test]
async fn verify_passes_with_the_kek_and_exits_3_with_another_without_printing_keys() {
    let Some(c) = Ctx::new().await else { return };
    // An org data key exists once something is encrypted for the org.
    c.state
        .keyring
        .as_deref()
        .unwrap()
        .encrypt(c.org, &Aad::new("t", "c", c.org, "r"), b"x")
        .await
        .unwrap();
    let kek = test_data_kek();

    let out = run_admin(&c.db, &["verify"], &[("ASD_DATA_KEK", kek)]);
    assert!(out.status.success(), "{}", stderr(&out));
    let report: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(report["data_keys"], 1);
    assert!(report["tables"].as_u64().unwrap() > 10, "{report}");
    assert!(report["kek_id"].as_str().unwrap().starts_with("kek-"));
    assert!(!stdout(&out).contains(kek) && !stderr(&out).contains(kek));

    let wrong = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode([7u8; 32])
    };
    let out = run_admin(&c.db, &["verify"], &[("ASD_DATA_KEK", &wrong)]);
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    assert!(stdout(&out).is_empty());
    assert!(
        stderr(&out).contains("cannot be unwrapped"),
        "{}",
        stderr(&out)
    );
    for o in [stdout(&out), stderr(&out)] {
        assert!(!o.contains(&wrong) && !o.contains(kek));
    }

    // Without a KEK there is nothing to check against: a usage error.
    let out = run_admin(&c.db, &["verify"], &[]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    c.db.cleanup().await;
}
