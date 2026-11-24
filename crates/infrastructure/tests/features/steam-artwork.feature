Feature: Read the game artwork from the local Steam library cache

  Scenario: Prefer the square icon in the per-appid folder
    Given a Steam cache with an icon and a capsule for App ID 3875050
    When I look up the artwork for App ID 3875050
    Then I receive the icon bytes as "image/jpeg"

  Scenario: Use the cover from a hashed sub-folder when there is no icon
    Given a Steam cache with only a capsule and a logo in a hashed sub-folder for App ID 3875050
    When I look up the artwork for App ID 3875050
    Then I receive the capsule image

  Scenario: Read the older flat file names
    Given a Steam cache with only the flat file "3875050_library_600x900.jpg"
    When I look up the artwork for App ID 3875050
    Then I receive that image

  Scenario: Steam installed under an accented user folder
    Given the Steam root is inside "Usuário Padrão"
    And the cache has an icon for App ID 3875050
    When I look up the artwork for App ID 3875050
    Then I receive the icon bytes as "image/jpeg"

  Scenario: Skip files that are too large or not images
    Given the icon is larger than 2 MiB and the capsule is a text file renamed to .jpg
    And a valid header.jpg exists
    When I look up the artwork for App ID 3875050
    Then I receive the header image

  Scenario: Nothing cached for the game
    Given a Steam cache without files for App ID 3875050
    When I look up the artwork for App ID 3875050
    Then I receive no artwork

  Scenario: Steam path unknown
    Given SteamPath is not available
    When I look up the artwork for App ID 3875050
    Then I receive no artwork
