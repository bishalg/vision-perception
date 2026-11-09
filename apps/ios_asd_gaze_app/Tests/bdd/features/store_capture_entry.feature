@store @capture
Feature: Capture entry in the store build
  Before any camera use the operator sees "Before you begin" with the not-diagnostic
  disclaimer and what is recorded (S-13). Tapping Begin shows the in-app camera primer
  before the iOS prompt, and "Not now" always works (S-11).
  Executable form: maestro/flows/store/02-capture-entry.yaml with the StoreSim screenshot
  fixture, which lets a simulator stand in for a TrueDepth device. No camera frame and no
  stimulus clip appears in any screenshot.

  Scenario: Before you begin, then the camera primer, then Not now
    Given onboarding and consent are complete
    And the app is launched with the StoreSim screenshot fixture
    When the operator opens the Capture tab
    Then "Before you begin" is shown
    And a screenshot named 03_before_you_begin captures it
    When the operator taps Begin
    Then the camera primer explains what the camera is used for before any system prompt
    And a screenshot named 04_camera_primer captures it
    When the operator taps "Not now"
    Then "Before you begin" is shown again
    And no camera permission prompt is shown
