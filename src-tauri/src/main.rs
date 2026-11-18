// Evita a janela de console extra no Windows em release. NÃO remover.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    motogp_launcher_lib::run()
}
