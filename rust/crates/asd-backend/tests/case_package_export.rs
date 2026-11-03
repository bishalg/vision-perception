//! owner: WP-I.13
//!
//! `case_package` exports (SHR-1..7, acceptance 6 and 9 backend parts): the request
//! rules, the identified package (notes, MRN, consent, recipient; idempotent bytes on a
//! retry), the de-identified package (consent-filtered for research, nothing identifying
//! in any entry), the export row and its audit; and (WP-I.13b) the participant's
//! encounters and attachments: present in identified packages, encounter summaries only
//! when the requester could read clinical content, attachments only when asked for, and
//! none of it in de-identified packages. Needs `DATABASE_URL`.

mod common;
mod export_support;
mod participant_support;

use asd_contract::api_v1::Role;
use axum::body::Body;
use export_support::case::*;
use export_support::unzip;
use http::{Method, Request, StatusCode};
use participant_support::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
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

// ---- WP-I.13b: encounters and attachments (SHR-1, REC-3, REC-6) ----------------------------

const ENC_REASON: &str = "ENC-REASON-SENTINEL";
const ENC_SUMMARY: &str = "ENC-SUMMARY-SENTINEL calm, engaged";
const ATTACHMENT_MARKER: &[u8] = b"ATTACHMENT-BYTES-SENTINEL";

/// A small "PDF": the magic number and a recognisable marker.
fn pdf() -> Vec<u8> {
    let mut v = b"%PDF-1.4\n".to_vec();
    v.extend_from_slice(ATTACHMENT_MARKER);
    v.resize(256, b' ');
    v
}

/// An encounter of `participant` with a reason and a summary, linked to `SESSION_1`,
/// written by the clinician `clin_id`; returns its id.
async fn encounter(c: &Ctx, participant: Uuid, clin_id: Uuid, clin: &str) -> String {
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/participants/{participant}/encounters"),
            Some(json!({
                "kind": "follow_up_visit",
                "occurred_at": "2026-10-05T08:30:00Z",
                "clinician_id": clin_id.to_string(),
                "reason": ENC_REASON,
                "summary": ENC_SUMMARY,
                "session_ids": [SESSION_1],
            })),
            clin,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    b["id"].as_str().unwrap().to_owned()
}

/// Uploads [`pdf`] as a referral of `participant` (named with PHI-ish text the backend
/// sanitises); returns `(attachment id, stored file name)`.
async fn attachment(c: &Ctx, participant: Uuid, bearer: &str) -> (String, String) {
    let bytes = pdf();
    let (s, made) = c
        .call(
            Method::POST,
            &format!("/v1/participants/{participant}/attachments"),
            Some(
                json!({"kind": "referral", "file_name": "Referral letter.pdf",
                        "media_type": "application/pdf", "byte_size": bytes.len()}),
            ),
            bearer,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    let req = Request::builder()
        .method(Method::PUT)
        .uri(made["upload"]["url"].as_str().unwrap())
        .header("content-type", "application/pdf")
        .header("authorization", format!("Bearer {bearer}"))
        .body(Body::from(bytes))
        .unwrap();
    let res = c.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    (
        made["attachment"]["id"].as_str().unwrap().to_owned(),
        made["attachment"]["file_name"].as_str().unwrap().to_owned(),
    )
}

fn entry<'a>(entries: &'a [unzip::Entry], name: &str) -> Option<&'a unzip::Entry> {
    entries.iter().find(|e| e.name == name)
}

/// `notes/notes.json` of an identified package.
fn notes_json(zip: &[u8]) -> Value {
    let entries = unzip::entries(zip);
    serde_json::from_slice(&entry(&entries, "notes/notes.json").expect("notes").data).unwrap()
}

/// Requests a case package, optionally rewrites the stored row before the job runs (as
/// if requested by someone else), runs the queue and returns the finished export.
async fn case_with(c: &Ctx, bearer: &str, body: Value, rewrite: Option<&'static str>) -> Value {
    let (s, made) = c.request_export(bearer, body).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{made}");
    if let Some(sql) = rewrite {
        sqlx::query(sql)
            .bind(uuid(&made["id"]))
            .execute(&c.db.pool)
            .await
            .unwrap();
    }
    c.run_jobs().await;
    let (_, done) = c
        .call(
            Method::GET,
            &format!("/v1/exports/{}", made["id"].as_str().unwrap()),
            None,
            bearer,
        )
        .await;
    assert_eq!(done["status"], "succeeded", "{done}");
    done
}

#[tokio::test]
async fn identified_packages_carry_encounters_and_requested_attachments() {
    let Some(c) = Ctx::new().await else { return };
    let (clin_id, clin) = c.token(Role::Clinician).await;
    sqlx::query("UPDATE users SET display_name = 'Dr Encounter' WHERE id = $1")
        .bind(clin_id)
        .execute(&c.db.pool)
        .await
        .unwrap();
    let f = c.case_fixture(&clin).await;
    let enc = encounter(&c, f.participant, clin_id, &clin).await;
    let (att, file_name) = attachment(&c, f.participant, &clin).await;
    // Another participant's attachment never travels.
    let (other_att, _) = attachment(&c, f.other, &clin).await;

    let mut body = case_body("identified", "clinical_care", f.participant);
    body["case"]["include_attachments"] = json!(true);
    let done = case_with(&c, &clin, body, None).await;
    let zip = c.artifact(done["id"].as_str().unwrap()).await;
    let entries = unzip::entries(&zip);
    let path = format!("attachments/{att}-{file_name}");
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    let stored = entry(&entries, &path).unwrap_or_else(|| panic!("{path} in {names:?}"));
    assert_eq!(stored.data, pdf(), "the decrypted upload, byte for byte");
    assert_eq!(
        names
            .iter()
            .filter(|n| n.starts_with("attachments/"))
            .count(),
        1,
        "{names:?}"
    );
    assert_archive_clean(&zip, &[&other_att]);
    let manifest: Value = serde_json::from_slice(&entries[0].data).unwrap();
    let listed = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == path.as_str())
        .expect("the attachment is listed in the manifest");
    assert_eq!(listed["media_type"], "application/pdf");
    assert_eq!(listed["sha256"], hex(&Sha256::digest(pdf())));

    let notes = notes_json(&zip);
    let encounters = notes["encounters"].as_array().unwrap();
    assert_eq!(encounters.len(), 1, "{notes}");
    let e = &encounters[0];
    assert_eq!(e["id"], enc);
    assert_eq!(e["kind"], "follow_up_visit");
    assert_eq!(e["status"], "open");
    assert_eq!(e["reason"], ENC_REASON);
    assert_eq!(e["summary"], ENC_SUMMARY);
    assert_eq!(e["clinician_display"], "Dr Encounter", "{e}");
    assert_eq!(e["session_ids"], json!([SESSION_1]));

    // Without include_attachments: the encounters, no attachment.
    let done = c.ready_case(&clin, f.participant).await;
    let zip = c.artifact(done["id"].as_str().unwrap()).await;
    assert!(!unzip::entries(&zip)
        .iter()
        .any(|e| e.name.starts_with("attachments/")));
    assert_eq!(notes_json(&zip)["encounters"][0]["summary"], ENC_SUMMARY);
    c.db.cleanup().await;
}

#[tokio::test]
async fn encounter_summaries_and_attachments_need_clinical_content_read() {
    let Some(c) = Ctx::new().await else { return };
    let (clin_id, clin) = c.token(Role::Clinician).await;
    let f = c.case_fixture(&clin).await;
    encounter(&c, f.participant, clin_id, &clin).await;
    attachment(&c, f.participant, &clin).await;
    // Every role that may share a case today reads clinical content; the stored flag is
    // what the job honours, so a requester without it is simulated on the row.
    let mut body = case_body("identified", "clinical_care", f.participant);
    body["case"]["include_attachments"] = json!(true);
    let done = case_with(
        &c,
        &clin,
        body,
        Some("UPDATE exports SET include_clinical_content = false WHERE id = $1"),
    )
    .await;
    let zip = c.artifact(done["id"].as_str().unwrap()).await;
    assert!(!unzip::entries(&zip)
        .iter()
        .any(|e| e.name.starts_with("attachments/")));
    let notes = notes_json(&zip);
    let e = &notes["encounters"][0];
    assert_eq!(e["reason"], ENC_REASON, "the header still travels: {notes}");
    assert!(e["summary"].is_null(), "{e}");
    assert_archive_clean(
        &zip,
        &[
            ENC_SUMMARY,
            std::str::from_utf8(ATTACHMENT_MARKER).unwrap(),
            NOTE,
        ],
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn deidentified_packages_carry_no_encounter_text_and_no_attachment() {
    let Some(c) = Ctx::new().await else { return };
    let (clin_id, clin) = c.token(Role::Clinician).await;
    let f = c.case_fixture(&clin).await;
    let enc = encounter(&c, f.participant, clin_id, &clin).await;
    let (att, _) = attachment(&c, f.participant, &clin).await;
    let done = case_with(
        &c,
        &clin,
        case_body("deidentified", "clinical_care", f.participant),
        None,
    )
    .await;
    let zip = c.artifact(done["id"].as_str().unwrap()).await;
    let names: Vec<String> = unzip::entries(&zip).into_iter().map(|e| e.name).collect();
    assert!(
        !names
            .iter()
            .any(|n| n.starts_with("notes/") || n.starts_with("attachments/")),
        "{names:?}"
    );
    assert_archive_clean(
        &zip,
        &[
            &enc,
            &att,
            ENC_REASON,
            ENC_SUMMARY,
            std::str::from_utf8(ATTACHMENT_MARKER).unwrap(),
            "2026-10-05T08:30",
        ],
    );

    // Control: the identified package of the same case carries them.
    let mut body = case_body("identified", "clinical_care", f.participant);
    body["case"]["include_attachments"] = json!(true);
    let done = case_with(&c, &clin, body, None).await;
    let zip = c.artifact(done["id"].as_str().unwrap()).await;
    assert_archive_has(
        &zip,
        &[
            &enc,
            &att,
            ENC_REASON,
            ENC_SUMMARY,
            std::str::from_utf8(ATTACHMENT_MARKER).unwrap(),
        ],
    );
    c.db.cleanup().await;
}
