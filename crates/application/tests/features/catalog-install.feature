Feature: Install a catalog entry

  Scenario: A downloadable entry is installed into the library
    Given the catalog has the downloadable entry "rebalanced-ai" with a matching file
    When the user installs "rebalanced-ai"
    Then the mod appears in the library
    And no staging files remain

  Scenario: The downloaded size differs from the catalog
    Given the catalog entry "rebalanced-ai" declares a size different from the file
    When the user installs "rebalanced-ai"
    Then the install fails with a size mismatch
    And the library is unchanged and no staging files remain

  Scenario: The SHA-256 does not match
    Given the catalog entry "rebalanced-ai" declares another SHA-256
    When the user installs "rebalanced-ai"
    Then the install fails with a hash mismatch
    And the library is unchanged and no staging files remain

  Scenario: An entry without download cannot be installed
    Given the catalog entry "page-only" has no download
    When the user installs "page-only"
    Then the install fails as not downloadable
    And nothing is downloaded

  Scenario: Installs are refused while a session is running
    Given a game session is active
    When the user installs "rebalanced-ai"
    Then the install fails because the library is locked by the session
    And nothing is downloaded

  Scenario: A cancelled install leaves nothing behind
    Given an install of "rebalanced-ai" is downloading
    When the user cancels it
    Then the install fails as cancelled
    And no staging files remain
