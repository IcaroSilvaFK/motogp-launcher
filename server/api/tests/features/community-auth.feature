Feature: Community server auth

  Scenario: Health check
    When a client calls GET /health
    Then it answers 200 with status "ok"

  Scenario: Only loopback redirects are accepted
    When the app starts sign-in with redirect "https://evil.example/callback"
    Then the server answers 400 with code "invalid_redirect"

  Scenario: Full sign-in issues a session
    Given Discord authorizes the user "rider"
    When the app starts sign-in, the browser follows the callback and the app exchanges the one-time code
    Then the app receives a session token
    And GET /me with that token returns the display name "rider"

  Scenario: A one-time code works only once
    Given a one-time code was already exchanged
    When the app exchanges it again
    Then the server answers 400 with code "invalid_code"

  Scenario: A wrong code verifier is rejected
    When the app exchanges a valid code with the wrong verifier
    Then the server answers 400 with code "invalid_code"

  Scenario: Logout revokes the session
    Given a signed-in session
    When the app calls POST /auth/logout
    Then GET /me with the same token answers 401

  Scenario: An expired session is unauthorized
    Given a session created 31 days ago
    Then GET /me answers 401 with code "unauthorized"

  Scenario: Rate limiting
    Given the per-IP limit for /auth is 10 per minute
    When the same IP calls /auth/discord/start 11 times within a minute
    Then the 11th call answers 429 with code "rate_limited"

  Scenario: Tokens are stored hashed
    Given a signed-in session
    Then the sessions table does not contain the token in plain text
