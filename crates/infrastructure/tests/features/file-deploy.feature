# language: en
Feature: Deploy only the session mods and recover their cleanup on the real file system

  Scenario: Deploy in the profile order
    Given the mods B and A in that order
    When I deploy their files
    Then the destinations are "000_<idB>_B.pak" and "001_<idA>_A.pak"
    And each deployed file has a recoverable persisted record

  Scenario: Clean only what was deployed
    Given registered deployed files and the manual file "meu_mod.pak" in ~mods
    When I clean the session
    Then only the registered files are removed
    And meu_mod.pak keeps the same bytes
    And the manifest is empty after a complete cleanup

  Scenario: Tolerate an already-missing deployed file
    Given a registered file was already removed
    When I clean the session
    Then the cleanup finishes without an error for that file
    And the count considers only files actually removed

  Scenario: Clean the previous session before redeploying
    Given a previous session with A deployed
    When I deploy only B
    Then the files of A are removed before those of B
    And only B is registered

  Scenario: Refuse a collision with a user file
    Given a manual file with the exact name of the planned destination
    When I try to deploy the mod by hard link or copy fallback
    Then I get an error containing "already exists"
    And the manual file stays intact and outside the cleanup

  Scenario: Deploy an empty profile without creating a directory
    Given there is no previous session nor a ~mods folder
    When I deploy a profile without mods
    Then ~mods remains absent

  Scenario: Recover from an interruption during deploy
    Given an interruption at a point between the record and the materialization of a file
    When I resume recovery from the manifest and the journal
    Then every file proven deployed can be cleaned
    And no foreign file is deleted

  Scenario: Retry a partially failed cleanup
    Given two registered files and a failure when removing the second
    When I clean the session
    Then I get an error
    And the second file remains tracked for recovery

  Scenario: Reject a path outside the authorized directory
    Given a tampered manifest with a path containing ".."
    When I try to clean the session
    Then the operation returns an error
    And no external file is removed
