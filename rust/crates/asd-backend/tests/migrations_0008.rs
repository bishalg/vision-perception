//! owner: WP-C.9
//!
//! MIG-2 `0008_clinical.sql`: clinical tables, `users` profile columns, `review_items`
//! ciphertext columns, and the database-side guarantees: a signed review version never
//! changes except `signed -> superseded` (SIGN-7); signatures and notes are append-only
//! (SIGN-7, NOTE-1); finalized letters are immutable (NOTE-3); a cosign by the first signer
//! is refused (SIGN-6). Needs `DATABASE_URL`; skipped otherwise.

mod common;

use std::collections::BTreeSet;

use common::TestDb;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    db: TestDb,
    org: Uuid,
    participant: Uuid,
    session: String,
    alice: Uuid,
    bob: Uuid,
}

async fn user(pool: &PgPool, org: Uuid, email: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (org_id, email, pw_hash, role) VALUES ($1, $2, 'x', 'clinician') RETURNING id",
    )
    .bind(org)
    .bind(email)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn setup() -> Option<Fx> {
    let db = TestDb::migrated().await?;
    let pool = &db.pool;
    let org: Uuid = sqlx::query_scalar(
        "INSERT INTO organizations (name, kind) VALUES ('Clinic', 'clinic') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let participant: Uuid = sqlx::query_scalar(
        "INSERT INTO participants (org_id, token) VALUES ($1, 'child-A') RETURNING id",
    )
    .bind(org)
    .fetch_one(pool)
    .await
    .unwrap();
    let session = "S-CLIN-1".to_owned();
    sqlx::query(
        "INSERT INTO sessions (id, org_id, participant_id, participant_token, capture_source, \
           started_at, duration_sec, schema_version, quality_verdict, valid_frame_ratio, \
           social_preference, attention_flag, summary) \
         VALUES ($1, $2, $3, 'child-A', 'truedepth', now(), 30, '1.6', 'optimal', 0.9, 0.6, 'red', '{}')",
    )
    .bind(&session)
    .bind(org)
    .bind(participant)
    .execute(pool)
    .await
    .unwrap();
    let alice = user(pool, org, "alice@example.org").await;
    let bob = user(pool, org, "bob@example.org").await;
    Some(Fx {
        db,
        org,
        participant,
        session,
        alice,
        bob,
    })
}

fn ct() -> Vec<u8> {
    vec![1u8; 1 + 12 + 4 + 16]
}

impl Fx {
    fn pool(&self) -> &PgPool {
        &self.db.pool
    }

    async fn review(&self) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO clinical_reviews (org_id, session_id, created_by) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(self.org)
        .bind(&self.session)
        .bind(self.alice)
        .fetch_one(self.pool())
        .await
        .unwrap()
    }

    /// A draft version, then signed by alice; returns the version id.
    async fn signed_version(&self, review: Uuid, version_no: i32) -> Uuid {
        let v: Uuid = sqlx::query_scalar(
            "INSERT INTO clinical_review_versions \
               (review_id, org_id, version_no, disposition, impression_ct, impression_key_id, created_by) \
             VALUES ($1, $2, $3, 'monitor', $4, gen_random_uuid(), $5) RETURNING id",
        )
        .bind(review)
        .bind(self.org)
        .bind(version_no)
        .bind(ct())
        .bind(self.alice)
        .fetch_one(self.pool())
        .await
        .unwrap();
        sqlx::query(
            "UPDATE clinical_review_versions \
             SET status = 'signed', content_hash = $2, signed_at = now() WHERE id = $1",
        )
        .bind(v)
        .bind(vec![7u8; 32])
        .execute(self.pool())
        .await
        .expect("a draft may be signed");
        sqlx::query(
            "INSERT INTO clinical_signatures \
               (org_id, review_id, version_id, signer_id, role, meaning, content_hash, signer_display_name) \
             VALUES ($1, $2, $3, $4, 'sign', 'I approve', $5, 'Dr Alice')",
        )
        .bind(self.org)
        .bind(review)
        .bind(v)
        .bind(self.alice)
        .bind(vec![7u8; 32])
        .execute(self.pool())
        .await
        .unwrap();
        v
    }

    async fn exec(&self, sql: &'static str, id: Uuid) -> Result<u64, String> {
        sqlx::query(sql)
            .bind(id)
            .execute(self.pool())
            .await
            .map(|r| r.rows_affected())
            .map_err(|e| e.to_string())
    }
}

async fn columns(pool: &PgPool, table: &str) -> BTreeSet<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT column_name::text FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = $1",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .collect()
}

#[tokio::test]
async fn tables_and_columns_exist() {
    let Some(f) = setup().await else { return };
    for (table, cols) in [
        (
            "org_data_keys",
            &["org_id", "key_id", "wrapped_dek", "kek_id", "status"][..],
        ),
        (
            "patient_refs",
            &[
                "participant_id",
                "org_id",
                "assigning_authority",
                "mrn_ct",
                "mrn_key_id",
                "mrn_bidx",
            ],
        ),
        (
            "clinical_reviews",
            &[
                "session_id",
                "org_id",
                "state",
                "row_version",
                "assigned_to",
            ],
        ),
        (
            "clinical_review_versions",
            &[
                "review_id",
                "version_no",
                "status",
                "disposition",
                "impression_ct",
                "content_hash",
            ],
        ),
        (
            "clinical_signatures",
            &["version_id", "signer_id", "role", "content_hash"],
        ),
        (
            "clinical_notes",
            &["participant_id", "body_ct", "body_key_id", "corrects_id"],
        ),
        (
            "referral_letters",
            &[
                "participant_id",
                "kind",
                "status",
                "recipient_ct",
                "body_ct",
            ],
        ),
        ("users", &["display_name", "professional_registration"]),
        ("review_items", &["note", "note_ct", "note_key_id"]),
    ] {
        let have = columns(f.pool(), table).await;
        for c in cols {
            assert!(have.contains(*c), "{table}.{c} missing: {have:?}");
        }
    }
    let indexes: BTreeSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT indexname::text FROM pg_indexes WHERE schemaname = current_schema()",
    )
    .fetch_all(f.pool())
    .await
    .unwrap()
    .into_iter()
    .collect();
    for idx in [
        "clinical_reviews_org_state_idx",
        "clinical_notes_participant_created_idx",
        "patient_refs_org_authority_bidx_key",
    ] {
        assert!(indexes.contains(idx), "{idx} missing");
    }
    f.db.cleanup().await;
}

#[tokio::test]
async fn signed_version_is_immutable_except_superseded() {
    let Some(f) = setup().await else { return };
    let review = f.review().await;
    let v = f.signed_version(review, 1).await;

    for sql in [
        "UPDATE clinical_review_versions SET disposition = 'refer_for_evaluation' WHERE id = $1",
        "UPDATE clinical_review_versions SET impression_ct = '\\x00' WHERE id = $1",
        "UPDATE clinical_review_versions SET content_hash = decode(repeat('00', 32), 'hex') WHERE id = $1",
        "UPDATE clinical_review_versions SET status = 'draft' WHERE id = $1",
        "UPDATE clinical_review_versions SET status = 'superseded', disposition = 'monitor', \
            version_no = 9 WHERE id = $1",
        "DELETE FROM clinical_review_versions WHERE id = $1",
    ] {
        let err = f.exec(sql, v).await.unwrap_err();
        assert!(err.contains("immutable"), "{sql}: {err}");
    }
    // The one allowed change.
    f.exec(
        "UPDATE clinical_review_versions SET status = 'superseded', superseded_at = now() WHERE id = $1",
        v,
    )
    .await
    .unwrap();
    // And nothing after that.
    let err = f
        .exec(
            "UPDATE clinical_review_versions SET status = 'signed' WHERE id = $1",
            v,
        )
        .await
        .unwrap_err();
    assert!(err.contains("immutable"), "{err}");
    let err = f
        .exec("DELETE FROM clinical_review_versions WHERE id = $1", v)
        .await
        .unwrap_err();
    assert!(err.contains("immutable"), "{err}");
    f.db.cleanup().await;
}

#[tokio::test]
async fn drafts_stay_editable_and_a_signed_version_needs_its_hash() {
    let Some(f) = setup().await else { return };
    let review = f.review().await;
    let v: Uuid = sqlx::query_scalar(
        "INSERT INTO clinical_review_versions (review_id, org_id, version_no, created_by) \
         VALUES ($1, $2, 1, $3) RETURNING id",
    )
    .bind(review)
    .bind(f.org)
    .bind(f.alice)
    .fetch_one(f.pool())
    .await
    .unwrap();
    f.exec(
        "UPDATE clinical_review_versions SET disposition = 'monitor' WHERE id = $1",
        v,
    )
    .await
    .unwrap();
    let err = f
        .exec(
            "UPDATE clinical_review_versions SET status = 'signed' WHERE id = $1",
            v,
        )
        .await
        .unwrap_err();
    assert!(err.contains("check constraint"), "{err}");
    // Ciphertext and key id come together.
    let err = f
        .exec(
            "UPDATE clinical_review_versions SET impression_ct = '\\x01' WHERE id = $1",
            v,
        )
        .await
        .unwrap_err();
    assert!(err.contains("check constraint"), "{err}");
    f.db.cleanup().await;
}

#[tokio::test]
async fn signatures_are_append_only() {
    let Some(f) = setup().await else { return };
    let review = f.review().await;
    let v = f.signed_version(review, 1).await;
    let sig: Uuid = sqlx::query_scalar("SELECT id FROM clinical_signatures WHERE version_id = $1")
        .bind(v)
        .fetch_one(f.pool())
        .await
        .unwrap();
    for sql in [
        "UPDATE clinical_signatures SET meaning = 'changed' WHERE id = $1",
        "DELETE FROM clinical_signatures WHERE id = $1",
    ] {
        let err = f.exec(sql, sig).await.unwrap_err();
        assert!(err.contains("append-only"), "{sql}: {err}");
    }
    let err = sqlx::query("TRUNCATE clinical_signatures CASCADE")
        .execute(f.pool())
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("append-only"), "{err}");
    f.db.cleanup().await;
}

async fn cosign(
    f: &Fx,
    review: Uuid,
    version: Uuid,
    signer: Uuid,
    primary: Option<Uuid>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO clinical_signatures \
           (org_id, review_id, version_id, signer_id, role, meaning, content_hash, \
            signer_display_name, primary_signer_id) \
         VALUES ($1, $2, $3, $4, 'cosign', 'I approve', $5, 'Dr X', $6)",
    )
    .bind(f.org)
    .bind(review)
    .bind(version)
    .bind(signer)
    .bind(vec![7u8; 32])
    .bind(primary)
    .execute(f.pool())
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[tokio::test]
async fn cosign_by_the_signer_is_refused() {
    let Some(f) = setup().await else { return };
    let review = f.review().await;
    let v = f.signed_version(review, 1).await;

    // Trigger: the version's actual signer cannot cosign, whatever the row claims.
    let err = cosign(&f, review, v, f.alice, Some(f.alice))
        .await
        .unwrap_err();
    assert!(err.contains("cosigner must differ"), "{err}");
    // A forged `primary_signer_id` cannot get around it either.
    let err = cosign(&f, review, v, f.alice, Some(f.bob))
        .await
        .unwrap_err();
    assert!(err.contains("cosigner must differ"), "{err}");
    let err = cosign(&f, review, v, f.alice, None).await.unwrap_err();
    assert!(err.contains("cosigner must differ"), "{err}");
    // A different user is fine, with the primary filled in from the version.
    cosign(&f, review, v, f.bob, None).await.unwrap();
    let primary: Option<Uuid> = sqlx::query_scalar(
        "SELECT primary_signer_id FROM clinical_signatures WHERE version_id = $1 AND role = 'cosign'",
    )
    .bind(v)
    .fetch_one(f.pool())
    .await
    .unwrap();
    assert_eq!(primary, Some(f.alice));
    // One cosign per version.
    let carol = user(f.pool(), f.org, "carol@example.org").await;
    assert!(cosign(&f, review, v, carol, None).await.is_err());
    f.db.cleanup().await;
}

#[tokio::test]
async fn cosign_check_constraint_holds_without_the_trigger() {
    let Some(f) = setup().await else { return };
    let review = f.review().await;
    let v = f.signed_version(review, 1).await;
    // Defence in depth: with the trigger off (throwaway database), the CHECK alone still
    // refuses a cosign naming its own signer as the primary.
    sqlx::query("ALTER TABLE clinical_signatures DISABLE TRIGGER clinical_signatures_cosign")
        .execute(f.pool())
        .await
        .unwrap();
    let err = cosign(&f, review, v, f.alice, Some(f.alice))
        .await
        .unwrap_err();
    assert!(
        err.contains("clinical_signatures_cosigner_differs"),
        "{err}"
    );
    let err = cosign(&f, review, v, f.bob, None).await.unwrap_err();
    assert!(
        err.contains("clinical_signatures_cosigner_differs"),
        "{err}"
    );
    f.db.cleanup().await;
}

#[tokio::test]
async fn cosign_needs_a_signed_version() {
    let Some(f) = setup().await else { return };
    let review = f.review().await;
    let draft: Uuid = sqlx::query_scalar(
        "INSERT INTO clinical_review_versions (review_id, org_id, version_no, created_by) \
         VALUES ($1, $2, 1, $3) RETURNING id",
    )
    .bind(review)
    .bind(f.org)
    .bind(f.alice)
    .fetch_one(f.pool())
    .await
    .unwrap();
    let err = cosign(&f, review, draft, f.bob, None).await.unwrap_err();
    assert!(err.contains("no primary signature"), "{err}");
    f.db.cleanup().await;
}

async fn note(f: &Fx, corrects: Option<Uuid>) -> Result<Uuid, String> {
    sqlx::query_scalar(
        "INSERT INTO clinical_notes (org_id, participant_id, session_id, kind, body_ct, body_key_id, \
           corrects_id, author_id) \
         VALUES ($1, $2, $3, $4, $5, gen_random_uuid(), $6, $7) RETURNING id",
    )
    .bind(f.org)
    .bind(f.participant)
    .bind(&f.session)
    .bind(if corrects.is_some() { "correction" } else { "general" })
    .bind(ct())
    .bind(corrects)
    .bind(f.alice)
    .fetch_one(f.pool())
    .await
    .map_err(|e| e.to_string())
}

#[tokio::test]
async fn notes_are_append_only_and_corrections_are_new_rows() {
    let Some(f) = setup().await else { return };
    let original = note(&f, None).await.unwrap();
    for sql in [
        "UPDATE clinical_notes SET body_ct = '\\x00' WHERE id = $1",
        "DELETE FROM clinical_notes WHERE id = $1",
    ] {
        let err = f.exec(sql, original).await.unwrap_err();
        assert!(err.contains("append-only"), "{sql}: {err}");
    }
    let correction = note(&f, Some(original)).await.unwrap();
    assert_ne!(correction, original);
    // `kind` and `corrects_id` agree.
    let err = sqlx::query(
        "INSERT INTO clinical_notes (org_id, participant_id, kind, body_ct, body_key_id, author_id) \
         VALUES ($1, $2, 'correction', '\\x01', gen_random_uuid(), $3)",
    )
    .bind(f.org)
    .bind(f.participant)
    .bind(f.alice)
    .execute(f.pool())
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("check constraint"), "{err}");
    f.db.cleanup().await;
}

#[tokio::test]
async fn finalized_letters_are_immutable_drafts_are_not() {
    let Some(f) = setup().await else { return };
    let letter: Uuid = sqlx::query_scalar(
        "INSERT INTO referral_letters (org_id, participant_id, kind, session_ids, recipient_ct, \
           recipient_key_id, body_ct, body_key_id, author_id) \
         VALUES ($1, $2, 'referral', ARRAY[$3], $4, gen_random_uuid(), $4, gen_random_uuid(), $5) \
         RETURNING id",
    )
    .bind(f.org)
    .bind(f.participant)
    .bind(&f.session)
    .bind(ct())
    .bind(f.alice)
    .fetch_one(f.pool())
    .await
    .unwrap();
    f.exec(
        "UPDATE referral_letters SET kind = 'summary', row_version = row_version + 1 WHERE id = $1",
        letter,
    )
    .await
    .unwrap();
    // Finalizing needs the hash and the signer.
    let err = f
        .exec(
            "UPDATE referral_letters SET status = 'final' WHERE id = $1",
            letter,
        )
        .await
        .unwrap_err();
    assert!(err.contains("check constraint"), "{err}");
    sqlx::query(
        "UPDATE referral_letters SET status = 'final', content_hash = $2, finalized_by = $3, \
           finalized_at = now(), signer_display_name = 'Dr Alice' WHERE id = $1",
    )
    .bind(letter)
    .bind(vec![9u8; 32])
    .bind(f.alice)
    .execute(f.pool())
    .await
    .unwrap();
    for sql in [
        "UPDATE referral_letters SET body_ct = '\\x00' WHERE id = $1",
        "UPDATE referral_letters SET status = 'draft' WHERE id = $1",
        "DELETE FROM referral_letters WHERE id = $1",
    ] {
        let err = f.exec(sql, letter).await.unwrap_err();
        assert!(err.contains("immutable"), "{sql}: {err}");
    }
    f.db.cleanup().await;
}

#[tokio::test]
async fn one_review_per_session_and_one_mrn_per_participant_and_authority() {
    let Some(f) = setup().await else { return };
    f.review().await;
    let err = sqlx::query("INSERT INTO clinical_reviews (org_id, session_id) VALUES ($1, $2)")
        .bind(f.org)
        .bind(&f.session)
        .execute(f.pool())
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("duplicate key"), "{err}");

    let insert_ref = |participant: Uuid, bidx: Vec<u8>| {
        let pool = f.pool().clone();
        let org = f.org;
        async move {
            sqlx::query(
                "INSERT INTO patient_refs (org_id, participant_id, assigning_authority, mrn_ct, \
                   mrn_key_id, mrn_bidx) VALUES ($1, $2, 'TUTH', '\\x01', gen_random_uuid(), $3)",
            )
            .bind(org)
            .bind(participant)
            .bind(bidx)
            .execute(&pool)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
        }
    };
    insert_ref(f.participant, vec![1u8; 32]).await.unwrap();
    // A second MRN for the same participant.
    let err = insert_ref(f.participant, vec![2u8; 32]).await.unwrap_err();
    assert!(err.contains("duplicate key"), "{err}");
    // The same MRN (same blind index) for another participant in the same authority.
    let other: Uuid = sqlx::query_scalar(
        "INSERT INTO participants (org_id, token) VALUES ($1, 'child-B') RETURNING id",
    )
    .bind(f.org)
    .fetch_one(f.pool())
    .await
    .unwrap();
    let err = insert_ref(other, vec![1u8; 32]).await.unwrap_err();
    assert!(err.contains("patient_refs_org_authority_bidx_key"), "{err}");
    // A blind index is 32 bytes.
    let err = insert_ref(other, vec![1u8; 31]).await.unwrap_err();
    assert!(err.contains("check constraint"), "{err}");
    f.db.cleanup().await;
}

#[tokio::test]
async fn one_active_data_key_per_org() {
    let Some(f) = setup().await else { return };
    let insert = || {
        let pool = f.pool().clone();
        let org = f.org;
        async move {
            sqlx::query(
                "INSERT INTO org_data_keys (org_id, key_id, wrapped_dek, kek_id) \
                 VALUES ($1, gen_random_uuid(), '\\x01', 'kek')",
            )
            .bind(org)
            .execute(&pool)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
        }
    };
    insert().await.unwrap();
    let err = insert().await.unwrap_err();
    assert!(err.contains("org_data_keys_one_active_idx"), "{err}");
    f.db.cleanup().await;
}

#[tokio::test]
async fn profile_lengths_are_bounded() {
    let Some(f) = setup().await else { return };
    let ok = f
        .exec(
            "UPDATE users SET display_name = 'Dr Alice', professional_registration = 'NMC 12345' WHERE id = $1",
            f.alice,
        )
        .await;
    assert!(ok.is_ok(), "{ok:?}");
    for sql in [
        "UPDATE users SET display_name = repeat('a', 121) WHERE id = $1",
        "UPDATE users SET professional_registration = repeat('a', 61) WHERE id = $1",
    ] {
        let err = f.exec(sql, f.alice).await.unwrap_err();
        assert!(err.contains("check constraint"), "{sql}: {err}");
    }
    f.db.cleanup().await;
}
