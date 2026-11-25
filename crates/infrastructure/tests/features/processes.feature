Feature: Observe the game processes
  Scenario: Observe only executables inside the root
    Given a game root "C:/Jogos/MotoGP25"
    And executables at "c:/jogos/motogp25/Binaries/jogo.exe" and "C:/Jogos/MotoGP25Backup/outro.exe"
    When I classify the processes
    Then only the first is considered a game process

  Scenario: Only processes inside the install root are terminated
    Given a process running from inside the install root
    And a process running from outside the install root
    When the game processes are terminated
    Then the process inside the root has exited
    And the process outside the root is still running

  Scenario: Terminating with no game process ends nothing
    Given no process runs from inside the install root
    When the game processes are terminated
    Then 0 processes were ended
