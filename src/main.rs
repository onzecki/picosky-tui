use reqwest::*;

fn main() {
    let mut user_agent: String =
        format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    if cfg!(debug_assertions) {
        user_agent.push_str(" by ");
        user_agent.push_str(env!("CARGO_PKG_AUTHORS"));
        user_agent.push_str(" (DEBUGGING)");
    }

    let client: Client = reqwest::Client::builder()
        .user_agent(user_agent.as_str())
        .build()
        .unwrap();
    println!("{user_agent}");
}
