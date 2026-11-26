Feature: Moderation (community server)

  Background:
    Given a community server with the users "Ada" and "Bob" and the admin "Mod"

  Scenario: Reporting a comment
    Given "Ada" posted "Buy followers now" on "livery-pack"
    When "Bob" reports that comment as "spam"
    Then the server answers 204
    And the admin queue has 1 reported comment with 1 report
    And the admin page shows "Buy followers now"

  Scenario: Reporting is idempotent and not allowed on own comments
    Given "Ada" posted "Buy followers now" on "livery-pack"
    And "Bob" reported that comment as "spam"
    When "Bob" reports that comment as "spam"
    Then the server answers 204
    And the admin queue has 1 reported comment with 1 report
    When "Ada" reports that comment as "spam"
    Then the server answers 400

  Scenario: Hiding a comment
    Given "Ada" posted "Buy followers now" on "livery-pack"
    And "Bob" reported that comment as "spam"
    When the admin hides that comment with the reason "advertising"
    Then "livery-pack" shows no comments
    And the admin queue is empty
    And the audit log records "hide_comment" by "Mod" with the reason "advertising"
    And "Bob" reports that comment as "spam" and the server answers 404

  Scenario: Restoring a comment
    Given "Ada" posted "Buy followers now" on "livery-pack"
    And the admin hid that comment with the reason "advertising"
    When the admin restores that comment
    Then "livery-pack" shows the comment "Buy followers now"
    And the audit log records "restore_comment" by "Mod" with no reason

  Scenario: A banned user cannot write
    Given "Ada" posted "Ada was here" on "livery-pack"
    And "Ada" started an upload of "brazil-livery" version "1.0"
    When the admin bans "Ada" with the reason "spam wave"
    Then "Ada" is refused with 403 banned when they comment
    And "Ada" is refused with 403 banned when they edit their comment
    And "Ada" is refused with 403 banned when they delete their comment
    And "Ada" is refused with 403 banned when they star a mod
    And "Ada" is refused with 403 banned when they unstar a mod
    And "Ada" is refused with 403 banned when they report a comment
    And "Ada" is refused with 403 banned when they start an upload
    And "Ada" is refused with 403 banned when they send a chunk
    And "Ada" is refused with 403 banned when they add a screenshot
    And "Ada" is refused with 403 banned when they complete the upload
    And "Ada" is refused with 403 banned when they abort the upload
    And "livery-pack" shows no comments
    And the audit log records "ban_user" by "Mod" with the reason "spam wave"

  Scenario: Unbanning
    Given "Ada" posted "Ada was here" on "livery-pack"
    And the admin banned "Ada" with the reason "spam wave"
    When the admin unbans "Ada"
    Then "livery-pack" shows the comment "Ada was here"
    And "Ada" can comment again
    And the audit log records "unban_user" by "Mod" with no reason

  Scenario: Admin login
    When the admin signs in at /admin/login
    Then the server answers 200
    And an HttpOnly, Secure, SameSite=Strict admin session cookie is set
    And the admin page opens with that session

  Scenario: A non-admin cannot sign in to the admin page
    When a non-admin signs in at /admin/login
    Then the server answers 403
    And no admin session cookie is set

  Scenario: CSRF protection
    Given "Ada" posted "Buy followers now" on "livery-pack"
    And the admin is signed in to the admin page
    When the admin posts a hide form for that comment without a csrf token
    Then the server answers 403
    And "livery-pack" shows the comment "Buy followers now"

  Scenario: Reviewing a mod from the admin page
    Given "Ada" published "brazil-livery" version "1.0" with a screenshot
    And "Ada" published "brazil-livery" version "2.0" with a screenshot
    When the admin opens the admin page
    Then the pending version "brazil-livery" shows its metadata, screenshots and a download link
    When the admin approves "brazil-livery" version "1.0" on the admin page
    Then the server answers 303
    And the catalog lists "brazil-livery" version "1.0"
    And the audit log records "approve_version" by "Mod" with no reason
    When the admin rejects "brazil-livery" version "2.0" on the admin page with the reason "wrong game"
    Then the server answers 303
    And the audit log records "reject_version" by "Mod" with the reason "wrong game"
