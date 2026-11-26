Feature: Hosting mods on the community server

  Scenario: A creator publishes a mod
    Given the creator is signed in
    When they upload "brazil-livery" version "1.0" with a zip file
    Then "My mods" lists "brazil-livery" version "1.0" as "pending"

  Scenario: A pending mod is not public
    Given the creator uploaded "brazil-livery" version "1.0" and it is waiting for review
    Then the public catalog does not list "brazil-livery"
    And the file of "brazil-livery" version "1.0" cannot be downloaded

  Scenario: A file that is too large is refused
    Given the size cap is 2 GiB
    When the creator declares a 3 GiB upload
    Then the server answers 413 with code "file_too_large"
    And no byte was stored

  Scenario: An interrupted upload resumes
    Given half of a file was uploaded
    When the connection drops and the creator asks where to continue
    Then the server reports the bytes it already has
    And the upload completes after sending only the rest

  Scenario: Only admins review
    Given the creator uploaded "brazil-livery" version "1.0" and it is waiting for review
    And a signed-in user who is not an admin
    When they try to approve "brazil-livery" version "1.0"
    Then the server answers 403 with code "forbidden"

  Scenario: A new version keeps the history
    Given an admin approved "brazil-livery" version "1.0"
    When the creator publishes version "1.1" and an admin approves it
    Then the public catalog shows "brazil-livery" version "1.1"
    And "My mods" lists both versions of "brazil-livery"

  Scenario: Downloads support ranges
    Given an admin approved a 250-byte file for "brazil-livery" version "1.0"
    When a client asks for bytes 0-99 of the file
    Then the server answers 206 with those 100 bytes
