Feature: Community sign-in
  Settings > Community holds the Discord account row (#122). The backend owns the browser sign-in,
  the session token (Windows Credential Manager) and the avatar download; the UI only sees the
  status (signed out, signed in with a name and an avatar, or offline) and never the token.

  Scenario: Signing in with Discord
    Given the user is signed out
    When they choose "Sign in with Discord" and authorize in the browser
    Then the launcher shows their Discord name and avatar

  Scenario: Signing out revokes the session
    Given the user is signed in
    When they sign out
    Then the session token is revoked on the server and removed from the Credential Manager

  Scenario: An expired session
    Given the stored session expired
    When the launcher starts
    Then the user is shown as signed out without an error

  Scenario: Server unreachable
    Given the server cannot be reached
    Then the rest of the launcher works and the account row says the community is offline

  Scenario: Secrets stay out of the repository and logs
    Then no client secret or session token appears in the repository, the logs or the frontend
