Feature: Install a mod hosted on the community server (issue #130)

  Scenario: An approved mod installs in one click
    Given an admin approved "Brazil Livery" 1.0
    When a player opens it in Discover and chooses Install
    Then the file downloads from the community server, its SHA-256 is checked and it is installed

  Scenario: A community file whose SHA-256 differs is not installed
    Given the community server lists "Brazil Livery" 1.0 with a wrong SHA-256
    When a player opens it in Discover and chooses Install
    Then the install fails with code "download_hash_mismatch" and nothing is installed
