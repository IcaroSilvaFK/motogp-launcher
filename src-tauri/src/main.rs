// Prevents the extra console window on Windows in release. DO NOT remove.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    motogp_launcher_lib::run()
}
