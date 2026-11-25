Feature: Polished dark launcher interface
  Scenario: Play is the prominent call to action
    Given the launcher with a configured game and an active profile
    Then I see a "Play" button styled as the primary call to action
    And it is enabled

  Scenario: The status bar shows the game, the session and Play
    Given the launcher with a configured game and an active profile
    Then the status bar shows the game name and its store
    And the status bar holds the active profile selector and the "Play" button
    And the "Play" button is styled as the primary button
    And the status bar says the launcher is ready to play

  Scenario: The status bar guides the setup when no game is configured
    Given the launcher without a configured game
    Then the status bar says no game is selected
    When I click "Set up game" in the status bar
    Then the Settings item is marked as the current page

  Scenario: The sidebar navigation marks the current screen
    Given the launcher open on the Library
    Then the main navigation sits in a left sidebar with one item per screen
    And there is no top tab bar
    When I select Settings in the navigation
    Then the Settings item is marked as the current page
    And the Library item is no longer marked

  Scenario: Collapse the sidebar to icons only
    Given the launcher open on the Library with the sidebar expanded
    Then the sidebar toggle is named "Collapse sidebar" and reports it is expanded
    When I activate the sidebar toggle
    Then the sidebar is collapsed
    And the toggle is named "Expand sidebar" and reports it is collapsed
    And every navigation item keeps its accessible name and shows it as a tooltip
    When I select Profiles in the navigation
    Then the Profiles item is marked as the current page

  # The custom title bar (issue #69) already shows the mark and the app name (user feedback, #71).
  Scenario: The sidebar does not repeat the brand shown in the title bar
    Given the launcher open on the Library
    Then the sidebar shows no brand mark
    And the main navigation is the first thing in the sidebar and keeps its name
    And the document has a single visually hidden h1 with the app name

  Scenario: The sidebar toggle works from the keyboard
    Given the launcher open on the Library with the sidebar expanded
    When I focus the sidebar toggle and press Enter
    Then the sidebar is collapsed
    When I press Space
    Then the sidebar is expanded

  Scenario: Every screen has one visible title and a live subtitle
    Given mods A and B in the library and only A in the active profile
    Then the Library title is visible with the subtitle "2 mods · 1 in Mine"
    When I open the Profiles screen
    Then the Profiles title is visible with the subtitle "2 profiles · active: Mine"

  Scenario: The library lists mods as dense rows
    Given mods A and B in the library and only A in the active profile
    When I open the Library
    Then I see a list with one row per mod showing its name, size and file count
    And the row for A says it is in the active profile
    And the row for B says it is not in the active profile
    And each row offers a remove button named after the mod

  Scenario: Filter the library by name
    Given mods "Yamaha Livery" and "Ducati Sound" in the library
    When I type "duc" in the library search
    Then only "Ducati Sound" is listed
    When I type "zzz" in the library search
    Then I see the no-results message

  Scenario: Filter the library by active profile with counts
    Given mods A and B in the library and only A in the active profile
    Then I see the facets "All" 2, "In active profile" 1 and "Not in active profile" 1
    And the "All" facet is selected
    When I choose the "Not in active profile" facet
    Then only "Mod B" is listed
    When I type "Mod A" in the library search
    Then I see the no-results message
    When I clear the filters
    Then both mods are listed again and the "All" facet is selected

  Scenario: Empty library offers the next action
    Given an empty library
    Then I see the empty-library message
    And an "Import mods" button that opens the import dialog

  Scenario: Loading shows a skeleton instead of an empty screen
    Given the launcher is still loading
    Then the content area is marked busy and announces that it is loading
    And the empty-library message is not shown

  Scenario: Profiles show their mod counts and guide the selection
    Given mods A and B in the library and only A in the active profile
    When I open the Profiles screen
    Then each profile in the list shows its mod count
    And the active profile is marked as active
    And the editor area asks me to select a profile

  Scenario: Settings has a section navigation
    Given the launcher open on the Settings screen
    Then I see a "Settings sections" navigation with "Game", "Preferences" and "About"
    And the "Game" section is marked as the current location
    When I select the "Preferences" section
    Then the "Preferences" section is marked as the current location
    And the language setting is a row with its help text

  Scenario: Session status uses a variant per state
    Given a running session
    Then the session status announces it is running with the running variant

  Scenario: Notices use banner variants
    Given an error from the last command
    Then I see an alert banner with the error variant
    When the error is dismissed and a success notice arrives
    Then I see a status banner with the success variant

  Scenario: The design tokens meet WCAG AA contrast
    Given the design tokens declared in styles.css
    Then body text on the background and on every surface level has a contrast ratio of at least 4.5
    And muted text on the surfaces has a contrast ratio of at least 4.5
    And the Play label on the accent has a contrast ratio of at least 4.5
    And the accent text on the surfaces has a contrast ratio of at least 4.5
    And the focus ring on the background has a contrast ratio of at least 3

  Scenario: Focus is always visible
    Given the stylesheet
    Then every :focus-visible rule draws an outline using the focus token
    And no rule removes the outline without a replacement

  Scenario: Motion respects the reduced-motion preference
    Given the stylesheet
    Then it declares a prefers-reduced-motion rule that disables animations and transitions

  # User feedback on #71: with the sidebar, the content uses the whole window, with breathing room.
  Scenario: The content uses the full width next to the sidebar
    Given the stylesheet
    Then the page container, the loading state and the messages column set no max-width
    And there is no content max-width token
    And the content and the messages use the page gutter of 24px
    And a narrow-window media query reduces the page gutter to 16px
    And text-heavy blocks keep a readable measure

  Scenario: The stylesheet stays restrained
    Given the stylesheet
    Then no rule uses clip-path
    And the radius tokens are between 4px and 8px

  # Material 3 type scale, hand-built (issue #69): body-large 16px for content text, nothing
  # below label-small 12px, captions (body-small) at 13px.
  Scenario: The type scale is readable
    Given the stylesheet
    Then the body-large text token is at least 16px and the page text uses it
    And every type token is at least 12px
    And the caption token is at least 13px
    And no rule sets a font size outside the type tokens
    And buttons use the label-large token

  Scenario: The title bar close hover is readable
    Given the stylesheet
    Then the close button glyph on its hover background has a contrast of at least 4.5

  Scenario: The title bar buttons use Material state layers and motion
    Given the stylesheet
    Then the state layer opacities are 8% for hover and 10% for focus and pressed
    And the title bar buttons draw their hover, focus and pressed states with those opacities
    And the motion tokens use the Material standard easing
