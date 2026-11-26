Feature: Publishing a mod to the community server from the launcher

  Scenario: Only a signed-in creator sees the publish entry
    Given the creator is signed out
    When they open Discover
    Then there is no "Publish a mod" button
    When they sign in from Settings and open Discover again
    Then Discover shows a "Publish a mod" button

  Scenario: A creator publishes a mod
    Given the creator is signed in
    When they upload "Brazil Livery" version 1.0 with a zip file
    Then they see that the mod is waiting for review
    And it appears in "My mods" as waiting for review

  Scenario: The mod id follows the title until the creator edits it
    Given the creator is signed in and opens the publish form
    When they type the title "Pista Épica 2025"
    Then the mod id reads "pista-epica-2025"
    When they change the mod id to "my-track" and edit the title
    Then the mod id still reads "my-track"

  Scenario: An incomplete form shows what to fix
    Given the creator is signed in and opens the publish form
    When they choose Publish with an empty form
    Then each required field shows its error next to it
    And nothing is uploaded

  Scenario: Screenshots can be added and removed
    Given the creator is signed in and opens the publish form
    When they add two screenshots and remove the first one
    Then only the second screenshot is listed

  Scenario: The upload shows its progress and can be cancelled
    Given the creator is signed in and a publish is uploading
    Then a progress bar shows the bytes sent and the total
    And the form cannot be submitted again
    When they cancel the upload
    Then they see that the upload was cancelled and the form is kept

  Scenario: A failed upload explains why and can be retried
    Given the creator is signed in and the server already has this version
    When they publish the mod
    Then they see that the version was already published
    When they change the version and try again
    Then the mod is waiting for review

  Scenario: My mods shows every version with its review status
    Given the creator has a mod with an approved, a rejected and a pending version
    When they open "My mods"
    Then each version shows its status
    And the rejected version shows the reason

  Scenario: My mods explains an empty list and a failed load
    Given the creator has never published a mod
    When they open "My mods"
    Then they are told they have no mods yet
    When the server cannot be reached and they refresh
    Then they see the error and can try again

  Scenario: Escape goes back to Discover
    Given the creator is signed in and opens the publish form
    When they press Escape
    Then Discover lists the mods again

  Scenario: A suspended creator sees why publishing is refused
    Given the creator is signed in but their community account is suspended
    When they publish the mod
    Then they see that the account is suspended, with no retry
