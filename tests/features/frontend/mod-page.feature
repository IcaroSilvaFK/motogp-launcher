Feature: Mod page

  Scenario: Opening a mod shows its page
    Given the catalog has a mod with screenshots
    When the user opens it from Discover
    Then a full page shows its title, author, stats, gallery and description
    And a back button returns to the same Discover list and scroll position

  Scenario: The breadcrumb leads back to the category
    Given the user opened a mod of the category "liveries"
    When the user selects "liveries" in the breadcrumb
    Then the Discover list shows only the "liveries" category

  Scenario: Browsing screenshots
    Given the mod page shows 3 screenshots
    When the user selects the second thumbnail
    Then it becomes the main image
    And pressing Enter opens it in a lightbox that Escape closes

  Scenario: A broken screenshot collapses
    Given the mod page shows 2 screenshots and the first fails to load
    Then its thumbnail is removed and the second becomes the main image

  Scenario: A mod without screenshots
    Given a mod has no screenshots
    When its page opens
    Then no gallery is shown and nothing shifts

  Scenario: Unsafe content is not rendered
    Given a long description contains raw HTML
    Then the HTML is shown as text, not rendered

  Scenario: The long description uses the safe Markdown subset
    Given a long description with a heading, bold text and a list
    Then the page shows a heading, bold text and a list

  Scenario: Download stats appear only for downloadable mods
    Given a mod with a download of 2 MB
    Then the stats show its size and SHA-256
    And a mod without a download shows no size or SHA-256
