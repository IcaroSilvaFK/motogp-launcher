Feature: Mod comments (community server)

  Background:
    Given a community server with the users "Ada" and "Bob"

  Scenario: Reading comments without an account
    Given the mod "livery-pack" has 3 comments
    When an anonymous client lists the comments of "livery-pack"
    Then 3 comments are returned
    And every comment has an author, a date and a text

  Scenario: Posting a comment
    Given the mod "livery-pack" has 2 comments
    And "Ada" is signed in
    When "Ada" posts "Great livery" on "livery-pack"
    Then the server answers 201
    And the last comment of "livery-pack" is "Great livery"

  Scenario: Loading more comments
    Given the mod "livery-pack" has 25 comments
    When an anonymous client lists the comments of "livery-pack"
    Then 20 comments are returned
    And a next cursor is returned
    When the client asks for the next page
    Then 5 comments are returned
    And no next cursor is returned
    And all 25 comments were seen in posting order

  Scenario: Editing my own comment
    Given "Ada" is signed in
    And "Ada" posted "Nice" on "livery-pack"
    When "Ada" edits it to "Nice livery"
    Then the server answers 200
    And the comment shows "Nice livery" and is marked edited

  Scenario: Deleting my own comment
    Given "Ada" is signed in
    And "Ada" posted "Oops" on "livery-pack"
    When "Ada" deletes it
    Then the server answers 204
    And "livery-pack" has no comments

  Scenario: Someone else's comment cannot be changed
    Given "Bob" posted "Bob says" on "livery-pack"
    And "Ada" is signed in
    When "Ada" edits it to "Hijacked"
    Then the server answers 404
    When "Ada" deletes it
    Then the server answers 404
    And the last comment of "livery-pack" is "Bob says"

  Scenario: Comment length cap
    Given "Ada" is signed in
    When "Ada" posts a comment of 2001 characters on "livery-pack"
    Then the server answers 400
    And "livery-pack" has no comments

  Scenario: Per-user write limit
    Given "Ada" is signed in
    And "Ada" posted 5 comments in the last minute
    When "Ada" posts "One too many" on "livery-pack"
    Then the server answers 429
    And the answer has a Retry-After header

  Scenario: No raw HTML
    Given "Ada" is signed in
    When "Ada" posts "<img src=x onerror=alert(1)>" on "livery-pack"
    Then the server answers 201
    And the last comment of "livery-pack" is "<img src=x onerror=alert(1)>"
    And the stored body is "<img src=x onerror=alert(1)>"
