@care @data
Feature: Longitudinal care and data control
  Caregivers can switch where the app is used, read milestone guidance, log
  emotions, and delete their data — and deletion always asks twice before it
  runs. Nothing here is a diagnosis.

  Scenario: Switch app mode between home and clinic
    Given the Settings tab is open
    When the caregiver selects "Clinic / Professional"
    Then a screenshot named settings-clinic-mode captures the clinic layout
    When the caregiver selects "Parent & Caregiver"
    Then the app returns to home mode

  Scenario: Read a CDC / AAP milestone in detail
    Given the dashboard is open
    When the caregiver scrolls to the "18-month CDC / AAP" milestones
    Then a screenshot named milestones-section captures the checklist rows
    When the caregiver opens a milestone row
    Then a screenshot named milestone-detail captures the detail sheet
    And Done dismisses the sheet

  Scenario: Log an emotion in the diary
    Given the dashboard is open
    When the caregiver taps "Log emotion"
    Then the mood diary sheet opens
    And a screenshot named mood-diary captures the diary
    And Close dismisses it without saving

  Scenario: Data deletion is guarded
    Given the Settings tab is open
    When the caregiver taps "Delete all observation data"
    Then the app asks "Delete all observation data?" before deleting anything
    And a screenshot named delete-data-confirm captures the confirmation
    When the caregiver taps Cancel
    Then no data has been deleted
