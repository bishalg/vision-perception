@store @privacy
Feature: Privacy & Your Data and Settings in the store build
  Privacy & Your Data shows the saved sessions, whether the device is enrolled, the
  optional features this build has (off by default; S-16: the age-range check and clinic
  delivery), the signed consent record and delete-all (S-12).
  Settings ends with About, the Privacy Policy and Support links and the disclaimer
  (S-6, S-13). Mode, medical conditions, care alerts and HealthKit mood are not shown.
  Executable form: maestro/flows/store/04-privacy-and-settings.yaml.

  Scenario: Privacy & Your Data
    Given the app is launched with the StoreSim screenshot fixture
    When the operator opens Settings and then "Privacy & Your Data"
    Then the saved session count and the delivery state are shown
    And a screenshot named 07_privacy_your_data captures the screen
    When the operator scrolls to the optional features
    Then the clinic delivery toggle is shown
    And no toggle is shown for reminders, Health mood, dictation or photo attachments

  Scenario: About and the disclaimer
    When the operator scrolls Settings to the About section
    Then the app version, Privacy Policy and Support links are shown
    And the not-diagnostic disclaimer is shown under them
    And a screenshot named 08_settings_about captures the section
