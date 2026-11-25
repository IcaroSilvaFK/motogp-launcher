Feature: Catalog error codes
  Scenario: Catalog error codes are known and translated
    Then "catalog_unavailable" and "catalog_unsupported_schema" are error codes with en and pt-BR messages
