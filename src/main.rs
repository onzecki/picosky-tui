use futures_util::{StreamExt, TryStreamExt};
use reqwest::*;
use reqwest_websocket::RequestBuilderExt;
use serde::{Deserialize, Serialize};
use serde_json::Number;

#[derive(Serialize, Deserialize, Clone)]
struct Post {
    did: String,
    handle: Option<String>,
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
    let client: Client = reqwest::Client::builder()
        .user_agent(construct_user_agent().as_str())
        .build()
        .unwrap();
    
    let url = format!(
        "https://pico.api.bsky.mom/posts?limit={}&cursor=0",
        MESSAGES_SHOWN
    );

    let ws_url = "wss://pico.api.bsky.mom/subscribe";

    get_history(client.clone(), url).await?;

    tokio::join!(
        get_new_messages(client.clone(), ws_url)
    );
    Ok(())
}

fn construct_user_agent() -> String {
    let mut user_agent: String =
    format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    if cfg!(debug_assertions) {
        user_agent.push_str(" by ");
        user_agent.push_str(env!("CARGO_PKG_AUTHORS"));
        user_agent.push_str(" (DEBUGGING)");
        println!("Using user_agent: {user_agent}");
    }
    return user_agent;

}

fn display_post(post: Post) {
    println!(
        "{} {}: {}\n",
        post.nickname.to_owned().unwrap_or(String::new()),
        post.handle.unwrap_or(String::new()),
        post.post
    );
}

async fn get_history(client: Client, history_url: String) -> Result<()>{
    let initial_content: Content = client.get(history_url).send().await?.json().await?;
    for post in initial_content.posts.iter().rev() {
        display_post(post.clone());
    }
    Ok(())
}

async fn get_new_messages(client: Client, websocket_url: &str) -> Result<()>{
    let upgrade_response = client
        .get(websocket_url)
        .upgrade()
        .send()
        .await
        .unwrap();

    let websocket = upgrade_response.into_websocket().await.unwrap();
    let (mut _sender, mut receiver) = websocket.split();
    while let Some(item) = receiver.try_next().await.unwrap() {
        if let reqwest_websocket::Message::Text(json_post) = item {
            let post: Post = serde_json::from_str(json_post.as_str()).unwrap();
            display_post(post);
        }
    }
    Ok(())
}
