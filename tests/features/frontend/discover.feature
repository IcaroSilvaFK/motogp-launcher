Feature: Discover tab
  Scenario: Reach Discover from the sidebar
    When I click "Discover" in the sidebar
    Then the Discover screen is shown and the catalog is requested

  Scenario: Loading skeleton
    Given the catalog request is pending
    Then a loading skeleton is shown

  Scenario: Empty catalog explains how mods get listed
    Given the catalog has no entries
    Then an empty state explains that mods are added to the curated catalog by pull request
    And it offers a refresh action
    And it offers a way to go to the Library

  Scenario: Browse categories with counts
    Given the catalog has 2 "Physics" entries and 1 "Liveries" entry
    Then the category tiles show "Physics 2" and "Liveries 1"
    When I pick "Physics"
    Then only the 2 Physics entries are listed

  Scenario: Search by title, author or category
    Given the catalog has 2 "Physics" entries and 1 "Liveries" entry
    When I search "rebal"
    Then only matching entries are listed

  Scenario: Sort by recently updated or name
    Given the catalog has 2 "Physics" entries and 1 "Liveries" entry
    Then entries are listed by most recently updated first
    When I sort by name
    Then entries are listed alphabetically

  Scenario: Switch grid and list views
    Given the catalog has 2 "Physics" entries and 1 "Liveries" entry
    When I switch to the list view
    Then entries are shown as rows

  Scenario: Entry detail and Open page
    Given the catalog has 2 "Physics" entries and 1 "Liveries" entry
    When I open the entry "Rebalanced AI"
    Then its description, version, author and updated date are shown
    When I click "Open page"
    Then openCatalogPage is called with the entry id
    When I press Escape
    Then the entry detail is closed

  Scenario: Install is offered only for entries with a download
    Given an entry with a download block
    When I open the entry "Rebalanced AI"
    Then an Install button is shown

  Scenario: No Install button for an entry without a download
    Given an entry without a download block
    When I open the entry "Rebalanced AI"
    Then no Install button is shown

  Scenario: Stale banner with the saved date
    Given the catalog is stale and was fetched on 2026-11-20
    Then a banner says the catalog saved on that date is shown

  Scenario: Error with retry
    Given the catalog request fails with "catalog_unavailable"
    Then an error with a Retry button is shown
    When I click Retry
    Then refreshCatalog is called

  Scenario: Already in library badge
    Given the library has a mod matching the entry "rebalanced-ai"
    Then the entry shows "In library"

  Scenario: Open page failure is shown
    Given opening the page fails with "catalog_page_not_allowed"
    When I open the entry "Rebalanced AI"
    And I click "Open page"
    Then an error notice is shown
