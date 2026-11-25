Feature: Catalog gateway contract
  Scenario: The gateway calls the catalog commands
    When getCatalog and refreshCatalog are called
    Then "get_catalog" and "refresh_catalog" are invoked
