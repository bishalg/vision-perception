# ADR-0005: Publishing organisation, bundle identifier and review contact

Status: Accepted (H4 resolved 2026-10-29: bundle id `com.open-spectrum.eye-gaze`, widget `com.open-spectrum.eye-gaze.carewidget`, team `6J3WY94J6T`, Apple ID `6828468388`, App Name "Eye Gaze")
Spec ids: CI-4, CI-5, CLM-6

## Context

- `fastlane/Appfile:5-7` and `fastlane/config.yml:9-11` carry a personal Yahoo Apple
  ID and `team_name "Binod Ghimire (Individual)"`; `.env.example:17` repeats it.
- `fastlane/metadata/review_information/` holds a reviewer contact (a different
  person's name, Gmail address and Nepali mobile number) and a demo password. These
  were committed in `307312e` (2026-09-15) and are on `origin/dev` and `cursor/dev`.
- Bundle ids are `org.curv.asd.gazecollector` and `.carewidget` (`project.yml:38`,
  `:88`), team `6J3WY94J6T`, copyright "Curv Engineering", URLs on
  `curv.engineering`, and `age_suitability_url.txt` points at `astro-fusion.com`,
  an unrelated product.
- The local FHIR CodeSystem namespace is `http://curv.org/fhir` (ADR-0003).
- The app is to ship under the Open Spectrum Tracking mission, whose GitHub home is
  `github.com/bishalg/open-spectrum-tracking` and whose web domain is not recorded
  in either repository.

Apple requires an Organization account (with D-U-N-S) for an app published by an
organisation; an Individual account publishes under the person's legal name, which
is incompatible with "published by Open Spectrum Tracking" and with review contacts
that belong to the organisation.

## Decisions

1. Identity is environment-driven. `APP_IDENTIFIER`, `WIDGET_IDENTIFIER`,
   `APPLE_TEAM_ID`, `APPLE_ID`, and `REVIEW_CONTACT_*`/`REVIEW_DEMO_*` are read from
   the environment by the Appfile, Matchfile, config and the `update_metadata` lane.
   Committed defaults may name a bundle prefix but never a person.
2. `fastlane/metadata/review_information/` is removed from the tree and ignored.
   Review information is passed to `deliver` from environment at run time.
3. Bundle id: keep `org.curv.asd.gazecollector` only if Curv Engineering is the
   legal publisher under an Organization account; otherwise the maintainer chooses a
   reverse-DNS prefix of a domain the publishing organisation controls, and the
   same base is used for the FHIR namespace (`FHIRNamespaces.local`) and all store
   URLs. The change is mechanical (project.yml, Matchfile, config.yml, .env.example,
   Fastfile defaults, FHIR constant, vault keychain service string stays as is to
   preserve existing keys).
4. History: the agent does not rewrite git history. The maintainer decides (H1)
   between `git filter-repo --invert-paths --path apps/ios_asd_gaze_app/fastlane/metadata/review_information`
   with a coordinated force-push to both remotes, or accepting the history and
   rotating the demo password. The demo account must be rotated in either case.
5. Primary category defaults to Health & Fitness (H5), consistent with ADR-0004.

## Alternatives rejected

- Keep the individual account and ship now. Rejected: the publisher name on the
  store would be an individual, and the committed contact data belongs to people,
  not an organisation.
- Hardcode a new `org.openspectrumtracking.*` prefix. Rejected: nobody has
  confirmed domain ownership; a wrong prefix is painful to change after the first
  TestFlight build.

## Consequences

- Until H4 is decided, signing and upload lanes cannot run; everything else in the
  plan proceeds because it does not depend on the identifier.
- The claims script fails on `astro-fusion.com` and on blank organisation URLs so
  the gap is visible in CI rather than discovered in review.
