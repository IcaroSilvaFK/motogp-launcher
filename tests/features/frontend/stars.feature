Feature: Stars
  Signed-in members star the catalog mods they like. Discover shows the counts and can sort by
  them; the mod page has the star button. Without the community nothing breaks: the counts and the
  button simply are not shown.

  Scenario: Starring a mod
    Given the user is signed in
    When they star a mod with 10 stars
    Then it shows 11 and the button shows as starred

  Scenario: Unstarring a mod
    Given the user is signed in and has starred a mod with 11 stars
    When they choose the star button again
    Then it shows 10 and the button shows as not starred

  Scenario: Starring needs an account
    Given the user is signed out
    When they choose the star button
    Then they are offered "Sign in with Discord"

  Scenario: Signing in from the star applies it
    Given the user is signed out and was offered "Sign in with Discord" on a mod with 4 stars
    When they sign in with Discord
    Then the mod is starred and shows 5

  Scenario: Sort by stars
    Given mods with 5, 12 and 0 stars
    When the user sorts Discover by most stars
    Then the mods are listed with 12, 5 and 0 stars

  Scenario: Offline community
    Given the community server is unreachable
    When the user opens Discover
    Then the mods are listed without star counts and no error is shown

  Scenario: Stats are cached briefly
    Given Discover loaded the stats less than a minute ago
    When the user opens a mod page
    Then the stats are not fetched again
