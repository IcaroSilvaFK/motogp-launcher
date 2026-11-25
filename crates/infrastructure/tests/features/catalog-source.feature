Feature: HTTPS catalog source and cache (infrastructure, local server)

  Scenario: Fetch and parse the catalog
    Given a local server serving a valid catalog with 2 entries and ETag "v1"
    When the source fetches it
    Then 2 drafts are returned with the ETag "v1"

  Scenario: Conditional request
    Given the server answers 304 when If-None-Match is "v1"
    When the source fetches with the ETag "v1"
    Then the result is "not modified"
    And the server received If-None-Match "v1"

  Scenario: Size cap
    Given the server sends a body larger than the cap
    When the source fetches it
    Then it fails as too large

  Scenario: Rate limit
    Given the server answers 429 with Retry-After 60
    When the source fetches it
    Then it fails as rate limited for 60 seconds

  Scenario: Identify the launcher
    Given a local server serving a valid catalog with 2 entries and ETag "v1"
    When the source fetches it
    Then the request carries the launcher User-Agent

  Scenario: Cache roundtrip in a path with accents
    Given a data folder "Usuário Padrão"
    When a snapshot is saved and loaded
    Then the same entries and timestamp come back

  Scenario: A missing catalog file is "not published", not a network error
    Given a catalog server that answers 404
    When the catalog is fetched
    Then the fetch fails as "not published"

  Scenario: The bundled catalog is the repository catalog
    When the bundled catalog is parsed
    Then it equals the parsed repository file "catalog/catalog.json"
    And it passes the strict validation
