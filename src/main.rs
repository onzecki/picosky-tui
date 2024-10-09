use color_eyre::{eyre::Context, Result};
use futures_util::{StreamExt, TryStreamExt};
use ratatui::{
    crossterm::event::{self, Event, KeyCode},
    widgets::{List, ListDirection, ListItem, Paragraph},
    DefaultTerminal, Frame,
};
use reqwest::*;
use reqwest_websocket::RequestBuilderExt;
use serde::{Deserialize, Serialize};
use serde_json::Number;
use std::io;
use std::time::Duration;
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
    color_eyre::install()?;
    let terminal = ratatui::init();

    let client: Client = reqwest::Client::builder()
        .user_agent(construct_user_agent().as_str())
        .build()
        .unwrap();

    let url = format!(
        "https://pico.api.bsky.mom/posts?limit={}&cursor=0",
        MESSAGES_SHOWN
    );
    let ws_url = "wss://pico.api.bsky.mom/subscribe";
    let app_result = run(terminal, client, url).await?;
    ratatui::restore();

    Ok(())
}

async fn run(mut terminal: DefaultTerminal, client: Client, history_url: String) -> Result<()> {
    let list_items = get_history(client, history_url).await?;

    loop {
        terminal.draw(|frame| draw(frame, list_items.clone()))?; // Draw the list of posts
        if should_quit()? {
            break;
        }
    }
    Ok(())
}

fn draw(frame: &mut Frame, list_items: Vec<ListItem>) {
    let list = List::new(list_items).block(
        ratatui::widgets::Block::default()
            .title("Posts")
            .borders(ratatui::widgets::Borders::ALL),
    ).direction(ListDirection::BottomToTop);

    frame.render_widget(list, frame.area());
}

fn should_quit() -> Result<bool> {
    if event::poll(Duration::from_millis(250)).context("event poll failed")? {
        if let Event::Key(key) = event::read().context("event read failed")? {
            return Ok(KeyCode::Char('q') == key.code);
        }
    }
    Ok(false)
}

fn construct_user_agent() -> String {
    let mut user_agent: String =
        format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    if cfg!(debug_assertions) {
        user_agent.push_str(" by ");
        user_agent.push_str(env!("CARGO_PKG_AUTHORS"));
        user_agent.push_str(" (DEBUGGING)");
    }
    return user_agent;
}

fn post_display(post: Post) -> String {
    format!(
        "{} {}: {}\n",
        post.nickname.to_owned().unwrap_or(String::new()),
        post.handle.unwrap_or(String::new()),
        post.post
    )
}

async fn get_history(client: Client, history_url: String) -> Result<Vec<ListItem<'static>>> {
    let initial_content: Content = client.get(history_url).send().await?.json().await?;
    let list_items = initial_content
        .posts
        .into_iter()
        .map(|post| {
            let content = post_display(post);
            ListItem::new(content)
        })
        .collect();

    Ok(list_items)
}

async fn get_new_messages(client: Client, websocket_url: &str) -> Option<ListItem>{
    let upgrade_response = client
        .get(websocket_url)
        .upgrade()
        .send()
        .await
        .unwrap();

    let websocket = upgrade_response.into_websocket().await.unwrap();
    let (mut _sender, mut receiver) = websocket.split();
    if let Some(item) = receiver.try_next().await.unwrap() {
        if let reqwest_websocket::Message::Text(json_post) = item {
            let post: Post = serde_json::from_str(json_post.as_str()).unwrap();
            return Some(ListItem::new(post_display(post)));
        }
    }
    return None;
}