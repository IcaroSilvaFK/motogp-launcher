Feature: Observe the game processes
  Scenario: Observe only executables inside the root
    Given a game root "C:/Jogos/MotoGP25"
    And executables at "c:/jogos/motogp25/Binaries/jogo.exe" and "C:/Jogos/MotoGP25Backup/outro.exe"
    When I classify the processes
    Then only the first is considered a game process
