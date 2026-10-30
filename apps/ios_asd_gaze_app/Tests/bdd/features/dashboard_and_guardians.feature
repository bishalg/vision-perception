@dashboard
Feature: Longitudinal dashboard and caregiver guardrails
  The dashboard lists M-CHAT-R/F status, care plan progress and reminders. The
  pediatrician report is reachable from the toolbar. On devices without a
  TrueDepth camera the gaze tab explains why live capture cannot run, and never
  pretends to record.

  Scenario: Review the dashboard after onboarding
    Given the caregiver completed onboarding
    When the dashboard tab is open
    Then the Care Plan card is visible
    And a screenshot named dashboard captures the dashboard

  Scenario: Open the M-CHAT-R/F questionnaire
    Given the build is generated with MCHAT_LICENSED=1
    When the caregiver taps "Open M-CHAT-R/F"
    Then the questionnaire modal opens
    And a screenshot named mchat-survey captures the questionnaire
    And cancelling returns to the dashboard

  Scenario: Generate the pediatrician report
    When the caregiver taps the toolbar action "Report for your pediatrician"
    Then a screenshot named pediatrician-report captures the report
    And Done dismisses the report

  Scenario: Gaze capture is gated on unsupported devices
    When the caregiver opens the Gaze tab
    Then the app shows "This device cannot run gaze observations"
    And no camera permission prompt appears
    And a screenshot named gaze-gate-simulator captures the honest gate
    And the app still offers the diary, reminders and notes

  Scenario: Settings capture tools are gated the same way
    Given the device has no TrueDepth camera
    When the caregiver opens Quick Calibrate or Camera & Distance Check from Settings
    Then the app shows "This device cannot run gaze observations"
    And no camera permission prompt appears
    And the caregiver can close it and return to Settings

  Scenario: Switch modes and review settings
    When the Settings tab is open
    Then the app mode can be changed to "Parent & Caregiver" or "Clinic / Professional"
    And privacy status shows "Verified On-Device"
    And CareKit sync shows "Encrypted On-Device"
    And a screenshot named settings captures the settings
