Feature: Server dev mode

  Scenario: Fake sign-in completes the launcher flow
    Given the server runs with DEV_FAKE_AUTH on a loopback base URL
    When the launcher starts a Discord sign-in
    Then it receives a session for the fake user without contacting Discord

  Scenario: Dev mode refuses a public base URL
    Given DEV_FAKE_AUTH is set and PUBLIC_BASE_URL is https://community.example.com
    Then the server refuses to start

  Scenario: Seeding is idempotent
    When the server starts with seeding twice
    Then the demo data exists once
