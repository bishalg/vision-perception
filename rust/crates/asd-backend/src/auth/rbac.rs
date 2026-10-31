//! owner: WP-B.2
//!
//! AUTH-4: roles `owner, admin, clinician, researcher, reviewer, viewer` scoped to one org,
//! plus the global `superadmin`. [`MATRIX`] is the single table of who may do what; the
//! [`super::extractors::Require`] extractor applies it per route (`Require<perm::OrgUpdate>`).
//!
//! Scoping rule: a resource of another org answers **404**, never 403 (the caller learns
//! nothing about it). A 403 is returned only for a role that may not perform the action at
//! all, which does not depend on the resource.
//!
//! Route packages add rows here (and a marker in [`perm`]) for their own actions; the
//! session/review/config rows below are the defaults B.3/B.4/B.5 build on.
//!
//! WP-C.9 (RBAC-1..3, ADR-0008 D1/D2): the fourteen clinician-workspace and interop actions.
//! `superadmin` (the vendor) is allowed everything **except** [`CLINICAL_CONTENT_ACTIONS`]:
//! it operates the system but never reads clinical content or takes copies of a clinic's
//! data, even de-identified. The carve-out lives in [`allows`], so no route can forget it.

use asd_contract::api_v1::Role;
use uuid::Uuid;

use super::extractors::UserPrincipal;
use crate::error::{AppError, AppResult};

/// Everything a console user can be authorised for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    OrgRead,
    OrgUpdate,
    UserRead,
    UserManage,
    DeviceRead,
    EnrolCodeCreate,
    DeviceRevoke,
    AuditRead,
    SessionRead,
    ParticipantRead,
    MetricsRead,
    ReviewRead,
    ReviewWrite,
    ConfigRead,
    ConfigWrite,
    ConfigPublish,
    /// Create, approve, publish and retire the global `config_documents` (no org: every
    /// tenant's ingest reads them). No org role; superadmin only (security audit
    /// finding 2). Composing an org's bundle stays `ConfigPublish`.
    ConfigGlobalManage,
    JobRun,
    ParticipantErase,
    // ---- WP-C.9: clinician workspace and interop (RBAC-1)
    WorklistRead,
    ClinicalReviewWrite,
    ClinicalSign,
    ClinicalCosign,
    ClinicalContentRead,
    ClinicalNoteWrite,
    LetterWrite,
    PatientRefRead,
    PatientRefWrite,
    ExportRead,
    ExportDeidentified,
    ExportIdentified,
    IntegrationRead,
    IntegrationManage,
}

impl Action {
    pub const ALL: &'static [Action] = &[
        Action::OrgRead,
        Action::OrgUpdate,
        Action::UserRead,
        Action::UserManage,
        Action::DeviceRead,
        Action::EnrolCodeCreate,
        Action::DeviceRevoke,
        Action::AuditRead,
        Action::SessionRead,
        Action::ParticipantRead,
        Action::MetricsRead,
        Action::ReviewRead,
        Action::ReviewWrite,
        Action::ConfigRead,
        Action::ConfigWrite,
        Action::ConfigPublish,
        Action::ConfigGlobalManage,
        Action::JobRun,
        Action::ParticipantErase,
        Action::WorklistRead,
        Action::ClinicalReviewWrite,
        Action::ClinicalSign,
        Action::ClinicalCosign,
        Action::ClinicalContentRead,
        Action::ClinicalNoteWrite,
        Action::LetterWrite,
        Action::PatientRefRead,
        Action::PatientRefWrite,
        Action::ExportRead,
        Action::ExportDeidentified,
        Action::ExportIdentified,
        Action::IntegrationRead,
        Action::IntegrationManage,
    ];

    /// Actions that change nothing.
    pub fn is_read_only(self) -> bool {
        matches!(
            self,
            Action::OrgRead
                | Action::UserRead
                | Action::DeviceRead
                | Action::AuditRead
                | Action::SessionRead
                | Action::ParticipantRead
                | Action::MetricsRead
                | Action::ReviewRead
                | Action::ConfigRead
                | Action::WorklistRead
                | Action::ClinicalContentRead
                | Action::PatientRefRead
                | Action::ExportRead
                | Action::IntegrationRead
        )
    }
}

/// RBAC-3: actions `superadmin` never gets. Reading or writing clinical content, the MRN,
/// any export of clinic data (de-identified included) and managing a clinic's integrations.
/// There is no break-glass path in v1.
pub const CLINICAL_CONTENT_ACTIONS: &[Action] = &[
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

use Role::{Admin, Clinician, Owner, Researcher, Reviewer, Viewer};

const ALL_ORG_ROLES: &[Role] = &[Owner, Admin, Clinician, Researcher, Reviewer, Viewer];

/// The RBAC matrix: action -> org roles allowed. `superadmin` is not listed: it is allowed
/// every action except [`CLINICAL_CONTENT_ACTIONS`].
pub const MATRIX: &[(Action, &[Role])] = &[
    (Action::OrgRead, ALL_ORG_ROLES),
    (Action::OrgUpdate, &[Owner, Admin]),
    (Action::UserRead, &[Owner, Admin]),
    (Action::UserManage, &[Owner, Admin]),
    (Action::DeviceRead, &[Owner, Admin, Clinician]),
    (Action::EnrolCodeCreate, &[Owner, Admin, Clinician]),
    (Action::DeviceRevoke, &[Owner, Admin]),
    (Action::AuditRead, &[Owner, Admin]),
    (Action::SessionRead, ALL_ORG_ROLES),
    (Action::ParticipantRead, ALL_ORG_ROLES),
    (Action::MetricsRead, ALL_ORG_ROLES),
    (Action::ReviewRead, ALL_ORG_ROLES),
    (Action::ReviewWrite, &[Owner, Admin, Clinician, Reviewer]),
    (Action::ConfigRead, &[Owner, Admin, Clinician, Researcher]),
    (Action::ConfigWrite, &[Owner, Admin, Researcher]),
    (Action::ConfigPublish, &[Owner, Admin]),
    // Global documents: superadmin only (it is allowed every non-clinical action).
    (Action::ConfigGlobalManage, &[]),
    (Action::JobRun, &[Owner, Admin, Researcher]),
    (Action::ParticipantErase, &[Owner, Admin]),
    // WP-C.9, RBAC-2. Admin (operations) never reads clinical content (ADR-0008 D2).
    (Action::WorklistRead, &[Owner, Admin, Clinician]),
    (Action::ClinicalReviewWrite, &[Clinician]),
    (Action::ClinicalSign, &[Clinician]),
    (Action::ClinicalCosign, &[Clinician]),
    (Action::ClinicalContentRead, &[Owner, Clinician]),
    (Action::ClinicalNoteWrite, &[Clinician]),
    (Action::LetterWrite, &[Clinician]),
    (Action::PatientRefRead, &[Owner, Admin, Clinician]),
    (Action::PatientRefWrite, &[Owner, Admin, Clinician]),
    (Action::ExportRead, &[Owner, Admin, Clinician, Researcher]),
    (
        Action::ExportDeidentified,
        &[Owner, Admin, Clinician, Researcher],
    ),
    (Action::ExportIdentified, &[Owner, Clinician]),
    (Action::IntegrationRead, &[Owner, Admin]),
    (Action::IntegrationManage, &[Owner, Admin]),
];

/// True when `role` may perform `action` (in its own org). `superadmin` may do everything
/// but [`CLINICAL_CONTENT_ACTIONS`] (RBAC-3).
pub fn allows(role: Role, action: Action) -> bool {
    if role == Role::Superadmin {
        return !CLINICAL_CONTENT_ACTIONS.contains(&action);
    }
    MATRIX
        .iter()
        .find(|(a, _)| *a == action)
        .is_some_and(|(_, roles)| roles.contains(&role))
}

/// Org check first (404), then the role (403). `org = None` checks the role only; the
/// handler then calls [`UserPrincipal::ensure_org`] on the loaded resource.
pub fn authorize(principal: &UserPrincipal, action: Action, org: Option<Uuid>) -> AppResult<()> {
    if let Some(org) = org {
        principal.ensure_org(org)?;
    }
    if allows(principal.role, action) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

/// Seniority for role assignment: superadmin > owner > admin > the rest.
pub fn rank(role: Role) -> u8 {
    match role {
        Role::Superadmin => 3,
        Role::Owner => 2,
        Role::Admin => 1,
        Role::Clinician | Role::Researcher | Role::Reviewer | Role::Viewer => 0,
    }
}

/// An actor may grant or touch roles up to its own rank (an admin cannot create or edit
/// an owner). `superadmin` is never assignable through the org API.
pub fn can_assign(actor: Role, target: Role) -> bool {
    target != Role::Superadmin && rank(target) <= rank(actor)
}

/// RBAC-3 and ADR-0008 D2 applied to user management (security audit finding 1): whether
/// `actor` may give a user role `to` (from role `from`; `None` when creating the user).
///
/// Whoever creates a user chooses its password, and whoever promotes a user may have
/// chosen it earlier, so either can sign in as that user. An actor that may not read
/// clinical content (superadmin, admin) therefore never grants a clinical-content action
/// ([`CLINICAL_CONTENT_ACTIONS`]) that it lacks itself and that the user did not already
/// hold: superadmin gets no owner, admin, clinician or researcher; admin gets no clinician
/// (owner is above its rank anyway). Demotions and status changes stay allowed. An actor
/// with `ClinicalContentRead` (owner: the clinic itself) provisions clinicians. There is
/// no invitation flow yet (users are created `active` with a password), so these are
/// refused with `403 clinical-role-not-assignable`.
pub fn can_provision(actor: Role, from: Option<Role>, to: Role) -> bool {
    allows(actor, Action::ClinicalContentRead)
        || CLINICAL_CONTENT_ACTIONS
            .iter()
            .all(|&a| !allows(to, a) || allows(actor, a) || from.is_some_and(|f| allows(f, a)))
}

pub fn role_as_str(role: Role) -> &'static str {
    match role {
        Role::Owner => "owner",
        Role::Admin => "admin",
        Role::Clinician => "clinician",
        Role::Researcher => "researcher",
        Role::Reviewer => "reviewer",
        Role::Viewer => "viewer",
        Role::Superadmin => "superadmin",
    }
}

pub fn role_from_str(s: &str) -> Option<Role> {
    Some(match s {
        "owner" => Role::Owner,
        "admin" => Role::Admin,
        "clinician" => Role::Clinician,
        "researcher" => Role::Researcher,
        "reviewer" => Role::Reviewer,
        "viewer" => Role::Viewer,
        "superadmin" => Role::Superadmin,
        _ => return None,
    })
}

/// A compile-time permission for [`super::extractors::Require`].
pub trait Permission: Send + Sync + 'static {
    const ACTION: Action;
}

/// Marker types, one per [`Action`]: `Require<perm::DeviceRevoke>`.
pub mod perm {
    use super::{Action, Permission};

    macro_rules! markers {
        ($($name:ident),* $(,)?) => {
            $(
                #[derive(Debug, Clone, Copy)]
                pub struct $name;
                impl Permission for $name {
                    const ACTION: Action = Action::$name;
                }
            )*
        };
    }

    markers!(
        OrgRead,
        OrgUpdate,
        UserRead,
        UserManage,
        DeviceRead,
        EnrolCodeCreate,
        DeviceRevoke,
        AuditRead,
        SessionRead,
        ParticipantRead,
        MetricsRead,
        ReviewRead,
        ReviewWrite,
        ConfigRead,
        ConfigWrite,
        ConfigPublish,
        ConfigGlobalManage,
        JobRun,
        ParticipantErase,
        WorklistRead,
        ClinicalReviewWrite,
        ClinicalSign,
        ClinicalCosign,
        ClinicalContentRead,
        ClinicalNoteWrite,
        LetterWrite,
        PatientRefRead,
        PatientRefWrite,
        ExportRead,
        ExportDeidentified,
        ExportIdentified,
        IntegrationRead,
        IntegrationManage,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(role: Role, org: Option<Uuid>) -> UserPrincipal {
        UserPrincipal {
            user_id: Uuid::new_v4(),
            org_id: org,
            role,
        }
    }

    #[test]
    fn every_action_has_exactly_one_row() {
        for action in Action::ALL {
            assert_eq!(
                MATRIX.iter().filter(|(a, _)| a == action).count(),
                1,
                "{action:?}"
            );
        }
        assert_eq!(MATRIX.len(), Action::ALL.len());
    }

    #[test]
    fn org_check_precedes_role_check() {
        let org = Uuid::new_v4();
        let other = Uuid::new_v4();
        let viewer = principal(Role::Viewer, Some(org));
        assert!(authorize(&viewer, Action::OrgRead, Some(org)).is_ok());
        assert!(matches!(
            authorize(&viewer, Action::OrgUpdate, Some(org)),
            Err(AppError::Forbidden)
        ));
        assert!(matches!(
            authorize(&viewer, Action::OrgUpdate, Some(other)),
            Err(AppError::NotFound)
        ));
        let root = principal(Role::Superadmin, None);
        assert!(authorize(&root, Action::OrgUpdate, Some(other)).is_ok());
        // RBAC-3: the org check passes for superadmin, the clinical carve-out does not.
        assert!(matches!(
            authorize(&root, Action::ClinicalContentRead, Some(other)),
            Err(AppError::Forbidden)
        ));
    }

    #[test]
    fn clinical_content_actions_are_matrix_actions() {
        for action in CLINICAL_CONTENT_ACTIONS {
            assert!(Action::ALL.contains(action), "{action:?}");
            assert!(!allows(Role::Superadmin, *action), "{action:?}");
        }
    }

    #[test]
    fn assignment_ceiling() {
        assert!(can_assign(Role::Owner, Role::Owner));
        assert!(can_assign(Role::Admin, Role::Admin));
        assert!(!can_assign(Role::Admin, Role::Owner));
        assert!(!can_assign(Role::Superadmin, Role::Superadmin));
        assert!(can_assign(Role::Superadmin, Role::Owner));
        assert!(!can_assign(Role::Clinician, Role::Admin));
    }

    #[test]
    fn non_clinical_actors_never_provision_clinical_roles() {
        use Role::Superadmin;
        // Creating a user.
        for target in [Owner, Admin, Clinician, Researcher] {
            assert!(!can_provision(Superadmin, None, target), "{target:?}");
        }
        for target in [Reviewer, Viewer] {
            assert!(can_provision(Superadmin, None, target), "{target:?}");
        }
        assert!(!can_provision(Admin, None, Clinician));
        for target in [Admin, Researcher, Reviewer, Viewer] {
            assert!(can_provision(Admin, None, target), "{target:?}");
        }
        for target in [Owner, Admin, Clinician, Researcher, Reviewer, Viewer] {
            assert!(can_provision(Owner, None, target), "{target:?}");
        }
        // Role changes: promotions into new clinical actions are refused, demotions are not.
        assert!(!can_provision(Superadmin, Some(Viewer), Clinician));
        assert!(!can_provision(Superadmin, Some(Viewer), Owner));
        assert!(!can_provision(Admin, Some(Researcher), Clinician));
        assert!(can_provision(Superadmin, Some(Owner), Admin));
        assert!(can_provision(Superadmin, Some(Clinician), Viewer));
        assert!(can_provision(Superadmin, Some(Viewer), Reviewer));
        assert!(can_provision(Admin, Some(Clinician), Clinician));
    }

    #[test]
    fn role_strings_round_trip_and_match_serde() {
        for role in [
            Role::Owner,
            Role::Admin,
            Role::Clinician,
            Role::Researcher,
            Role::Reviewer,
            Role::Viewer,
            Role::Superadmin,
        ] {
            assert_eq!(role_from_str(role_as_str(role)), Some(role));
            assert_eq!(
                serde_json::to_value(role).unwrap(),
                serde_json::Value::from(role_as_str(role))
            );
        }
        assert_eq!(role_from_str("root"), None);
    }
}
