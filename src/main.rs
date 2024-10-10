use color_eyre::Result;
use futures::{StreamExt, TryStreamExt};
use ratatui::{
    buffer::Buffer,
    crossterm::{
        event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
        style::Color,
    },
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style, Stylize},
    symbols,
    widgets::{
        Block, Borders, HighlightSpacing, List, ListItem, ListState, Paragraph, StatefulWidget,
        Widget,
    },
    DefaultTerminal,
};
use reqwest::*;
use reqwest_websocket::RequestBuilderExt;
use serde::*;
use serde_json::Number;
use std::{collections::VecDeque, fmt};
use std::sync::{Arc, Mutex};

// Define a couple of useful constants
const MESSAGES_SHOWN: usize = 20;
const URL: &str = "https://pico.api.bsky.mom/posts";
const WS_URL: &str = "wss://pico.api.bsky.mom/subscribe";

// social.psky.feed.post#create definition
#[derive(Serialize, Deserialize, Clone)]
struct Post {
    did: String,
    handle: String,
    indexedAt: Number,
    nickname: Option<String>,
    post: String,
    rkey: String,
}

// Display for each post - currently used as the chat message display
impl fmt::Display for Post {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}: {}\n",
            &self.nickname.to_owned().unwrap_or(String::new()),
            &self.handle,
            &self.post
        )
    }
}
// Definition used for the initial message load
#[derive(Serialize, Deserialize)]
struct Content {
    cursor: Number,
    posts: VecDeque<Post>,
}

// Definition for the App stuff... Contains things like the Posts which are used app-wide
#[derive(Default, Clone)]
struct App {
    client: Client,
    cursor: usize,
    post_state: ListState,
    posts: Arc<Mutex<VecDeque<Post>>>,
    should_exit: bool,
}

impl App {
    // Defaults - Runs once, on start
    async fn load() -> Self {
        let client: Client = reqwest::Client::builder()
            .user_agent(construct_user_agent().as_str())
            .build()
            .unwrap();
        let posts = Arc::new(Mutex::new(get_history(client.clone()).await.unwrap())); 
        let cursor = 0;
        Self {
            client,
            cursor,
            post_state: ListState::default(),
            posts,
            should_exit: false,
        }
    }
    // App function by itself
    async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        // Spawning the WS, and letting it run alongside the rest of the app
        let posts_ref = Arc::clone(&self.posts);
        tokio::spawn(get_new_messages(posts_ref, self.client.clone()));
        while !self.should_exit {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            if let Event::Key(key) = event::read()? {
                self.handle_key(key);
            };
        }
        Ok(())
    }
    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_exit = true,
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            _ => {}
        }
    }
    fn select_next(&mut self) {
        self.post_state.select_next();
    }
    fn select_previous(&mut self) {
        self.post_state.select_previous();
    }


}
// Ratatui "Display" for the main App 
impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [header_area, chat_area, footer_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        App::render_header(header_area, buf);
        App::render_footer(footer_area, buf);
        self.render_chat(chat_area, buf);
    }
}

// Functions used in the rendering process
impl App {
    fn render_header(area: Rect, buf: &mut Buffer) {
        Paragraph::new("Picosky-TUI")
            .bold()
            .centered()
            .render(area, buf);
    }

    fn render_footer(area: Rect, buf: &mut Buffer) {
        Paragraph::new("Made by onzecki")
            .centered()
            .render(area, buf);
    }

    fn render_chat(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .borders(Borders::TOP)
            .border_set(symbols::border::EMPTY)
            .border_style(Style::new().fg(Color::White.into()))
            .bg(Color::Black);
    let posts_lock = self.posts.lock().unwrap();
    let items: Vec<ListItem> = posts_lock
        .iter()
        .enumerate()
        .map(|(_, todo_item)| {
            let color = Color::Black;
            ListItem::from(todo_item.to_string()).bg(color)
        })
        .collect();
        let list = List::new(items)
            .block(block)
            .highlight_style(Style::new().add_modifier(Modifier::BOLD))
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);
        StatefulWidget::render(list, area, buf, &mut self.post_state);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let app_result = App::load().await.run(terminal).await;
    ratatui::restore();
    Ok(())
}
// Making sure that if everything goes south, the fault is attributed to me.
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
// Initial message history load
async fn get_history(client: Client) -> Result<VecDeque<Post>> {
    let initial_content: Content = client
        .get(format!("{}?limit={}&cursor=0", URL, MESSAGES_SHOWN))
        .send()
        .await?
        .json()
        .await?;
    let mut list_items: VecDeque<Post> = initial_content.posts;
    list_items.make_contiguous().reverse();
    Ok(list_items)
}
// WebSocket-based live message update
async fn get_new_messages(posts: Arc<Mutex<VecDeque<Post>>>, client: Client) -> Result<()> {
    let upgrade_response = client
        .get(WS_URL)
        .upgrade()
        .send()
        .await
        .unwrap();

    let websocket = upgrade_response.into_websocket().await.unwrap();
    let (mut _sender, mut receiver) = websocket.split();
    while let Some(item) = receiver.try_next().await.unwrap() {
        if let reqwest_websocket::Message::Text(json_post) = item {
            if json_post.contains("social.psky.feed.post#create"){
                let post: Post = serde_json::from_str(&json_post).unwrap();
                let mut posts_lock = posts.lock().unwrap();
                posts_lock.push_back(post);
            }
            
        }
    }
    Ok(())
}