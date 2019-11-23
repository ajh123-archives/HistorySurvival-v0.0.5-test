use self::widgets::{Text, WithStyle};
use crate::ui::renderer::TextPart;
use crate::ui::widgets::Button;
use crate::window::WindowData;
use anyhow::Result;
use wgpu_glyph::Scale;
use winit::dpi::LogicalPosition;
use quint::{wt, Size, Style, WidgetTree};
use std::collections::HashMap;
use voxel_rs_common::debug::DebugInfo;

pub mod renderer;
pub mod widgets;

#[derive(Debug, Clone, Copy)]
pub enum Message {
    ExitMenu,
    ExitGame,
}

pub struct Ui {
    pub ui: quint::Ui<renderer::PrimitiveBuffer, Message>,
    messages: Vec<Message>,
    show_menu: bool,
    should_exit: bool,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            ui: quint::Ui::new(),
            messages: Vec::new(),
            show_menu: false,
            should_exit: false,
        }
    }

    pub fn cursor_moved(&mut self, p: LogicalPosition) {
        self.ui.set_cursor_position(quint::Position {
            x: p.x as f32,
            y: p.y as f32,
        });
    }

    pub fn should_update_camera(&self) -> bool {
        !self.show_menu
    }

    /// Rebuild the Ui if it changed
    pub fn rebuild(&mut self, debug_info: &mut DebugInfo, data: &WindowData) -> Result<()> {
        self.update();

        let mut layers = Vec::new();

        // Always draw debug info
        {
            layers.push(self.draw_debug_info(debug_info.get_debug_info()));
        }

        // Draw menu
        if self.show_menu {
            layers.push(self.draw_menu());
        }

        let (win_w, win_h) = (
            data.logical_window_size.width,
            data.logical_window_size.height,
        );
        self.ui.rebuild(
            layers,
            Size {
                width: win_w as f32,
                height: win_h as f32,
            },
        );

        Ok(())
    }

    fn draw_debug_info(
        &self,
        debug_info: HashMap<String, HashMap<String, String>>,
    ) -> WidgetTree<renderer::PrimitiveBuffer, Message> {
        let white = [1.0, 1.0, 1.0, 1.0];
        let mut text = debug_info
            .into_iter()
            .map(|(section, messages)| {
                vec![
                    TextPart {
                        text: format!("\n{}", section),
                        font_size: Scale::uniform(25.0),
                        color: white,
                        font: Some("medium_italic".to_owned()),
                    },
                    TextPart {
                        text: " DEBUG INFO\n".to_owned(),
                        font_size: Scale::uniform(25.0),
                        color: white,
                        font: Some("regular".to_owned()),
                    },
                    TextPart {
                        text: messages
                            .into_iter()
                            .map(|(_id, m)| m)
                            .collect::<Vec<String>>()
                            .join("\n"),
                        font_size: Scale::uniform(20.0),
                        color: white,
                        font: Some("regular".to_owned()),
                    },
                ]
            })
            .flatten()
            .collect::<Vec<TextPart>>();

        text.insert(
            0,
            TextPart {
                text: format!("VOXEL-RS\n"),
                font_size: Scale::uniform(40.0),
                color: white,
                font: Some("medium".to_owned()),
            },
        );

        wt! {
            WithStyle { style: Style::default().percent_size(1.0, 1.0) },
            wt! {
                Text { text },
            },
        }
    }

    fn draw_menu(&self) -> WidgetTree<renderer::PrimitiveBuffer, Message> {
        let menu_button = |text: &'static str, message| {
            wt! {
                Button {
                    text: vec![
                        TextPart {
                            text: text.to_owned(),
                            font_size: Scale::uniform(50.0),
                            color: [1.0, 1.0, 1.0, 1.0],
                            font: Some("arcade".to_owned()),
                        },
                    ],
                    message,
                    style: Style::default().absolute_size(400.0, 100.0),
                },
            }
        };

        let buttons_container = WidgetTree::new(
            Box::new(WithStyle {
                style: Style::default()
                    .percent_size(1.0, 1.0)
                    .center_cross()
                    .center_main()
                    .vertical(),
            }),
            vec![
                menu_button("RESUME", Message::ExitMenu),
                menu_button("EXIT", Message::ExitGame),
            ],
        );
        buttons_container
    }

    pub fn handle_mouse_state_changes(
        &mut self,
        changes: Vec<(winit::event::MouseButton, winit::event::ElementState)>,
    ) {
        let changes = changes
            .into_iter()
            .map(|(button, state)| quint::Event::MouseInput {
                button: quint_mouse_button(button),
                state: quint_element_state(state),
            })
            .collect();
        self.messages.extend(self.ui.update(changes));
    }

    pub fn handle_key_state_changes(&mut self, changes: Vec<(u32, winit::event::ElementState)>) {
        for (key, state) in changes.into_iter() {
            // Escape key
            if key == 1 {
                if let winit::event::ElementState::Pressed = state {
                    self.show_menu = !self.show_menu;
                }
            }
        }
    }

    fn update(&mut self) {
        for message in self.messages.drain(..) {
            match message {
                Message::ExitMenu => self.show_menu = false,
                Message::ExitGame => self.should_exit = true,
            }
        }
    }

    pub fn should_capture_mouse(&self) -> bool {
        !self.show_menu
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }
}

pub fn quint_mouse_button(button: winit::event::MouseButton) -> quint::MouseButton {
    use winit::event::MouseButton::*;
    match button {
        Left => quint::MouseButton::Left,
        Right => quint::MouseButton::Right,
        Middle => quint::MouseButton::Middle,
        Other(x) => quint::MouseButton::Other(x),
    }
}

pub fn quint_element_state(state: winit::event::ElementState) -> quint::ButtonState {
    match state {
        winit::event::ElementState::Pressed => quint::ButtonState::Pressed,
        winit::event::ElementState::Released => quint::ButtonState::Released,
    }
}
