@store @sessions
Feature: After a session: recording quality and the Sessions list
  The post-session screen shows only the scanpath replay, recording quality, where the
  session is, New Observation, Done and the not-diagnostic footer (S-3). The Sessions tab
  lists date, duration, clips, recording quality and storage state (S-2). Neither shows a
  score, band, flag or report: the clinic reviews each session.
  Executable form: maestro/flows/store/03-session-and-sessions.yaml. A simulator cannot
  record a session, so the StoreSim screenshot fixture provides example sessions for that
  launch only (in memory, marked simulated, no score values).

  Scenario: Post-session recording quality
    Given the app is launched with the StoreSim screenshot fixture and the post-session screen
    Then the screen is titled "Observation saved" and shows recording quality and "Saved on this device"
    And no score, band, flag or report is shown
    And a screenshot named 05_recording_quality captures it
    When the operator taps Done
    Then the post-session screen closes

  Scenario: Sessions list
    When the operator opens the Sessions tab
    Then each session row shows date, duration, clips, recording quality and storage state
    And the list footer carries the not-diagnostic disclaimer
    And a screenshot named 06_sessions captures the list
