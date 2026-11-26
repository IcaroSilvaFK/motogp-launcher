Feature: Community server stars

  Scenario: Starring a mod
    Given the user is signed in
    And the mod "ducati-2025" has 10 stars from other users
    When they star "ducati-2025"
    Then the server answers 200 with 11 stars and starredByMe true

  Scenario: Unstarring a mod
    Given the user is signed in
    And the mod "ducati-2025" has 10 stars from other users
    And the user has starred "ducati-2025"
    When they unstar "ducati-2025"
    Then the server answers 200 with 10 stars and starredByMe false

  Scenario: One star per user
    Given the user is signed in
    And the user has starred "ducati-2025"
    When they star "ducati-2025"
    Then the server answers 200 with 1 stars and starredByMe true

  Scenario: Anonymous stats
    Given the mod "ducati-2025" has 3 stars from other users
    When stats are requested without a session
    Then "ducati-2025" shows 3 stars and no starredByMe flag

  Scenario: Stats with a session
    Given the user is signed in
    And the user has starred "ducati-2025"
    When stats are requested with their session
    Then "ducati-2025" shows 1 stars and starredByMe true

  Scenario: Stats with a stale session stay anonymous
    Given the user is signed in
    And the user has starred "ducati-2025"
    And their session expired
    When stats are requested with their session
    Then "ducati-2025" shows 1 stars and no starredByMe flag

  Scenario: Starring without a session is refused
    When a star is sent without a valid session
    Then the server answers 401 with code "unauthorized"

  Scenario: Invalid mod id
    Given the user is signed in
    When they star the mod id "Not A Slug!"
    Then the server answers 400 with code "bad_request"
