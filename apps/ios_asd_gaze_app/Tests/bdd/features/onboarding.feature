@onboarding
Feature: Caregiver onboarding, consent and child profile
  Consent is explicit: the Parent Consent switch must be on before the app will
  continue. The child profile picks the age-appropriate stimulus battery, and
  activities are downloaded once for offline use. No raw video is ever stored.

  Scenario: Agree to consent and set up the child profile
    Given onboarding shows the "Privacy & Security" step
    And the Parent Consent switch is off and Continue is disabled
    When the caregiver turns the Parent Consent switch on
    Then a screenshot named onboarding-consent captures the agreed state
    And Continue advances to "Child Profile Setup"

  When the caregiver types the child name into the profile form
  Then a screenshot named child-profile captures the age bracket selection

  When the app downloads activities for the child's age and language
  Then a screenshot named activities-download captures the download progress

  When onboarding reaches the "Getting Ready" environment guidance step
  Then a screenshot named getting-ready captures the configured profile preview

  When the caregiver taps "Complete Setup & Enter App"
  Then the dashboard shows the Care Plan
  And a screenshot named dashboard captures the longitudinal dashboard

  Scenario: No raw video claim is visible during onboarding
    Then the privacy step explains "Zero Raw Video Recording"
    And the privacy step explains hardware-bound encryption
