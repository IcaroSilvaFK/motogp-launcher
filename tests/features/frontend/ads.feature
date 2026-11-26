Feature: Sponsored placements
  The launcher shows labeled, non-intrusive sponsored cards. The backend fetches the creative and
  returns it over IPC with the image as a data URI; the UI never holds a click URL.

  Scenario: Discover shows a labeled sponsored card
    Given an ad is available
    When the user opens Discover
    Then a card labeled "Sponsored" is shown outside the catalog results and counts

  Scenario: The sidebar card hides when collapsed
    Given an ad is available
    When the sidebar is collapsed
    Then the sidebar ad is not shown

  Scenario: Settings About shows a quiet sponsored row
    Given an ad is available
    When the user opens Settings > About
    Then a row labeled "Sponsored" is shown in the About card

  Scenario Outline: No ads while a game session is active
    Given an ad is available
    And the session is "<state>"
    Then no ad is rendered in any screen

    Examples:
      | state       |
      | starting    |
      | running     |
      | stopping    |
      | cleaning up |

  Scenario: Play is never blocked
    Given an ad is loading
    Then the Play button is enabled and does not move
    And no ad placeholder or spinner is shown

  Scenario: Clicking an ad sends only its id
    Given an ad is shown in the "discover" slot
    When the user activates it
    Then the provider receives a click for slot "discover" and that creative id only

  Scenario: A shown ad is reported once
    Given an ad is available
    When the Discover card is rendered
    Then "shown" is reported once for slot "discover" and that creative id

  Scenario: No ad means no slot
    Given the provider has no ad, or it fails, or the command does not exist
    Then no slot is rendered and no error is shown to the user

  Scenario: Ads disabled renders nothing
    Given an ad is available
    And the user turned sponsored content off
    When the user opens Discover and Settings > About
    Then no slot renders and no ad request is made

  Scenario: A creative without an image
    Given a house ad has no image
    When it is shown in the sidebar
    Then a compact card shows an icon tile, the title and the body

  Scenario: One label only
    Given an ad from "Acme Racing" is available
    When the user opens Discover
    Then the card shows "Sponsored" once and the sponsor name as a byline

  Scenario: Turning sponsored content off in Settings removes the slots
    Given an ad is shown in the sidebar
    When the user selects "Off" in Settings > Preferences
    Then no slot renders in the sidebar, Discover or Settings > About
