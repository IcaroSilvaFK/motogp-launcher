Feature: Custom title bar
  The native window decorations are off, so the launcher draws its own title bar with the app
  name, a drag region and the minimize, maximize/restore and close buttons.

  Scenario: The title bar shows the app name and the window controls
    Given the launcher is open with window controls
    Then the title bar shows "MotoGP Launcher"
    And the window controls group has the buttons "Minimize", "Maximize" and "Close"

  Scenario: Minimizing and closing the window
    Given the launcher is open with window controls
    When I press "Minimize"
    Then the window is asked to minimize
    When I press "Close"
    Then the window is asked to close

  Scenario: Maximize turns into Restore
    Given the launcher is open with window controls
    When I press "Maximize"
    Then the window is asked to toggle maximize
    When the window reports it is maximized
    Then the window controls show "Restore" instead of "Maximize"

  Scenario: The title area is the window drag region
    Given the launcher is open with window controls
    Then the title area is a drag region
    And no window control button is a drag region

  Scenario: The window controls speak the chosen language
    Given the launcher is open with window controls in Portuguese
    Then the window controls group has the buttons "Minimizar", "Maximizar" and "Fechar"

  Scenario: A failing window call does not break the launcher
    Given window controls that reject every call
    When the launcher opens
    And I press "Maximize"
    Then the launcher is still shown

  Scenario: Without window controls the title bar shows only the title
    Given the launcher is open without window controls
    Then the title bar shows "MotoGP Launcher"
    And there is no window controls group
