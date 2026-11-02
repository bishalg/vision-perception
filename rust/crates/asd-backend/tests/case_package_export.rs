//! owner: WP-I.13
//!
//! `case_package` exports (SHR-1..7, acceptance 6 and 9 backend parts): the request
//! rules, the identified package (notes, MRN, consent, recipient; idempotent bytes on a
//! retry), the de-identified package (consent-filtered for research, nothing identifying
//! in any entry), the export row and its audit. Encounters and attachments arrive with
//! WP-I.13b. Needs `DATABASE_URL`.

mod common;
mod export_support;
mod participant_support;

use asd_contract::api_v1::Role;
use export_support::case::*;
use export_support::unzip;
use http::{Method, StatusCode};
use participant_support::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn uuid(v: &Value) -> Uuid {
    v.as_str().unwrap().parse().unwrap()
}

#[tokio::test]
async fn case_requests_follow_the_shr7_rules() {
    let Some(c) = Ctx::new().await else { return };
    let (_, clin) = c.token(Role::Clinician).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (_, admin) = c.token(Role::Admin).await;
    let f = c.case_fixture(&clin).await;
    let p = f.participant;
    let expect = |want: StatusCode, slug: &'static str, (s, b): (StatusCode, Value)| {
        assert_eq!(s, want, "{b}");
        assert_eq!(slug_of(&b), slug, "{b}");
    };

    // Identified without consent: 400 case-consent-missing (either field missing).
    for field in ["consent_reference", "consent_recorded_at"] {
        let mut body = case_body("identified", "clinical_care", p);
        body["case"].as_object_mut().unwrap().remove(field);
        expect(
            StatusCode::BAD_REQUEST,
            "case-consent-missing",
            c.request_export(&clin, body).await,
        );
    }
    // Roles: a researcher has neither ExportIdentified nor CaseShare; an admin no CaseShare.
    expect(
        StatusCode::FORBIDDEN,
        "forbidden",
        c.request_export(&researcher, case_body("identified", "clinical_care", p))
            .await,
    );
    expect(
        StatusCode::FORBIDDEN,
        "forbidden",
        c.request_export(&researcher, case_body("deidentified", "research", p))
            .await,
    );
    expect(
        StatusCode::FORBIDDEN,
        "forbidden",
        c.request_export(&admin, case_body("deidentified", "research", p))
            .await,
    );
    // Identified is for care or the patient's own request.
    for purpose in ["research", "quality_improvement"] {
        let (s, b) = c
            .request_export(&clin, case_body("identified", purpose, p))
            .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{purpose}: {b}");
    }
    // One participant: filters are refused, and so is another participant's session.
    let mut body = case_body("deidentified", "clinical_care", p);
    body["filters"] = json!({"participant_ids": [p, f.other]});
    expect(
        StatusCode::BAD_REQUEST,
        "validation-failed",
        c.request_export(&clin, body).await,
    );
    let mut body = case_body("deidentified", "clinical_care", p);
    body["case"]["session_ids"] = json!([SESSION_1, OTHER_SESSION]);
    expect(
        StatusCode::BAD_REQUEST,
        "validation-failed",
        c.request_export(&clin, body).await,
    );
    // Shape: the case is required, belongs to case_package only, and is validated.
    let mut body = case_body("deidentified", "clinical_care", p);
    body.as_object_mut().unwrap().remove("case");
    expect(
        StatusCode::BAD_REQUEST,
        "validation-failed",
        c.request_export(&clin, body).await,
    );
    let mut body = case_body("deidentified", "clinical_care", p);
    body["format"] = json!("csv");
    expect(
        StatusCode::BAD_REQUEST,
        "validation-failed",
        c.request_export(&clin, body).await,
    );
    for (field, value) in [
        ("recipient", json!("too short")),
        ("recipient", json!("x".repeat(201))),
        ("participant_id", json!("not-a-uuid")),
        ("participant_id", json!(Uuid::new_v4())),
        ("consent_recorded_at", json!("yesterday")),
        ("consent_recorded_at", json!("2999-01-01T00:00:00Z")),
        ("consent_reference", json!("x".repeat(201))),
        ("include_attachments", json!(true)),
    ] {
        let mut body = case_body("deidentified", "clinical_care", p);
        body["case"][field] = value;
        let (s, b) = c.request_export(&clin, body).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{field}: {b}");
        assert_eq!(slug_of(&b), "validation-failed", "{field}: {b}");
    }
    // Withdrawn: 409.
    c.withdraw(f.other).await;
    expect(
        StatusCode::CONFLICT,
        "participant-withdrawn",
        c.request_export(&clin, case_body("identified", "clinical_care", f.other))
            .await,
    );
    // Nothing was queued by any refused request.
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM exports")
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(n, 0);

    // Accepted: identified with consent (attachments accepted, no effect until I.13b),
    // de-identified without.
    let mut body = case_body("identified", "patient_request", p);
    body["case"]["include_attachments"] = json!(true);
    let (s, made) = c.request_export(&clin, body).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{made}");
    assert_eq!(made["format"], "case_package");
    assert_eq!(made["case_participant_id"], p.to_string());
    assert_eq!(made["share_links_n"], 0);
    let mut body = case_body("deidentified", "clinical_care", p);
    body["case"]
        .as_object_mut()
        .unwrap()
        .remove("consent_reference");
    let (s, made) = c.request_export(&clin, body).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{made}");
    c.db.cleanup().await;
}

#[tokio::test]
async fn the_recipient_is_encrypted_and_never_audited() {
    let Some(c) = Ctx::new().await else { return };
    let (_, clin) = c.token(Role::Clinician).await;
    let f = c.case_fixture(&clin).await;
    let (s, made) = c
        .request_export(
            &clin,
            case_body("identified", "clinical_care", f.participant),
        )
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{made}");
    let row: (Vec<u8>, Option<String>, Value) = sqlx::query_as(
        "SELECT case_recipient_ct, case_consent_reference, filters FROM exports WHERE id = $1",
    )
    .bind(uuid(&made["id"]))
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert!(!row
        .0
        .windows(RECIPIENT.len())
        .any(|w| w == RECIPIENT.as_bytes()));
    assert_eq!(row.1.as_deref(), Some(CONSENT_REF));
    assert_eq!(row.2, json!({"participant_ids": [f.participant]}));
    let audit = c.audit_rows("export.create").await;
    assert_eq!(audit.len(), 1);
    let case = &audit[0]["after"]["case"];
    assert_eq!(case["participant_id"], f.participant.to_string());
    assert_eq!(case["consent_reference"], CONSENT_REF);
    assert!(!c.audit_dump().await.contains("Recipient-Sentinel"));
    c.db.cleanup().await;
}

#[tokio::test]
async fn identified_packages_carry_notes_mrn_and_consent_and_retry_to_the_same_bytes() {
    let Some(c) = Ctx::new().await else { return };
    let (_, clin) = c.token(Role::Clinician).await;
    let f = c.case_fixture(&clin).await;
    let done = c.ready_case(&clin, f.participant).await;
    let id = uuid(&done["id"]);
    assert_eq!(done["row_count"], 2, "both sessions of the participant");
    assert!(done["file_name"]
        .as_str()
        .unwrap()
        .ends_with("-case-package.zip"));
    assert_eq!(done["content_type"], "application/zip");
    assert_eq!(
        c.export_items(&id.to_string()).await,
        vec![SESSION_1.to_owned(), SESSION_2.to_owned()]
    );

    let zip = c.artifact(&id.to_string()).await;
    assert_eq!(done["sha256"], hex(&Sha256::digest(&zip)));
    let entries = unzip::entries(&zip);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        &names[..3],
        &["manifest.json", "README.txt", "fhir/bundle.json"]
    );
    assert!(names.contains(&"notes/notes.json"), "{names:?}");
    assert!(!names.iter().any(|n| n.starts_with("attachments/")));
    let manifest: Value = serde_json::from_slice(&entries[0].data).unwrap();
    assert_eq!(manifest["package_id"], id.to_string());
    assert_eq!(manifest["mode"], "identified");
    assert_eq!(manifest["created_at"], done["created_at"]);
    assert_eq!(manifest["recipient"]["label"], RECIPIENT);
    assert_eq!(manifest["consent"]["reference"], CONSENT_REF);
    assert_eq!(
        manifest["consent"]["recorded_at"],
        "2026-10-20T09:00:00.000Z"
    );
    assert!(manifest["consent"]["recorded_by_display"].is_string());
    assert_eq!(manifest["participant"]["token"], TOKEN);
    assert_eq!(manifest["participant"]["mrn"]["value"], MRN);
    assert_eq!(manifest["sessions_n"], 2);
    assert_archive_has(&zip, &[TOKEN, SESSION_1, SESSION_2, NOTE, MRN]);
    assert_archive_clean(&zip, &[OTHER_TOKEN, OTHER_SESSION]);
    // MRN-5: the decrypted MRN is audited once per export, without the MRN.
    let reads = c.audit_rows("patient_ref.export_read").await;
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0]["after"]["count"], 2);
    assert!(!c.audit_dump().await.contains(MRN));

    // A crash before the record phase re-renders under the next attempt: same bytes.
    sqlx::query("UPDATE exports SET status = 'running' WHERE id = $1")
        .bind(id)
        .execute(&c.db.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE jobs SET status = 'queued'")
        .execute(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(c.run_jobs().await, 1);
    let (_, again) = c
        .call(Method::GET, &format!("/v1/exports/{id}"), None, &clin)
        .await;
    assert_eq!(again["status"], "succeeded", "{again}");
    assert_eq!(
        again["sha256"], done["sha256"],
        "same export id, same bytes"
    );
    let key: String = sqlx::query_scalar("SELECT blob_key FROM exports WHERE id = $1")
        .bind(id)
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert!(key.contains("/2/"), "the second attempt's key: {key}");
    assert_eq!(c.export_items(&id.to_string()).await.len(), 2);
    c.db.cleanup().await;
}

#[tokio::test]
async fn named_sessions_limit_the_package() {
    let Some(c) = Ctx::new().await else { return };
    let (_, clin) = c.token(Role::Clinician).await;
    let f = c.case_fixture(&clin).await;
    let mut body = case_body("identified", "clinical_care", f.participant);
    body["case"]["session_ids"] = json!([SESSION_2]);
    let done = c.export_and_run(&clin, body).await;
    assert_eq!(done["status"], "succeeded", "{done}");
    assert_eq!(done["row_count"], 1);
    let zip = c.artifact(done["id"].as_str().unwrap()).await;
    assert_archive_has(&zip, &[SESSION_2]);
    // The note belongs to SESSION_1, which is not in the package.
    assert_archive_clean(&zip, &[SESSION_1, NOTE]);
    c.db.cleanup().await;
}

#[tokio::test]
async fn deidentified_research_packages_are_consent_filtered_and_grep_clean() {
    let Some(c) = Ctx::new().await else { return };
    let (_, clin) = c.token(Role::Clinician).await;
    let (_, owner) = c.token(Role::Owner).await;
    let f = c.case_fixture(&clin).await;
    let done = c
        .export_and_run(&owner, case_body("deidentified", "research", f.participant))
        .await;
    assert_eq!(done["status"], "succeeded", "{done}");
    // XC-5: research keeps only the session with recorded research consent.
    assert_eq!(done["row_count"], 1);
    assert_eq!(done["excluded"]["no_research_consent"], 1, "{done}");
    let id = done["id"].as_str().unwrap();
    assert_eq!(c.export_items(id).await, vec![SESSION_1.to_owned()]);

    let zip = c.artifact(id).await;
    let names: Vec<String> = unzip::entries(&zip).into_iter().map(|e| e.name).collect();
    assert!(
        !names
            .iter()
            .any(|n| n.starts_with("notes/") || n.starts_with("attachments/")),
        "{names:?}"
    );
    let participant = f.participant.to_string();
    assert_archive_clean(
        &zip,
        &[
            TOKEN,
            &participant,
            SESSION_1,
            SESSION_2,
            MRN,
            NOTE,
            RECIPIENT,
            CONSENT_REF,
            // Exact start times never appear: de-identified dates are months.
            "2026-10-01T10:00",
            "2026-10-03T10:00",
        ],
    );
    let manifest: Value = serde_json::from_slice(&unzip::entries(&zip)[0].data).unwrap();
    assert_eq!(manifest["recipient"]["label"], "de-identified recipient");
    assert!(manifest.get("consent").is_none() || manifest["consent"].is_null());
    assert!(c.audit_rows("patient_ref.export_read").await.is_empty());

    // Control: the identified package of the same case carries them, so the search above
    // is not vacuous.
    let ident = c.ready_case(&clin, f.participant).await;
    let zip = c.artifact(ident["id"].as_str().unwrap()).await;
    assert_archive_has(&zip, &[TOKEN, SESSION_1, NOTE, MRN, RECIPIENT, CONSENT_REF]);
    c.db.cleanup().await;
}

#[tokio::test]
async fn a_package_whose_sessions_are_all_excluded_fails_with_a_reason() {
    let Some(c) = Ctx::new().await else { return };
    let (_, owner) = c.token(Role::Owner).await;
    let p = c.participant(c.org, "TOKEN-NO-CONSENT").await;
    c.exportable_session(
        p,
        "TOKEN-NO-CONSENT",
        "SESS-NC",
        "2026-10-01T10:00:00Z",
        None,
    )
    .await;
    let done = c
        .export_and_run(&owner, case_body("deidentified", "research", p))
        .await;
    assert_eq!(done["status"], "failed", "{done}");
    assert!(
        done["error"]
            .as_str()
            .unwrap()
            .starts_with("no-eligible-sessions"),
        "{done}"
    );
    c.db.cleanup().await;
}
