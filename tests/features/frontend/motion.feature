Feature: Motion and micro-interactions
  Scenario: The stylesheet defines Material motion tokens
    Given the launcher stylesheet
    Then it defines the standard, emphasized decelerate and emphasized accelerate easings
    And it defines short and medium durations
    And it defines a stagger step and an enter distance

  Scenario: Animations only use motion tokens
    Given the launcher stylesheet
    Then every transition and animation outside the root block references motion tokens
    And no keyframe animates a property other than transform or opacity

  Scenario: Reduced motion disables non-essential motion
    Given the launcher stylesheet
    Then a reduced-motion media query turns off every animation and transition

  Scenario: Switching screens plays an enter transition
    Given the launcher is loaded on the Library screen
    When the user opens the Profiles screen
    Then the new page is marked as entering

  Scenario: A notice slides out before it is removed
    Given a notice is shown
    When the notice is dismissed
    Then the notice is marked as exiting
    And it is removed after the exit duration

  Scenario: Under reduced motion a notice is removed at once
    Given the user prefers reduced motion
    And a notice is shown
    When the notice is dismissed
    Then the notice is removed immediately

  Scenario: List entrance is staggered with a cap
    Given the library has more mods than the stagger cap
    Then each row carries its stagger index
    And no row is delayed beyond the cap

  Scenario: The list entrance does not replay on re-renders
    Given the library has more mods than the stagger cap
    When the user toggles a mod in the active profile
    Then the rows stay mounted, so their entrance does not play again

  Scenario: The session dot pulses only while a session is live
    Given the launcher stylesheet
    Then the starting and running session dots have a pulse animation
    And the stopping and cleaning-up session dots pulse too
    And the idle session status has none

  Scenario: The session action looks busy while the game stops or cleans up
    Given the launcher stylesheet
    Then the stopping and cleaning-up progress button shows a spinner and a progress cursor
    And the progress button gives no hover or press feedback
    And Stop shows the same spinner through a busy modifier while the game is starting
