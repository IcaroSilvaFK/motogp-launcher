Feature: Online mod catalog (application, fake ports)
  Scenario: Use the cache while fresh
    Given the catalog was cached 1 hour ago and the TTL is 6 hours
    When the catalog is requested
    Then the cached snapshot is returned without calling the source

  Scenario: Refresh a stale cache
    Given the catalog was cached 7 hours ago
    When the catalog is requested
    Then the source is called once and the new snapshot is cached

  Scenario: Work offline
    Given the cache is stale and the source is unreachable
    When the catalog is requested
    Then the cached snapshot is returned with the stale flag

  Scenario: No cache and no network
    Given there is no cache and the source is unreachable
    When the catalog is requested
    Then the request fails as unavailable

  Scenario: Skip malformed entries
    Given the source returns 3 entries and one has a "file://" page URL
    When the catalog is requested
    Then 2 entries are returned and 1 entry is reported as skipped

  Scenario: Skip duplicate ids
    Given the source returns two entries with the id "rebalanced-ai"
    When the catalog is requested
    Then only the first one is kept and 1 entry is reported as skipped

  Scenario: Single-flight refresh
    Given the cache is stale
    When two requests arrive at the same time
    Then the source is called only once

  Scenario: Respect Retry-After
    Given the source answered "rate limited" with Retry-After 60
    When the catalog is refreshed again 30 seconds later
    Then the source is not called

  Scenario: Reject a newer schema
    Given the source returns schemaVersion 2
    When the catalog is requested with no cache
    Then the request fails as an unsupported schema

  Scenario: Not modified renews the cache
    Given the cache is stale and the source answers "not modified"
    When the catalog is requested
    Then the cached entries are returned as fresh
