use std::collections::HashMap;
use history_survival_common::debug::send_debug_info;
use history_survival_common::player::PlayerInput;
use history_survival_common::physics::player::YawPitch;
use winit::event::{ElementState, KeyboardInput, ModifiersState, MouseButton};

/// The state of the keyboard and mouse buttons.
pub struct InputState {
    keys: HashMap<u32, ElementState>,
    mouse_buttons: HashMap<MouseButton, ElementState>,
    modifiers_state: ModifiersState,
    flying: bool,             // TODO: reset this on game start
    pub enable_culling: bool, // TODO: don't put this here
}

impl InputState {
    pub fn new() -> InputState {
        Self {
            keys: HashMap::new(),
            mouse_buttons: HashMap::new(),
            modifiers_state: ModifiersState::default(),
            flying: true,
            enable_culling: true,
        }
    }

    /// Process a keyboard input, returning whether the state of the key changed or not
    pub fn process_keyboard_input(&mut self, input: KeyboardInput) -> bool {
        let previous_state = self.keys.get(&input.scancode).cloned();
        self.keys.insert(input.scancode, input.state);
        previous_state != Some(input.state)
    }

    /// Process a mouse input, returning whether the state of the button changed or not
    pub fn process_mouse_input(
        &mut self,
        state: ElementState,
        button: MouseButton,
    ) -> bool {
        let previous_state = self.mouse_buttons.get(&button).cloned();
        self.mouse_buttons.insert(button, state);
        previous_state != Some(state)
    }

    /// Update the modifiers
    pub fn set_modifiers_state(&mut self, modifiers_state: ModifiersState) {
        self.modifiers_state = modifiers_state;
    }

    pub fn _get_modifiers_state(&self) -> ModifiersState {
        self.modifiers_state
    }

    pub fn get_key_state(&self, scancode: u32) -> ElementState {
        self.keys
            .get(&scancode)
            .cloned()
            .unwrap_or(ElementState::Released)
    }

    pub fn clear(&mut self) {
        self.keys.clear();
        self.mouse_buttons.clear();
        self.modifiers_state = ModifiersState::default();
    }

    fn is_key_pressed(&self, scancode: u32) -> bool {
        match self.get_key_state(scancode) {
            ElementState::Pressed => true,
            ElementState::Released => false,
        }
    }

    // TODO: add configuration for this
    pub fn get_physics_input(&self, yaw_pitch: YawPitch, allow_movement: bool) -> PlayerInput {
        PlayerInput {
            key_move_forward: allow_movement && self.is_key_pressed(MOVE_FORWARD),
            key_move_left: allow_movement && self.is_key_pressed(MOVE_LEFT),
            key_move_backward: allow_movement && self.is_key_pressed(MOVE_BACKWARD),
            key_move_right: allow_movement && self.is_key_pressed(MOVE_RIGHT),
            key_move_up: allow_movement && self.is_key_pressed(MOVE_UP),
            key_move_down: allow_movement && self.is_key_pressed(MOVE_DOWN),
            key_rotate_left: allow_movement && self.is_key_pressed(ROTATE_LEFT),
            key_rotate_right: allow_movement && self.is_key_pressed(ROTATE_RIGHT),
            yaw_pitch: yaw_pitch,
            flying: self.flying,
        }
    }
}

pub const MOVE_FORWARD: u32 = 17;
pub const MOVE_LEFT: u32 = 30;
pub const MOVE_BACKWARD: u32 = 31;
pub const MOVE_RIGHT: u32 = 32;
pub const MOVE_UP: u32 = 57;
pub const MOVE_DOWN: u32 = 42;
pub const ROTATE_LEFT: u32 = 16;
pub const ROTATE_RIGHT: u32 = 18;
