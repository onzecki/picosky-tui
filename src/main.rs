use color_eyre::Result;
use ratatui::{
    buffer::Buffer,
    crossterm::{
        event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
        style::Color,
    },
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style, Stylize},
    symbols,
    text::Line,
    widgets::{
        Block, Borders, HighlightSpacing, List, ListItem, ListState, Paragraph, StatefulWidget,
        Widget,
    },
    DefaultTerminal,
};
use reqwest::*;
use serde::*;
use serde_json::Number;
use std::{collections::VecDeque, fmt};

const MESSAGES_SHOWN: usize = 20;
const URL: &str = "https://pico.api.bsky.mom/posts";
const WS_URL: &str = "wss://pico.api.bsky.mom/subscribe";
#[derive(Serialize, Deserialize, Clone)]
struct Post {
    did: String,
    handle: String,
    indexedAt: Number,
    nickname: Option<String>,
    post: String,
    rkey: String,
    cid: String,
}

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
#[derive(Serialize, Deserialize)]
struct Content {
    cursor: Number,
    posts: VecDeque<Post>,
}

#[derive(Default)]
struct App {
    client: Client,
    cursor: usize,
    post_state: ListState,
    posts: VecDeque<Post>,
    should_exit: bool,
}

impl App {
    async fn load() -> Self {
        let client: Client = reqwest::Client::builder()
            .user_agent(construct_user_agent().as_str())
            .build()
            .unwrap();
        let posts = get_history(client.clone()).await.unwrap();
        let cursor = 0;
        Self {
            client,
            cursor,
            post_state: ListState::default(),
            posts,
            should_exit: false,
        }
    }
    async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
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

        // Iterate through all elements in the `items` and stylize them.
        let items: Vec<ListItem> = self
            .posts
            .iter()
            .enumerate()
            .map(|(_, todo_item)| {
                let color = Color::Black;
                ListItem::from(todo_item.to_string()).bg(color)
            })
            .collect();

        // Create a List from all list items and highlight the currently selected one
        let list = List::new(items)
            .block(block)
            .highlight_style(Style::new().add_modifier(Modifier::BOLD))
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        // We need to disambiguate this trait method as both `Widget` and `StatefulWidget` share the
        // same method name `render`.
        StatefulWidget::render(list, area, buf, &mut self.post_state);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();

    let client: Client = reqwest::Client::builder()
        .user_agent(construct_user_agent().as_str())
        .build()
        .unwrap();
    let app_result = App::load().await.run(terminal).await;
    ratatui::restore();
    Ok(())
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
