@store @consent
Feature: Store build first launch, guardian consent and onboarding
  The App Store build is an observation capture tool (store-collection-build.md S-1).
  It opens on onboarding with no mode picker (S-2). A parent or guardian reviews the
  visual consent page by page, types a name and agrees before anything else (S-10).
  Executable form: maestro/flows/store/01-consent-and-onboarding.yaml (StoreSim build).

  Background:
    Given a clean install of the StoreSim build with no keychain items

  Scenario: Review and sign the consent, then finish onboarding
    Given onboarding shows the privacy and consent step
    And Continue is disabled because no consent is recorded
    When the guardian opens "Review and agree"
    Then a screenshot named 01_consent_overview captures the first consent page
    When the guardian pages through all nine consent pages
    And types the neutral name "A. Parent" on the review page
    Then a screenshot named 02_consent_sign captures the name field and "I agree"
    When the guardian agrees
    Then onboarding shows that the consent was agreed
    When the guardian continues through the child profile, activity pack and guidance steps
    Then the store tabs Capture, Sessions and Settings are shown and there is no Dashboard tab

  Scenario: A simulator without the screenshot fixture is honestly gated
    Given onboarding is complete on a simulator with no TrueDepth camera
    Then the Capture tab shows "This device cannot run gaze observations"
    And no camera permission prompt is shown
