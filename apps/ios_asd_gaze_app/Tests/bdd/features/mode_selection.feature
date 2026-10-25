@mode-selection
Feature: First-launch app mode selection
  The app asks where the device will be used before it asks anything else, so a
  caregiver never accidentally runs a clinic-style workflow at home (SIM-2).

  Scenario: Choose the parent home mode
    Given the app has never been launched
    When the launch splash dismisses
    Then the mode selection screen asks "How will you use Curv?"
    And the screen offers "Parent & Caregiver" and "Clinic / Professional"
    And a screenshot named mode-selection captures the unselected state

  When the caregiver selects "Parent & Caregiver"
  And taps Continue
  Then onboarding starts with the "Privacy & Security" step
  And a screenshot named mode-selection-parent captures the selected state
