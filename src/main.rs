#![allow(clippy::cast_lossless)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::option_if_let_else)]

use chip8_core::{Chip8, Display, Quirks};
use iced::alignment::Vertical;
use iced::keyboard;
use iced::keyboard::Key;
use iced::widget::image::{FilterMethod, Handle};
use iced::widget::space::horizontal;
use iced::widget::{Button, Checkbox, button, checkbox, column as col, container, image, text};
use iced::window;
use iced::{Color, Element, Length, Size, Subscription, Task};
use iced_aw::menu::DrawPath;
use rfd::AsyncFileDialog;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

type Item<'a, Message> = iced_aw::menu::Item<'a, Message, iced::Theme, iced::Renderer>;
type Menu<'a, Message> = iced_aw::menu::Menu<'a, Message, iced::Theme, iced::Renderer>;
type MenuBar<'a, Message> = iced_aw::menu::MenuBar<'a, Message, iced::Theme, iced::Renderer>;

const VIDEO_SCALE: f32 = 10.0;

const TIMER_HZ: u32 = 60;

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .window(window::Settings {
            size: Size::new(
                Display::WIDTH_F32 * VIDEO_SCALE,
                Display::HEIGHT_F32 * VIDEO_SCALE + 30.0,
            ),
            min_size: Some(Size::new(180.0, 180.0)),
            ..Default::default()
        })
        .run()
}

#[derive(Debug, Clone)]
enum KeyMessage {
    Pressed(Key),
    Released(Key),
}

#[derive(Debug, Clone, Copy)]
enum QuirksMessage {
    ToggleVfReset(bool),
    ToggleMemory(bool),
    ToggleClip(bool),
    ToggleShift(bool),
    ToggleJump(bool),
    ToggleRelease(bool),
    Discard,
    Apply,
}

#[derive(Debug, Clone)]
enum Message {
    OpenFilePicker,
    ProgramSelected(Option<PathBuf>),
    ProgramLoaded(Result<Program, io::ErrorKind>),
    Key(KeyMessage),
    TogglePause(bool),
    Stop,
    EmulateTick,
    TimerTick,
    Exit,
    Quirks(QuirksMessage),
}

#[derive(Debug, Clone)]
struct Program {
    pub name: String,
    pub data: Vec<u8>,
}

struct App {
    emulator: Chip8,
    clock_speed: u32,
    is_loaded: bool,
    is_paused: bool,
    error: Option<io::ErrorKind>,
    staged_quirks: Quirks,
    program: Option<Program>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    fn new() -> Self {
        let emulator = Chip8::new();
        Self {
            emulator,
            clock_speed: 500,
            is_loaded: false,
            is_paused: false,
            error: None,
            staged_quirks: Quirks::new(),
            program: None,
        }
    }

    fn title(&self) -> String {
        if let Some(program) = &self.program {
            format!("CHIP-8 Emulator - {}", program.name)
        } else {
            String::from("CHIP-8 Emulator")
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenFilePicker => Task::perform(pick_file(), Message::ProgramSelected),
            Message::ProgramSelected(path) => {
                if let Some(path) = path {
                    Task::perform(load_program(path), Message::ProgramLoaded)
                } else {
                    Task::none()
                }
            }
            Message::ProgramLoaded(Ok(program)) => {
                if self.is_loaded {
                    self.emulator.reset();
                }
                self.emulator.load_program(&program.data);
                self.program = Some(program);
                self.is_loaded = true;
                self.is_paused = false;
                Task::none()
            }
            Message::ProgramLoaded(Err(err)) => {
                self.error = Some(err);
                Task::none()
            }
            Message::Key(message) => {
                self.update_key(message);
                Task::none()
            }
            Message::TogglePause(checked) => {
                self.is_paused = checked;
                Task::none()
            }
            Message::Stop => {
                self.is_loaded = false;
                self.is_paused = false;
                self.emulator.reset();
                Task::none()
            }
            Message::EmulateTick => {
                if self.is_loaded {
                    self.emulator
                        .emulate()
                        .expect("Failed while emulating Chip8 instruction");
                }
                Task::none()
            }
            Message::TimerTick => {
                self.emulator.tick_timers();
                Task::none()
            }
            Message::Exit => window::latest().and_then(window::close),
            Message::Quirks(message) => {
                self.update_quirks(message);
                Task::none()
            }
        }
    }

    fn update_quirks(&mut self, message: QuirksMessage) {
        match message {
            QuirksMessage::ToggleVfReset(val) => {
                self.staged_quirks.vf_reset = val;
            }
            QuirksMessage::ToggleMemory(val) => {
                self.staged_quirks.memory = val;
            }
            QuirksMessage::ToggleClip(val) => {
                self.staged_quirks.clip = val;
            }
            QuirksMessage::ToggleShift(val) => {
                self.staged_quirks.shift = val;
            }
            QuirksMessage::ToggleJump(val) => {
                self.staged_quirks.jump = val;
            }
            QuirksMessage::ToggleRelease(val) => {
                self.staged_quirks.release = val;
            }
            QuirksMessage::Discard => {
                self.staged_quirks = self.emulator.quirks();
            }
            QuirksMessage::Apply => {
                if let Some(program) = &self.program {
                    self.emulator
                        .reload_with_quirks(self.staged_quirks, &program.data);
                } else {
                    self.emulator.set_quirks(self.staged_quirks);
                }
            }
        }
    }

    fn update_key(&mut self, message: KeyMessage) {
        match message {
            KeyMessage::Pressed(key) => {
                if let Some(key_idx) = get_key_idx(&key) {
                    self.emulator.set_key(key_idx, true);
                }
            }
            KeyMessage::Released(key) => {
                if let Some(key_idx) = get_key_idx(&key) {
                    self.emulator.set_key(key_idx, false);
                }
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let menu_bar = MenuBar::new(vec![
            Item::with_menu(
                menu_header("File"),
                menu(vec![
                    Item::new(menu_item("Open").on_press(Message::OpenFilePicker)),
                    Item::new(menu_item("Exit").on_press(Message::Exit)),
                ]),
            ),
            Item::with_menu(
                menu_header("Emulation"),
                menu(vec![
                    Item::new(menu_checkbox("Pause", self.is_paused).on_toggle_maybe(
                        if self.is_loaded {
                            Some(Message::TogglePause)
                        } else {
                            None
                        },
                    )),
                    Item::new(menu_item("Stop").on_press_maybe(if self.is_loaded {
                        Some(Message::Stop)
                    } else {
                        None
                    })),
                ]),
            ),
            Item::with_menu(
                menu_header("Quirks"),
                menu(vec![
                    Item::new(
                        menu_checkbox("VF Reset", self.staged_quirks.vf_reset)
                            .on_toggle(|b| Message::Quirks(QuirksMessage::ToggleVfReset(b))),
                    ),
                    Item::new(
                        menu_checkbox("Memory", self.staged_quirks.memory)
                            .on_toggle(|b| Message::Quirks(QuirksMessage::ToggleMemory(b))),
                    ),
                    Item::new(
                        menu_checkbox("Clip", self.staged_quirks.clip)
                            .on_toggle(|b| Message::Quirks(QuirksMessage::ToggleClip(b))),
                    ),
                    Item::new(
                        menu_checkbox("Shift", self.staged_quirks.shift)
                            .on_toggle(|b| Message::Quirks(QuirksMessage::ToggleShift(b))),
                    ),
                    Item::new(
                        menu_checkbox("Jump", self.staged_quirks.jump)
                            .on_toggle(|b| Message::Quirks(QuirksMessage::ToggleJump(b))),
                    ),
                    Item::new(
                        menu_checkbox("Release", self.staged_quirks.release)
                            .on_toggle(|b| Message::Quirks(QuirksMessage::ToggleRelease(b))),
                    ),
                    Item::new(menu_item("Discard Changes").on_press_maybe(
                        if self.staged_quirks == self.emulator.quirks() {
                            None
                        } else {
                            Some(Message::Quirks(QuirksMessage::Discard))
                        },
                    )),
                    Item::new(menu_item("Apply Changes").on_press_maybe(
                        if self.staged_quirks == self.emulator.quirks() {
                            None
                        } else {
                            Some(Message::Quirks(QuirksMessage::Apply))
                        },
                    )),
                ]),
            ),
        ])
        .draw_path(DrawPath::Backdrop)
        .width(Length::Fill);

        let pixels = convert_to_rgba(self.emulator.framebuffer());
        let screen = image(Handle::from_rgba(
            Display::WIDTH_U32,
            Display::HEIGHT_U32,
            pixels,
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .filter_method(FilterMethod::Nearest);

        container(col![menu_bar, horizontal().height(5), screen])
            .style(|_| container::Style::from(Color::BLACK))
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed {
                key,
                modifiers: keyboard::Modifiers::NONE,
                ..
            } => Some(Message::Key(KeyMessage::Pressed(key))),
            keyboard::Event::KeyReleased {
                key,
                modifiers: keyboard::Modifiers::NONE,
                ..
            } => Some(Message::Key(KeyMessage::Released(key))),
            _ => None,
        })];

        if self.is_loaded && !self.is_paused {
            let emulate = cycles_per_second(self.clock_speed).map(|_| Message::EmulateTick);
            let timer = cycles_per_second(TIMER_HZ).map(|_| Message::TimerTick);

            subscriptions.push(emulate);
            subscriptions.push(timer);
        }

        Subscription::batch(subscriptions)
    }
}

async fn pick_file() -> Option<PathBuf> {
    AsyncFileDialog::new()
        .set_title("Select Program")
        .pick_file()
        .await
        .map(PathBuf::from)
}

async fn load_program(path: impl AsRef<Path>) -> Result<Program, io::ErrorKind> {
    let name = path
        .as_ref()
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| String::from("unknown"));
    let data = tokio::fs::read(path).await.map_err(|err| err.kind())?;
    Ok(Program { name, data })
}

fn convert_to_rgba(data: &[bool]) -> Vec<u8> {
    data.iter()
        .map(|&pixel| if pixel { Color::WHITE } else { Color::BLACK })
        .flat_map(Color::into_rgba8)
        .collect()
}

fn get_key_idx(key: &Key) -> Option<usize> {
    match key.as_ref() {
        Key::Character("1") => Some(0x1),
        Key::Character("2") => Some(0x2),
        Key::Character("3") => Some(0x3),
        Key::Character("4") => Some(0xC),
        Key::Character("q" | "Q") => Some(0x4),
        Key::Character("w" | "W") => Some(0x5),
        Key::Character("e" | "E") => Some(0x6),
        Key::Character("r" | "R") => Some(0xD),
        Key::Character("a" | "A") => Some(0x7),
        Key::Character("s" | "S") => Some(0x8),
        Key::Character("d" | "D") => Some(0x9),
        Key::Character("f" | "F") => Some(0xE),
        Key::Character("z" | "Z") => Some(0xA),
        Key::Character("x" | "X") => Some(0x0),
        Key::Character("c" | "C") => Some(0xB),
        Key::Character("v" | "V") => Some(0xF),
        _ => None,
    }
}

fn menu(items: Vec<Item<'_, Message>>) -> Menu<'_, Message> {
    Menu::new(items).max_width(120.0).offset(5.0).spacing(5.0)
}

fn menu_header(label: &str) -> Button<'_, Message> {
    menu_button(label).width(Length::Shrink)
}

fn menu_item(label: &str) -> Button<'_, Message> {
    menu_button(label).width(Length::Fill)
}

fn menu_button(label: &str) -> Button<'_, Message> {
    button(text(label).align_y(Vertical::Center))
        .padding([4, 8])
        .style(|_, _| button::Style::default())
}

fn menu_checkbox(label: &str, is_checked: bool) -> Checkbox<'_, Message> {
    checkbox(is_checked).label(label).width(Length::Fill)
}

fn cycles_per_second(hertz: u32) -> Subscription<Instant> {
    iced::time::every(Duration::from_secs(1) / hertz)
}
