Feature: Support the project
  The launcher links to its Apoia.se support page from the sidebar footer and from Settings > About.
  The page address lives in the backend: the UI only asks the controller to open the support page.

  Scenario: Support entry in the expanded sidebar footer
    Given the sidebar is expanded
    Then the footer shows a "Support the project" entry

  Scenario: Support entry in the collapsed sidebar
    Given the sidebar is collapsed
    Then the footer shows only the support icon with an accessible name and tooltip

  Scenario: Support entry in Settings > About
    When the user opens Settings
    Then About shows a short support text and a "Support on Apoia.se" button

  Scenario: Clicking the entry opens the support page
    When the user activates any support entry
    Then the controller is asked to open the support page, with no URL

  Scenario: The browser cannot be opened
    Given opening the support page fails
    When the user activates the support entry
    Then a non-blocking message says the page could not be opened

  Scenario: Portuguese copy
    Given the language is pt-BR
    Then the support entry and the About text are shown in Portuguese
