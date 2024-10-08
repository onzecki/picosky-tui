use std::f32::MIN_POSITIVE;
use serde::{Serialize, Deserialize};
use reqwest::*;
use serde_json::Number;

#[derive(Serialize, Deserialize)]
struct Post {
    did: String,
    handle: String,
    indexedAt: Number,
    nickname: Option<String>,
    post: String,
    rkey: String,
}

#[derive(Serialize, Deserialize)]
struct Content {
    cursor: Number,
    posts: Vec<Post>,
}

const MESSAGES_SHOWN: u16 = 20;

#[tokio::main]
async fn main() -> Result<()> {
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
    println!("Using user_agent: {user_agent}");
    let url = format!(
        "https://pico.api.bsky.mom/posts?limit={}&cursor=0",
        MESSAGES_SHOWN
    );
    let content: Content = client.get(url).send().await?.json().await?;
    for post in content.posts.iter().rev() {
        println!("{} {}: {}\n", post.nickname.to_owned().unwrap_or(String::new()), post.handle, post.post);
    }
    Ok(())
}
