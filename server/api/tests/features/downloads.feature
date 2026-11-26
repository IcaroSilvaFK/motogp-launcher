Feature: Download counts (server)

  Scenario: An install is counted once a day
    Given a mod has 100 downloads
    When the same install reports it twice on the same day
    Then the count becomes 101

  Scenario: The next day counts again
    Given an install reported a mod yesterday
    When it reports the mod today
    Then the mod has 2 downloads and the last 30 days show 2

  Scenario: Only a hash of the install id is stored
    When an install reports a mod
    Then the database holds the SHA-256 of the install id and never the raw id

  Scenario: Invalid reports are rejected
    When a report has a malformed mod id or install id
    Then the server answers 400 bad_request in the JSON error format

  Scenario: Reports are rate limited
    When one IP sends more than 20 reports in a minute
    Then the server answers 429 rate_limited with Retry-After

  Scenario: The 30-day buckets
    Given an install reported a mod yesterday
    When the daily downloads of the mod are requested
    Then there are 30 UTC days, oldest first, ending today, with zero for days without downloads
