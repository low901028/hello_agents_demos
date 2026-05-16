use std::env;
use dotenvy::dotenv;

/// rust 代码脚本化
// #!/usr/bin/env cargo
// ---
// [dependencies]
// serde_json = "1.0"
// reqwest = { version ="0.12", features = ["blocking"] }
// ---
//
// use std::io::Read;
// use std::time::Duration;
// use reqwest::Client;
//
// fn main() {
//   let url = std::env::args().nth(1)
//    .unwrap_or_else(||"https://api.github.com/repos/rust-lang/rust".into());
//
//   let mut resp = reqwest::blocking::get(&url).unwrap();
//   let mut body = String::new();
//   resp.read_to_string(&mut body).unwrap();
//   let json: serde_json::Value = serde_json::from_str(&body).unwrap();
//   println!("Repository: {}", json["full_name"]);
//   println!("Stars: {}", json["stargazers_count"]);
//   println!("Language: {}", json["language"]);
// }

fn main() {

}