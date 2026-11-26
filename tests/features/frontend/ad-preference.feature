Feature: Ad preference

  Scenario: First launch shows a non-modal prompt
    Given the user never chose an ad preference
    When the launcher opens
    Then a dismissible bar offers "Keep" and "Turn off ads"
    And no dialog blocks the screen

  Scenario: Keeping ads hides the prompt for good
    Given the user never chose an ad preference
    When the launcher opens
    And the user clicks "Keep"
    Then the bar disappears
    And the stored ad preference is "on"
    And ads are enabled

  Scenario: Turning ads off from the prompt
    Given the user never chose an ad preference
    When the launcher opens
    And the user clicks "Turn off ads"
    Then the bar disappears
    And the stored ad preference is "off"
    And ads are disabled

  Scenario: A stored choice shows no prompt
    Given the stored ad preference is "on"
    When the launcher opens
    Then no ad preference bar is shown

  Scenario: Turning ads off removes every slot
    Given the stored ad preference is "on"
    When the user selects "Off" in Settings > Preferences
    Then no slot renders and no ad request is made
    And the stored ad preference is "off"

  Scenario: The store fails
    Given the preference store throws
    When the launcher opens
    Then ads are off
    And no ad preference bar is shown
