Feature: Catalog error codes
  Scenario: Catalog error codes are known and translated
    Then "catalog_unavailable" and "catalog_unsupported_schema" are error codes with en and pt-BR messages

  Scenario: Open page error codes are known and translated
    Then "catalog_entry_not_found", "catalog_page_not_allowed" and "open_page_failed" are error codes with en and pt-BR messages
