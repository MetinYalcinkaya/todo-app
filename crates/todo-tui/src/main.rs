use color_eyre::Result;
use crossterm::event;
use crossterm::event::{Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::Alignment;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text, ToSpan};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Widget};
use ratatui::{Frame, Terminal};
use ratatui::{prelude::CrosstermBackend, widgets::ListState};
use std::io::stdout;
use todo_common::Task;

#[derive(Default)]
struct App {
    tasks: Vec<Task>,
    state: ListState,
}

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode().unwrap();
    color_eyre::install()?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = {
        App {
            tasks: fetch_tasks().await.unwrap_or_default(),
            ..Default::default()
        }
    };

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if event::poll(std::time::Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Char('r') => match fetch_tasks().await {
                    Ok(tasks) => app.tasks = tasks,
                    Err(e) => {}
                },
                _ => {}
            }
        }
    }
    disable_raw_mode()?;
    Ok(())
}

fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    // let title_block = Block::default()
    //     .borders(Borders::ALL)
    //     .style(Style::default());

    let title = Paragraph::new(Text::styled("todo", Style::default().fg(Color::LightBlue)))
        .alignment(Alignment::Center);
    // .block(title_block);

    frame.render_widget(title, chunks[0]);

    // render todos
    let list = List::new(app.tasks.iter().map(|t| t.to_listitem()));
    frame.render_widget(list, chunks[1]);
}

async fn fetch_tasks() -> Result<Vec<Task>, Box<dyn std::error::Error>> {
    let url = "http://localhost:3000/todos";

    let tasks = reqwest::get(url).await?.json::<Vec<Task>>().await?;
    Ok(tasks)
}

trait TaskExt {
    fn to_listitem(&self) -> ListItem;
}

impl TaskExt for Task {
    fn to_listitem(&self) -> ListItem {
        let color = if self.done {
            Color::Green
        } else {
            Color::Yellow
        };
        let status_text = if self.done { "[x]" } else { "[ ]" };
        let line = Line::from(vec![
            Span::styled(format!("{} ", status_text), Style::default().fg(color)),
            Span::raw(format!("{} ", self.text)),
            Span::styled(
                format!("{}", self.priority),
                Style::default().fg(Color::Gray),
            ),
        ]);
        ListItem::new(line)
    }
}
