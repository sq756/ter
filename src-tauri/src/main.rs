// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
  // v2.11.12: Linux Rendering Fix for GitHub/Google
  std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
  ter_lib::run();
}
