Feature: Mod comments
  The mod page shows the community comments under the description (#118). Anyone can read them;
  posting, editing and deleting need a Discord sign-in. The backend owns the session and the
  avatars; the UI only sees comments, pages and typed errors. The server side of these rules
  (stored lengths, 404 on someone else's comment, the write limit) is covered by the server tests.

  Scenario: Reading comments without an account
    Given a mod has 3 comments
    When the user opens its page
    Then the 3 comments are shown with author, date and text

  Scenario: Posting a comment
    Given the user is signed in
    When they post "Great livery"
    Then the comment appears at the end of the list

  Scenario: Offline
    Given the server cannot be reached
    Then the page works and the comments area shows a quiet message

  Scenario: Signed out users are invited to sign in
    Given the user is not signed in
    When they open a mod page
    Then they see "Sign in with Discord to comment" instead of the composer

  Scenario: Loading more comments
    Given a mod has 25 comments
    When the user opens its page
    Then 20 comments are shown
    When they press "Load more"
    Then 25 comments are shown in posting order

  Scenario: Editing my own comment
    Given the user posted "Nice"
    When they edit it to "Nice livery"
    Then the comment shows "Nice livery" and is marked edited

  Scenario: Deleting my own comment
    Given the user posted "Oops"
    When they delete it and confirm
    Then the comment is gone

  Scenario: Someone else's comment cannot be changed
    Given another user posted a comment
    Then it has no edit or delete actions

  Scenario: Comment length cap
    When the user tries to post 2001 characters
    Then the comment is rejected and nothing is stored

  Scenario: Per-user write limit
    Given the user posted 5 comments in the last minute
    When they post another one
    Then the launcher shows a "slow down" message

  Scenario: No raw HTML
    Given a comment body "<img src=x onerror=alert(1)>"
    Then it is shown as text, not as an element
