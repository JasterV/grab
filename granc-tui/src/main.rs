use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use granc_core::client::{Descriptor, DynamicRequest, DynamicResponse, GrancClient};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget, Wrap},
};
use serde_json::json;
use teatui::{ProgramError, update::Update};

fn main() -> Result<(), ProgramError<Model, Message, Effect>> {
    teatui::start(
        || {
            (
                Model::default(),
                Some(Effect::Connect("http://localhost:50051".to_string())),
            )
        },
        update,
        view,
        run_effects,
    )
}

// --- Model: Functional Application State ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Services,
    Methods,
    Payload,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub uri: String,
    pub services: Vec<String>,
    pub selected_service_idx: usize,
    pub methods: Vec<String>,
    pub selected_method_idx: usize,
    pub method_definition: String,
    pub json_payload: String,
    pub response_log: String,
    pub active_pane: Pane,
    pub error: Option<String>,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            uri: "http://localhost:50051".into(),
            services: vec![],
            selected_service_idx: 0,
            methods: vec![],
            selected_method_idx: 0,
            method_definition: "Select a method to see schema...".into(),
            json_payload: json!({ "name": "Granc" }).to_string(),
            response_log: "Ready to inspect server.".into(),
            active_pane: Pane::Services,
            error: None,
        }
    }
}

// --- Messages & Effects ---

#[derive(Debug)]
pub enum Message {
    SetServices(Vec<String>),
    SetMethods(Vec<String>),
    SetMethodDefinition(String),
    SetResponse(String),
    SetError(String),
    MoveDown,
    MoveUp,
    SwitchPane,
    ExecuteCall,
    Tick,
    Exit,
}

#[derive(Debug, Clone)]
pub enum Effect {
    Connect(String),
    FetchMethods(String),
    DescribeSymbol(String),
    Call(String, String, String),
}

impl From<crossterm::event::Event> for Message {
    fn from(value: Event) -> Self {
        match value {
            Event::Key(KeyEvent {
                code: KeyCode::Char('q') | KeyCode::Esc,
                kind: KeyEventKind::Press,
                ..
            }) => Self::Exit,
            Event::Key(KeyEvent {
                code: KeyCode::Tab,
                kind: KeyEventKind::Press,
                ..
            }) => Self::SwitchPane,
            Event::Key(KeyEvent {
                code: KeyCode::Down | KeyCode::Char('j'),
                kind: KeyEventKind::Press,
                ..
            }) => Self::MoveDown,
            Event::Key(KeyEvent {
                code: KeyCode::Up | KeyCode::Char('k'),
                kind: KeyEventKind::Press,
                ..
            }) => Self::MoveUp,
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            }) => Self::ExecuteCall,
            _ => Self::Tick,
        }
    }
}

// --- Update: Pure Logic ---

pub fn update(mut model: Model, msg: Message) -> Update<Model, Effect> {
    match msg {
        Message::Exit => Update::Exit,
        Message::SwitchPane => {
            model.active_pane = match model.active_pane {
                Pane::Services => Pane::Methods,
                Pane::Methods => Pane::Payload,
                Pane::Payload => Pane::Services,
            };
            Update::Next(model, None)
        }
        Message::MoveDown => match model.active_pane {
            Pane::Services if !model.services.is_empty() => {
                model.selected_service_idx =
                    (model.selected_service_idx + 1) % model.services.len();
                Update::Next(
                    model.clone(),
                    Some(Effect::FetchMethods(
                        model.services[model.selected_service_idx].clone(),
                    )),
                )
            }
            Pane::Methods if !model.methods.is_empty() => {
                model.selected_method_idx = (model.selected_method_idx + 1) % model.methods.len();
                let symbol = format!(
                    "{}.{}",
                    model.services[model.selected_service_idx],
                    model.methods[model.selected_method_idx]
                );
                Update::Next(model, Some(Effect::DescribeSymbol(symbol)))
            }
            _ => Update::Next(model, None),
        },
        Message::MoveUp => match model.active_pane {
            Pane::Services if !model.services.is_empty() => {
                model.selected_service_idx = if model.selected_service_idx == 0 {
                    model.services.len() - 1
                } else {
                    model.selected_service_idx - 1
                };
                Update::Next(
                    model.clone(),
                    Some(Effect::FetchMethods(
                        model.services[model.selected_service_idx].clone(),
                    )),
                )
            }
            Pane::Methods if !model.methods.is_empty() => {
                model.selected_method_idx = if model.selected_method_idx == 0 {
                    model.methods.len() - 1
                } else {
                    model.selected_method_idx - 1
                };
                let symbol = format!(
                    "{}.{}",
                    model.services[model.selected_service_idx],
                    model.methods[model.selected_method_idx]
                );
                Update::Next(model, Some(Effect::DescribeSymbol(symbol)))
            }
            _ => Update::Next(model, None),
        },
        Message::SetServices(svcs) => {
            model.services = svcs;
            Update::Next(model, None)
        }
        Message::SetMethods(meths) => {
            model.methods = meths;
            Update::Next(model, None)
        }
        Message::SetMethodDefinition(def) => {
            model.method_definition = def;
            Update::Next(model, None)
        }
        Message::ExecuteCall => {
            if model.services.is_empty() || model.methods.is_empty() {
                return Update::Next(model, None);
            }
            let svc = model.services[model.selected_service_idx].clone();
            let meth = model.methods[model.selected_method_idx].clone();
            Update::Next(
                model.clone(),
                Some(Effect::Call(svc, meth, model.json_payload.clone())),
            )
        }
        Message::SetResponse(res) => {
            model.response_log = res;
            Update::Next(model, None)
        }
        Message::SetError(err) => {
            model.error = Some(err);
            Update::Next(model, None)
        }
        _ => Update::Next(model, None),
    }
}

// --- Effects: Async Isolation with local Tokio Reactor ---

pub async fn run_effects(model: Model, effect: Effect) -> Option<Message> {
    let uri = model.uri.clone();

    match effect {
        Effect::Connect(url) => match GrancClient::connect(&url).await {
            Ok(mut client) => Some(Message::SetServices(
                client.list_services().await.unwrap_or_default(),
            )),
            Err(e) => Some(Message::SetError(e.to_string())),
        },
        Effect::FetchMethods(svc_name) => {
            let mut client = GrancClient::connect(&uri).await.ok()?;
            if let Ok(Descriptor::ServiceDescriptor(sd)) =
                client.get_descriptor_by_symbol(&svc_name).await
            {
                return Some(Message::SetMethods(
                    sd.methods().map(|m| m.name().to_string()).collect(),
                ));
            }
            None
        }
        Effect::DescribeSymbol(symbol) => {
            let mut client = GrancClient::connect(&uri).await.ok()?;
            if let Ok(descriptor) = client.get_descriptor_by_symbol(&symbol).await {
                let def = match descriptor {
                    Descriptor::MessageDescriptor(m) => {
                        format!("message {} {{ // ... }}", m.name())
                    }
                    Descriptor::ServiceDescriptor(s) => {
                        format!("service {} {{ // ... }}", s.name())
                    }
                    Descriptor::EnumDescriptor(e) => format!("enum {} {{ // ... }}", e.name()),
                };
                return Some(Message::SetMethodDefinition(def));
            }
            None
        }
        Effect::Call(svc, meth, payload) => {
            let mut client = GrancClient::connect(&uri).await.ok()?;
            let body = serde_json::from_str(&payload).unwrap_or(json!({}));
            let req = DynamicRequest {
                service: svc,
                method: meth,
                body,
                headers: vec![],
            };
            match client.dynamic(req).await {
                Ok(DynamicResponse::Unary(Ok(v))) => Some(Message::SetResponse(v.to_string())),
                Ok(DynamicResponse::Unary(Err(s))) => {
                    Some(Message::SetError(s.message().to_string()))
                }
                _ => Some(Message::SetError("Call failed".into())),
            }
        }
    }
}

// --- View: Fully Featured Multi-Pane Widget ---

pub fn view(model: Model) -> AppWidget {
    AppWidget { model }
}

pub struct AppWidget {
    model: Model,
}

impl Widget for AppWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let active_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let normal_style = Style::default().fg(Color::DarkGray);

        // Layout: [Header] (3) -> [Content] (rest) -> [Footer] (1)
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(1),
            ])
            .split(area);

        // 1. Header
        let header_content = vec![
            Line::from(vec![
                Span::styled(
                    " 🦀 GRANC WORKSPACE ",
                    Style::default().bg(Color::Blue).fg(Color::White).bold(),
                ),
                Span::raw(" Server: "),
                Span::styled(self.model.uri.clone(), Color::Green).underlined(),
            ]),
            Line::from(
                " (TAB: Switch Pane | Arrows/JK: Select | ENTER: Call) "
                    .italic()
                    .dark_gray(),
            ),
        ];
        Paragraph::new(header_content).render(root[0], buf);

        // 2. Content Split: Sidebar (30%) and Main (70%)
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(root[1]);

        // 2a. Sidebar Vertical: Services (50%) and Methods (50%)
        let sidebar = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(body[0]);

        // Services List
        let svc_items: Vec<ListItem> = self
            .model
            .services
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let style = if i == self.model.selected_service_idx {
                    Style::default().fg(Color::Yellow).bold()
                } else {
                    Style::default()
                };
                ListItem::new(format!("  • {}", s)).style(style)
            })
            .collect();
        List::new(svc_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" 1. SERVICES ")
                    .border_style(if self.model.active_pane == Pane::Services {
                        active_style
                    } else {
                        normal_style
                    }),
            )
            .render(sidebar[0], buf);

        // Methods List
        let meth_items: Vec<ListItem> = self
            .model
            .methods
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let style = if i == self.model.selected_method_idx {
                    Style::default().fg(Color::Cyan).bold()
                } else {
                    Style::default()
                };
                ListItem::new(format!("  ƒ {}", m)).style(style)
            })
            .collect();
        List::new(meth_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" 2. METHODS ")
                    .border_style(if self.model.active_pane == Pane::Methods {
                        active_style
                    } else {
                        normal_style
                    }),
            )
            .render(sidebar[1], buf);

        // 2b. Main Vertical: Definition (40%) and Payload/Response (60%)
        let main = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(body[1]);

        Paragraph::new(self.model.method_definition.clone())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" PROTO DEFINITION ")
                    .italic()
                    .cyan(),
            )
            .render(main[0], buf);

        // Payload & Response Horizontal Split
        let execution = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main[1]);

        Paragraph::new(self.model.json_payload.clone())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" 3. PAYLOAD ")
                    .border_style(if self.model.active_pane == Pane::Payload {
                        active_style
                    } else {
                        normal_style
                    }),
            )
            .render(execution[0], buf);

        let resp_style = if self.model.error.is_some() {
            Color::Red
        } else {
            Color::LightGreen
        };
        Paragraph::new(
            self.model
                .error
                .clone()
                .unwrap_or(self.model.response_log.clone()),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" RESPONSE LOG "),
        )
        .style(Style::default().fg(resp_style))
        .wrap(Wrap { trim: true })
        .render(execution[1], buf);

        // 3. Footer
        Paragraph::new(
            " Press 'q' to Quit | Granc Workspace v0.1.0 "
                .on_dark_gray()
                .white(),
        )
        .render(root[2], buf);
    }
}
