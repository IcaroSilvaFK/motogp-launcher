Feature: Storage section in Settings
  Scenario: The section shows the data folder, totals and the breakdown
    Given the backend reports the data folder "C:\Users\Usuário Padrão\AppData\Roaming\motogp-launcher", mods 1.5 GB, launcher data 20 KB, other 0 B and 120 GB free
    When the user opens Settings
    Then the Settings section nav lists "Storage"
    And the Storage section shows the data folder path
    And it shows the total used and the free space
    And it shows one bar per category with its size and share

  Scenario: Open folder asks the backend to open the data folder
    Given the Storage section is shown
    When the user clicks "Open folder"
    Then the launcher asks to open the data folder

  Scenario: Refresh measures again without blocking the screen
    Given the Storage section is shown
    When the user clicks "Refresh" and the measurement is still running
    Then the Refresh button is disabled and the card is marked busy
    And the previous values stay visible
    And the other Settings controls stay enabled
    When the measurement finishes
    Then the new values are shown

  Scenario: Unknown free space is shown as unavailable
    Given the backend cannot read the free space
    When the user opens Settings
    Then the free space shows "Unavailable"

  Scenario: A failed measurement shows an inline error with retry
    Given the backend fails to measure the storage
    When the user opens Settings
    Then the Storage section shows an error with a "Retry" button
    And no global notice is shown

  Scenario: The section is translated
    Given the language is "pt-BR"
    When the user opens Settings
    Then the Storage section labels are in Portuguese
