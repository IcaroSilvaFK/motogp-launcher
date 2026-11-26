Feature: Download counts

  Scenario: Usage sharing is on by default
    Given the user never chose a usage preference
    When the user opens Settings > Preferences
    Then the "Share anonymous usage" switch is on
    And its hint says only the mod id and a random install id are sent

  Scenario: The launcher reports a successful install
    Given usage sharing is on
    When the user installs a catalog mod successfully
    Then the launcher reports that mod once

  Scenario: Opting out
    Given usage sharing is on
    When the user turns off "Share anonymous usage" in Settings
    And they install a mod
    Then nothing is sent
    And the stored usage preference is "off"

  Scenario: Failed or cancelled installs are not reported
    Given usage sharing is on
    When an install fails or is cancelled
    Then nothing is sent

  Scenario: Counts on Discover and the mod page
    Given the server knows the download counts
    When the user opens Discover and a mod page
    Then each card and the stats strip show the downloads

  Scenario: The 30-day chart
    Given the server knows the daily downloads of a mod
    When the mod page opens
    Then a bar chart shows one bar per day for the last 30 days
    And each bar names its date and count
    And the chart is summarised in text

  Scenario: Offline
    Given the community server cannot be reached
    When the user opens Discover, a mod page or installs a mod
    Then no count, no chart and no error are shown
