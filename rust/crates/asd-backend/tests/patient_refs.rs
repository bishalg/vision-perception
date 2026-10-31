//! owner: WP-C.11
//!
//! Hospital MRN mapping (MRN-1..6): disabled unless the org opts in (409
//! `patient-ref-disabled`), put/get/delete round trip, encrypted storage, audit rows that
//! never hold the MRN, exact-match search by POST across normalisation variants, one MRN
//! per authority, and the `has_patient_ref` flag only for callers who may read the MRN.
//! Needs `DATABASE_URL`; skipped otherwise.

mod common;
mod participant_support;

use asd_contract::api_v1::Role;
use asd_contract::clinical::{PatientRef, PatientRefSearchResponse};
use common::TestDb;
use http::{Method, StatusCode};
use participant_support::*;
use serde_json::{json, Value};

const MRN: &str = "TU-10042/7";

fn url(participant: impl std::fmt::Display) -> String {
    format!("/v1/participants/{participant}/patient-ref")
}

#[tokio::test]
async fn mrn_endpoints_are_409_when_mode_is_none() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    let (_, clin) = c.token(Role::Clinician).await;
    let body = json!({"assigning_authority": "TUTH", "mrn": MRN});

    let calls = [
        (Method::PUT, url(p), Some(body.clone())),
        (Method::GET, url(p), None),
        (Method::DELETE, url(p), None),
        (
            Method::POST,
            format!("/v1/orgs/{}/patient-refs/search", c.org),
            Some(body.clone()),
        ),
    ];
    for (method, uri, b) in calls {
        let (s, resp) = c.call(method.clone(), &uri, b, &clin).await;
        assert_eq!(s, StatusCode::CONFLICT, "{method} {uri}: {resp}");
        assert_eq!(slug_of(&resp), "patient-ref-disabled");
    }
    // Explicit `none` behaves the same.
    c.set_mode(c.org, "none").await;
    let (s, resp) = c.call(Method::GET, &url(p), None, &clin).await;
    assert_eq!(s, StatusCode::CONFLICT, "{resp}");
    c.db.cleanup().await;
}

#[tokio::test]
async fn put_get_delete_round_trip_is_encrypted_and_audited_without_the_mrn() {
    let Some(c) = Ctx::new().await else { return };
    c.set_mode(c.org, "mrn").await;
    let p = c.participant(c.org, "P-1").await;
    let (clin_id, clin) = c.token(Role::Clinician).await;

    let (s, _) = c.call(Method::GET, &url(p), None, &clin).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "no MRN yet");

    let (s, created) = c
        .call(
            Method::PUT,
            &url(p),
            Some(json!({"assigning_authority": "TUTH", "mrn": MRN})),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{created}");
    assert!(created.get("mrn").is_none(), "put does not echo the MRN");

    let (s, got) = c.call(Method::GET, &url(p), None, &clin).await;
    assert_eq!(s, StatusCode::OK, "{got}");
    let got: PatientRef = serde_json::from_value(got).unwrap();
    assert_eq!(got.mrn, MRN);
    assert_eq!(got.assigning_authority, "TUTH");
    assert_eq!(got.participant_id, p.to_string());

    // The stored column is a ciphertext, not the MRN.
    let (ct, bidx): (Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT mrn_ct, mrn_bidx FROM patient_refs WHERE participant_id = $1")
            .bind(p)
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert!(!String::from_utf8_lossy(&ct).contains("10042"));
    assert_eq!(bidx.len(), 32);

    // Replacing the MRN keeps one row and returns 200.
    let (s, _) = c
        .call(
            Method::PUT,
            &url(p),
            Some(json!({"assigning_authority": "TUTH", "mrn": "TU-99"})),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let (_, got) = c.call(Method::GET, &url(p), None, &clin).await;
    assert_eq!(got["mrn"], "TU-99");

    let (s, _) = c.call(Method::DELETE, &url(p), None, &clin).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = c.call(Method::GET, &url(p), None, &clin).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = c.call(Method::DELETE, &url(p), None, &clin).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // One audit row per write and per read; none holds an MRN.
    assert_eq!(c.audit_rows("patient_ref.put").await.len(), 2);
    assert_eq!(c.audit_rows("patient_ref.read").await.len(), 2);
    assert_eq!(c.audit_rows("patient_ref.delete").await.len(), 1);
    for row in c.audit_rows("patient_ref.put").await {
        assert_eq!(row["target_type"], "patient_ref");
        assert_eq!(row["actor_id"], clin_id.to_string());
        assert_eq!(row["org_id"], c.org.to_string());
    }
    let dump = c.audit_dump().await;
    for secret in ["10042", "TU-99", "10042/7"] {
        assert!(!dump.contains(secret), "audit log holds {secret}");
    }
    let put = &c.audit_rows("patient_ref.put").await[0];
    assert_eq!(put["after"], json!({"set": true}));
    c.db.cleanup().await;
}

#[tokio::test]
async fn search_is_post_exact_and_normalisation_tolerant() {
    let Some(c) = Ctx::new().await else { return };
    c.set_mode(c.org, "mrn").await;
    let p = c.participant(c.org, "P-1").await;
    let other = c.participant(c.org, "P-2").await;
    let (_, clin) = c.token(Role::Clinician).await;
    c.call(
        Method::PUT,
        &url(p),
        Some(json!({"assigning_authority": "TUTH", "mrn": MRN})),
        &clin,
    )
    .await;
    c.call(
        Method::PUT,
        &url(other),
        Some(json!({"assigning_authority": "AIIMS", "mrn": MRN})),
        &clin,
    )
    .await;

    let search = format!("/v1/orgs/{}/patient-refs/search", c.org);
    // The MRN never travels in a URL: GET is not a route.
    let (s, _) = c.call(Method::GET, &search, None, &clin).await;
    assert_eq!(s, StatusCode::METHOD_NOT_ALLOWED);

    for (authority, mrn) in [
        ("TUTH", MRN),
        ("tuth", "tu-10042/7"),
        (" TUTH ", "  TU-10042/7 "),
    ] {
        let (s, body) = c
            .call(
                Method::POST,
                &search,
                Some(json!({"assigning_authority": authority, "mrn": mrn})),
                &clin,
            )
            .await;
        assert_eq!(s, StatusCode::OK, "{body}");
        let r: PatientRefSearchResponse = serde_json::from_value(body).unwrap();
        assert_eq!(
            r.participant_id,
            Some(p.to_string()),
            "{authority:?} {mrn:?}"
        );
    }
    // Same MRN, other authority -> the other participant; exact match only.
    let (_, body) = c
        .call(
            Method::POST,
            &search,
            Some(json!({"assigning_authority": "AIIMS", "mrn": MRN})),
            &clin,
        )
        .await;
    assert_eq!(body["participant_id"], other.to_string());
    for (authority, mrn) in [("TUTH", "TU-10042"), ("OTHER", MRN)] {
        let (s, body) = c
            .call(
                Method::POST,
                &search,
                Some(json!({"assigning_authority": authority, "mrn": mrn})),
                &clin,
            )
            .await;
        assert_eq!(s, StatusCode::OK);
        assert!(body["participant_id"].is_null(), "{body}");
    }
    // Searches are reads: audited, MRN-free.
    assert_eq!(c.audit_rows("patient_ref.search").await.len(), 6);
    assert!(!c.audit_dump().await.contains("10042"));
    c.db.cleanup().await;
}

#[tokio::test]
async fn duplicate_mrn_in_one_authority_is_409_and_isolated_per_org() {
    let Some(c) = Ctx::new().await else { return };
    c.set_mode(c.org, "mrn").await;
    let org2 = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    c.set_mode(org2, "mrn").await;
    let p1 = c.participant(c.org, "P-1").await;
    let p2 = c.participant(c.org, "P-2").await;
    let q1 = c.participant(org2, "Q-1").await;
    let (_, clin) = c.token(Role::Clinician).await;
    let (_, clin2) = c.token_in(Some(org2), Role::Clinician).await;
    let body = json!({"assigning_authority": "TUTH", "mrn": MRN});

    let (s, _) = c
        .call(Method::PUT, &url(p1), Some(body.clone()), &clin)
        .await;
    assert_eq!(s, StatusCode::CREATED);
    // A formatting variant is the same MRN.
    let (s, resp) = c
        .call(
            Method::PUT,
            &url(p2),
            Some(json!({"assigning_authority": "tuth", "mrn": "tu-10042/7"})),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{resp}");
    assert_eq!(slug_of(&resp), "conflict");
    assert!(!resp.to_string().contains("10042"));
    // Re-putting the same value on the same participant is fine.
    let (s, _) = c
        .call(Method::PUT, &url(p1), Some(body.clone()), &clin)
        .await;
    assert_eq!(s, StatusCode::OK);
    // Another org may use the same MRN.
    let (s, _) = c.call(Method::PUT, &url(q1), Some(body), &clin2).await;
    assert_eq!(s, StatusCode::CREATED);
    // And cannot reach this org's participant.
    let (s, _) = c.call(Method::GET, &url(p1), None, &clin2).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    c.db.cleanup().await;
}

#[tokio::test]
async fn put_validates_authority_and_mrn() {
    let Some(c) = Ctx::new().await else { return };
    c.set_mode(c.org, "mrn").await;
    let p = c.participant(c.org, "P-1").await;
    let (_, clin) = c.token(Role::Clinician).await;
    let long_auth = "A".repeat(41);
    let long_mrn = "9".repeat(65);
    for body in [
        json!({"assigning_authority": "", "mrn": MRN}),
        json!({"assigning_authority": "  ", "mrn": MRN}),
        json!({"assigning_authority": long_auth, "mrn": MRN}),
        json!({"assigning_authority": "TUTH", "mrn": ""}),
        json!({"assigning_authority": "TUTH", "mrn": long_mrn}),
        json!({"assigning_authority": "TUTH", "mrn": "12\n34"}),
        json!({"assigning_authority": "TUTH"}),
    ] {
        let (s, resp) = c
            .call(Method::PUT, &url(p), Some(body.clone()), &clin)
            .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}: {resp}");
        assert_eq!(slug_of(&resp), "validation-failed");
    }
    c.db.cleanup().await;
}

#[tokio::test]
async fn roles_and_has_patient_ref_visibility() {
    let Some(c) = Ctx::new().await else { return };
    c.set_mode(c.org, "mrn").await;
    let p = c.participant(c.org, "P-1").await;
    let (_, clin) = c.token(Role::Clinician).await;
    c.call(
        Method::PUT,
        &url(p),
        Some(json!({"assigning_authority": "TUTH", "mrn": MRN})),
        &clin,
    )
    .await;

    // Reviewer, viewer, researcher and superadmin may not read or write the MRN.
    for role in [
        Role::Reviewer,
        Role::Viewer,
        Role::Researcher,
        Role::Superadmin,
    ] {
        let (_, t) = c.token(role).await;
        let (s, _) = c.call(Method::GET, &url(p), None, &t).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{role:?} GET");
        let (s, _) = c
            .call(
                Method::PUT,
                &url(p),
                Some(json!({"assigning_authority": "X", "mrn": "1"})),
                &t,
            )
            .await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{role:?} PUT");
        let (s, _) = c.call(Method::DELETE, &url(p), None, &t).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{role:?} DELETE");
        let (s, _) = c
            .call(
                Method::POST,
                &format!("/v1/orgs/{}/patient-refs/search", c.org),
                Some(json!({"assigning_authority": "TUTH", "mrn": MRN})),
                &t,
            )
            .await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{role:?} search");
        // They still get the participant, just no detail about the MRN.
        if role != Role::Superadmin {
            let (s, body) = c
                .call(Method::GET, &format!("/v1/participants/{p}"), None, &t)
                .await;
            assert_eq!(s, StatusCode::OK, "{role:?}");
            assert!(body.get("has_patient_ref").is_none(), "{role:?}: {body}");
            assert!(!body.to_string().contains("10042"));
        }
    }
    // Admin may read and write the MRN (RBAC-2).
    let (_, admin) = c.token(Role::Admin).await;
    let (s, _) = c.call(Method::GET, &url(p), None, &admin).await;
    assert_eq!(s, StatusCode::OK);

    // Roles with PatientRefRead see the flag on the detail and in the list.
    let (s, body) = c
        .call(Method::GET, &format!("/v1/participants/{p}"), None, &clin)
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["has_patient_ref"], true);
    let q = c.participant(c.org, "P-2").await;
    let (_, body) = c
        .call(Method::GET, &format!("/v1/participants/{q}"), None, &clin)
        .await;
    assert_eq!(body["has_patient_ref"], false);
    let (_, list) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{}/participants", c.org),
            None,
            &clin,
        )
        .await;
    let flags: Vec<(String, Value)> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["participant_token"].as_str().unwrap().to_owned(),
                i["has_patient_ref"].clone(),
            )
        })
        .collect();
    assert!(flags.contains(&("P-1".into(), json!(true))), "{flags:?}");
    assert!(flags.contains(&("P-2".into(), json!(false))), "{flags:?}");
    let (_, viewer) = c.token(Role::Viewer).await;
    let (_, list) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{}/participants", c.org),
            None,
            &viewer,
        )
        .await;
    assert!(list["items"][0].get("has_patient_ref").is_none());
    c.db.cleanup().await;
}

#[tokio::test]
async fn unknown_participant_is_404() {
    let Some(c) = Ctx::new().await else { return };
    c.set_mode(c.org, "mrn").await;
    let (_, clin) = c.token(Role::Clinician).await;
    let body = json!({"assigning_authority": "TUTH", "mrn": MRN});
    let (s, _) = c
        .call(
            Method::PUT,
            &url(uuid::Uuid::new_v4()),
            Some(body.clone()),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = c
        .call(Method::PUT, &url("not-a-uuid"), Some(body), &clin)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    c.db.cleanup().await;
}

#[tokio::test]
async fn without_a_database_the_routes_require_auth() {
    let _ = TestDb::migrated; // keep the import used when DATABASE_URL is unset
    let (s, _, _) = common::send(
        common::router(common::state_without_db()),
        common::get(&format!(
            "/v1/participants/{}/patient-ref",
            uuid::Uuid::new_v4()
        )),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}
