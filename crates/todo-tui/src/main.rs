use color_eyre::Result;
use crossterm::event;
use crossterm::event::{Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::{Frame, Terminal};
use ratatui::{prelude::CrosstermBackend, widgets::ListState};
use std::io::stdout;
use todo_common::Task;

#[derive(Default, PartialEq)]
enum InputMode {
    #[default]
    Normal,
    Editing,
}

#[derive(Default)]
struct App {
    tasks: Vec<Task>,
    state: ListState,
    input: String,
    mode: InputMode,
}

#[derive(serde::Serialize)]
struct CreateTodo {
    text: String,
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
            match app.mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('r') => match fetch_tasks().await {
                        Ok(tasks) => app.tasks = tasks,
                        Err(e) => {}
                    },
                    KeyCode::Char('i') => app.mode = InputMode::Editing,
                    KeyCode::Enter => {
                        if let Some(index) = app.state.selected()
                            && let Some(task) = app.tasks.get(index)
                        {
                            let _ = toggle_done(task.id).await;
                            if let Ok(tasks) = fetch_tasks().await {
                                app.tasks = tasks;
                            }
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let i = match app.state.selected() {
                            Some(i) => {
                                if i == 0 {
                                    app.tasks.len() - 1
                                } else {
                                    i - 1
                                }
                            }
                            None => 0,
                        };
                        app.state.select(Some(i));
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let i = match app.state.selected() {
                            Some(i) => {
                                if i >= app.tasks.len() - 1 {
                                    0
                                } else {
                                    i + 1
                                }
                            }
                            None => 0,
                        };
                        app.state.select(Some(i));
                    }
                    _ => {}
                },
                InputMode::Editing => match key.code {
                    KeyCode::Esc => {
                        app.mode = InputMode::Normal;
                        app.input.clear(); // clear buf
                    }
                    KeyCode::Char(c) => {
                        app.input.push(c);
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Enter => {
                        // TODO: this is blocking, change in future
                        let _ = create_task(app.input.clone()).await;
                        if let Ok(tasks) = fetch_tasks().await {
                            app.tasks = tasks;
                        }
                        // reset state
                        app.input.clear();
                        app.mode = InputMode::Normal;
                    }
                    _ => {}
                },
            }
        }
    }
    disable_raw_mode()?;
    Ok(())
}

const TITLE_INDEX: usize = 0;
const LIST_INDEX: usize = 1;
const INPUT_INDEX: usize = 2;
const FOOTER_INDEX: usize = 3;

fn ui(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Min(1),    // list
            Constraint::Length(3), // input
            Constraint::Length(1), // footer
        ])
        .split(frame.area());

    // render title

    let title = Paragraph::new(Text::styled("todo", Style::default().fg(Color::LightBlue)))
        .alignment(Alignment::Center);

    frame.render_widget(title, chunks[TITLE_INDEX]);

    // render list

    let list = List::new(app.tasks.iter().map(|t| t.to_listitem()))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, chunks[LIST_INDEX], &mut app.state);

    // render input
    let input_block = Block::default().borders(Borders::ALL).title("Add Task");
    let style = match app.mode {
        InputMode::Normal => Style::default().fg(Color::DarkGray),
        InputMode::Editing => Style::default().fg(Color::Yellow),
    };

    let input = Paragraph::new(app.input.as_str())
        .style(style)
        .block(input_block);

    frame.render_widget(input, chunks[INPUT_INDEX]);

    // render footer
    let help_text = match app.mode {
        InputMode::Normal => "q: quit | <CR>: toggle done | i: add task | r: refresh",
        InputMode::Editing => "Esc: exit editing mode | <CR>: Submit",
    };
    let footer = Paragraph::new(help_text).alignment(Alignment::Center);
    frame.render_widget(footer, chunks[FOOTER_INDEX]);
}

async fn fetch_tasks() -> Result<Vec<Task>, Box<dyn std::error::Error>> {
    let url = "http://localhost:3000/todos";

    let tasks = reqwest::get(url).await?.json::<Vec<Task>>().await?;
    Ok(tasks)
}

async fn create_task(text: String) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    client
        .post("http://localhost:3000/todos")
        .json(&CreateTodo { text })
        .send()
        .await?;
    Ok(())
}

async fn toggle_done(id: i64) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    client
        .patch(format!("http://localhost:3000/todos/{id}"))
        .send()
        .await?;
    Ok(())
}

trait TaskExt {
    fn to_listitem(&'_ self) -> ListItem<'_>;
}

impl TaskExt for Task {
    fn to_listitem(&'_ self) -> ListItem<'_> {
        let color = if self.done {
            Color::Green
        } else {
            Color::Yellow
        };
        let status_text = if self.done { "[x]" } else { "[ ]" };
        let line = Line::from(vec![
            Span::styled(status_text, Style::default().fg(color)),
            Span::raw(format!(" {} ", self.text)),
            Span::styled(
                format!("{}", self.priority),
                Style::default().fg(Color::Gray),
            ),
        ]);
        ListItem::new(line)
    }
}
