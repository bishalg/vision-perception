//! owner: WP-C.9
//!
//! RBAC-1..3: the fourteen clinician-workspace and interop actions, row by row against the
//! RBAC-2 table, and RBAC-3 (the vendor's `superadmin` never gets clinical content, even
//! de-identified exports). No database needed.

use asd_backend::auth::extractors::UserPrincipal;
use asd_backend::auth::rbac::{self, Action, CLINICAL_CONTENT_ACTIONS};
use asd_backend::error::AppError;
use asd_contract::api_v1::Role;
use uuid::Uuid;

const COLUMNS: [Role; 7] = [
    Role::Owner,
    Role::Admin,
    Role::Clinician,
    Role::Researcher,
    Role::Reviewer,
    Role::Viewer,
    Role::Superadmin,
];

/// RBAC-2, copied from `technical.md` (owner, admin, clinician, researcher, reviewer,
/// viewer, superadmin).
const TABLE: &[(Action, [bool; 7])] = {
    const Y: bool = true;
    const N: bool = false;
    &[
        (Action::WorklistRead, [Y, Y, Y, N, N, N, Y]),
        (Action::ClinicalReviewWrite, [N, N, Y, N, N, N, N]),
        (Action::ClinicalSign, [N, N, Y, N, N, N, N]),
        (Action::ClinicalCosign, [N, N, Y, N, N, N, N]),
        (Action::ClinicalContentRead, [Y, N, Y, N, N, N, N]),
        (Action::ClinicalNoteWrite, [N, N, Y, N, N, N, N]),
        (Action::LetterWrite, [N, N, Y, N, N, N, N]),
        (Action::PatientRefRead, [Y, Y, Y, N, N, N, N]),
        (Action::PatientRefWrite, [Y, Y, Y, N, N, N, N]),
        (Action::ExportRead, [Y, Y, Y, Y, N, N, Y]),
        (Action::ExportDeidentified, [Y, Y, Y, Y, N, N, N]),
        (Action::ExportIdentified, [Y, N, Y, N, N, N, N]),
        (Action::IntegrationRead, [Y, Y, N, N, N, N, Y]),
        (Action::IntegrationManage, [Y, Y, N, N, N, N, N]),
    ]
};

#[test]
fn rbac2_table_row_by_row() {
    assert_eq!(TABLE.len(), 14, "RBAC-1 lists fourteen actions");
    for (action, row) in TABLE {
        assert!(
            Action::ALL.contains(action),
            "{action:?} not in Action::ALL"
        );
        assert_eq!(
            rbac::MATRIX.iter().filter(|(a, _)| a == action).count(),
            1,
            "{action:?} needs exactly one MATRIX row"
        );
        for (role, want) in COLUMNS.iter().zip(row) {
            assert_eq!(
                rbac::allows(*role, *action),
                *want,
                "{role:?} on {action:?}"
            );
        }
    }
}

#[test]
fn rbac3_superadmin_is_denied_every_clinical_content_action() {
    let expected = [
        Action::ClinicalReviewWrite,
        Action::ClinicalSign,
        Action::ClinicalCosign,
        Action::ClinicalContentRead,
        Action::ClinicalNoteWrite,
        Action::LetterWrite,
        Action::PatientRefRead,
        Action::PatientRefWrite,
        Action::ExportDeidentified,
        Action::ExportIdentified,
        Action::IntegrationManage,
    ];
    assert_eq!(CLINICAL_CONTENT_ACTIONS.len(), expected.len());
    for action in expected {
        assert!(CLINICAL_CONTENT_ACTIONS.contains(&action), "{action:?}");
    }
    let root = UserPrincipal {
        user_id: Uuid::new_v4(),
        org_id: None,
        role: Role::Superadmin,
    };
    for action in CLINICAL_CONTENT_ACTIONS {
        assert!(!rbac::allows(Role::Superadmin, *action), "{action:?}");
        // Through `authorize` too, on any org (the carve-out is not a per-handler check).
        assert!(
            matches!(
                rbac::authorize(&root, *action, Some(Uuid::new_v4())),
                Err(AppError::Forbidden)
            ),
            "{action:?}"
        );
    }
}

#[test]
fn superadmin_keeps_every_non_clinical_action() {
    for action in Action::ALL {
        let want = !CLINICAL_CONTENT_ACTIONS.contains(action);
        assert_eq!(rbac::allows(Role::Superadmin, *action), want, "{action:?}");
    }
}

#[test]
fn clinical_content_never_reaches_reviewers_or_viewers() {
    for action in CLINICAL_CONTENT_ACTIONS {
        for role in [Role::Reviewer, Role::Viewer] {
            assert!(!rbac::allows(role, *action), "{role:?} {action:?}");
        }
    }
    // Operations (admin) cannot read notes or impressions (ADR-0008 D2).
    assert!(!rbac::allows(Role::Admin, Action::ClinicalContentRead));
}

#[test]
fn read_only_classification_of_the_new_actions() {
    for action in [
        Action::WorklistRead,
        Action::ClinicalContentRead,
        Action::PatientRefRead,
        Action::ExportRead,
        Action::IntegrationRead,
    ] {
        assert!(action.is_read_only(), "{action:?}");
    }
    for action in [
        Action::ClinicalReviewWrite,
        Action::ClinicalSign,
        Action::ClinicalCosign,
        Action::ClinicalNoteWrite,
        Action::LetterWrite,
        Action::PatientRefWrite,
        Action::ExportDeidentified,
        Action::ExportIdentified,
        Action::IntegrationManage,
    ] {
        assert!(!action.is_read_only(), "{action:?}");
    }
}
