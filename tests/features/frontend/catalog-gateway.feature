Feature: Catalog gateway contract
  Scenario: The gateway calls the catalog commands
    When getCatalog and refreshCatalog are called
    Then "get_catalog" and "refresh_catalog" are invoked

  Scenario: The gateway opens a catalog page by id
    When openCatalogPage is called with "rebalanced-ai"
    Then "open_catalog_page" is invoked with only the id
