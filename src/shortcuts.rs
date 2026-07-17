use eframe::egui::{Event, InputState, Key, KeyboardShortcut, Modifiers};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModifierKey {
    Shift,
    Alt,
    Control,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShortcutBinding {
    Key(KeyboardShortcut),
    Modifier(ModifierKey),
}

impl ShortcutBinding {
    pub const fn key(modifiers: Modifiers, key: Key) -> Self {
        Self::Key(KeyboardShortcut::new(modifiers, key))
    }

    pub const fn plain(key: Key) -> Self {
        Self::key(Modifiers::NONE, key)
    }

    pub const fn modifier(modifier: ModifierKey) -> Self {
        Self::Modifier(modifier)
    }

    pub fn display(self, ctx: &egui::Context) -> String {
        match self {
            Self::Key(shortcut) => ctx.format_shortcut(&shortcut),
            Self::Modifier(ModifierKey::Shift) => "Shift".to_owned(),
            Self::Modifier(ModifierKey::Alt) => "Alt / Option".to_owned(),
            Self::Modifier(ModifierKey::Control) => "Ctrl".to_owned(),
            Self::Modifier(ModifierKey::Command) => "Command".to_owned(),
        }
    }

    pub fn pressed(self, input: &InputState) -> bool {
        match self {
            Self::Key(shortcut) => input.events.iter().any(|event| {
                matches!(
                    event,
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } if *key == shortcut.logical_key
                        && modifiers.matches_exact(shortcut.modifiers)
                )
            }),
            Self::Modifier(modifier) => input.events.iter().any(|event| {
                matches!(
                    event,
                    Event::Key { key, pressed: true, .. } if modifier_matches_key(modifier, *key)
                )
            }),
        }
    }

    pub fn held(self, input: &InputState) -> bool {
        match self {
            Self::Key(shortcut) => {
                input.key_down(shortcut.logical_key)
                    && input.modifiers.matches_exact(shortcut.modifiers)
            }
            Self::Modifier(ModifierKey::Shift) => input.modifiers.shift,
            Self::Modifier(ModifierKey::Alt) => input.modifiers.alt,
            Self::Modifier(ModifierKey::Control) => input.modifiers.ctrl,
            Self::Modifier(ModifierKey::Command) => input.modifiers.command,
        }
    }

    pub fn from_event(event: &Event) -> Option<Self> {
        let Event::Key {
            key,
            pressed,
            modifiers,
            ..
        } = event
        else {
            return None;
        };
        let modifier = match key {
            Key::ShiftLeft | Key::ShiftRight => Some(ModifierKey::Shift),
            Key::AltLeft | Key::AltRight => Some(ModifierKey::Alt),
            Key::ControlLeft | Key::ControlRight => Some(ModifierKey::Control),
            Key::SuperLeft | Key::SuperRight => Some(ModifierKey::Command),
            _ => None,
        };
        match modifier {
            Some(modifier) if !pressed => Some(Self::Modifier(modifier)),
            Some(_) => None,
            None if *pressed => Some(Self::Key(KeyboardShortcut::new(*modifiers, *key))),
            None => None,
        }
    }
}

fn modifier_matches_key(modifier: ModifierKey, key: Key) -> bool {
    matches!(
        (modifier, key),
        (ModifierKey::Shift, Key::ShiftLeft | Key::ShiftRight)
            | (ModifierKey::Alt, Key::AltLeft | Key::AltRight)
            | (ModifierKey::Control, Key::ControlLeft | Key::ControlRight)
            | (ModifierKey::Command, Key::SuperLeft | Key::SuperRight)
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortcutSettings {
    pub new_project: ShortcutBinding,
    pub open_project: ShortcutBinding,
    pub save_project: ShortcutBinding,
    pub save_project_as: ShortcutBinding,
    pub assets_tab: ShortcutBinding,
    pub themes_tab: ShortcutBinding,

    pub crop_delete: ShortcutBinding,
    pub crop_delete_alternate: ShortcutBinding,
    pub crop_pan_left: ShortcutBinding,
    pub crop_pan_right: ShortcutBinding,
    pub crop_pan_up: ShortcutBinding,
    pub crop_pan_down: ShortcutBinding,
    pub crop_move_left: ShortcutBinding,
    pub crop_move_right: ShortcutBinding,
    pub crop_move_up: ShortcutBinding,
    pub crop_move_down: ShortcutBinding,
    pub crop_move_left_alternate: ShortcutBinding,
    pub crop_move_right_alternate: ShortcutBinding,
    pub crop_move_up_alternate: ShortcutBinding,
    pub crop_move_down_alternate: ShortcutBinding,
    pub crop_resize_left: ShortcutBinding,
    pub crop_resize_right: ShortcutBinding,
    pub crop_resize_up: ShortcutBinding,
    pub crop_resize_down: ShortcutBinding,
    pub crop_resize_left_alternate: ShortcutBinding,
    pub crop_resize_right_alternate: ShortcutBinding,
    pub crop_resize_up_alternate: ShortcutBinding,
    pub crop_resize_down_alternate: ShortcutBinding,
    pub crop_draw_tool: ShortcutBinding,
    pub crop_select_tool: ShortcutBinding,
    pub crop_fit: ShortcutBinding,
    pub crop_extract: ShortcutBinding,
    pub crop_copy: ShortcutBinding,
    pub crop_cancel: ShortcutBinding,

    pub sprite_delete: ShortcutBinding,
    pub sprite_delete_alternate: ShortcutBinding,
    pub sprite_pan_left: ShortcutBinding,
    pub sprite_pan_right: ShortcutBinding,
    pub sprite_pan_up: ShortcutBinding,
    pub sprite_pan_down: ShortcutBinding,
    pub sprite_erase: ShortcutBinding,
    pub sprite_restore: ShortcutBinding,
    pub sprite_pick_color: ShortcutBinding,
    pub sprite_pick_color_alternate: ShortcutBinding,
    pub sprite_crop_mode: ShortcutBinding,
    pub sprite_hide_overlays: ShortcutBinding,
    pub sprite_fit: ShortcutBinding,
    pub sprite_brush_smaller: ShortcutBinding,
    pub sprite_brush_larger: ShortcutBinding,
    pub sprite_undo: ShortcutBinding,
    pub sprite_redo: ShortcutBinding,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        let command = Modifiers::COMMAND;
        let command_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let ctrl = Modifiers::CTRL;
        let shift = Modifiers::SHIFT;
        Self {
            new_project: ShortcutBinding::key(command, Key::N),
            open_project: ShortcutBinding::key(command, Key::O),
            save_project: ShortcutBinding::key(command, Key::S),
            save_project_as: ShortcutBinding::key(command_shift, Key::S),
            assets_tab: ShortcutBinding::key(command, Key::Num1),
            themes_tab: ShortcutBinding::key(command, Key::Num2),

            crop_delete: ShortcutBinding::plain(Key::Delete),
            crop_delete_alternate: ShortcutBinding::plain(Key::Backspace),
            crop_pan_left: ShortcutBinding::key(ctrl, Key::ArrowLeft),
            crop_pan_right: ShortcutBinding::key(ctrl, Key::ArrowRight),
            crop_pan_up: ShortcutBinding::key(ctrl, Key::ArrowUp),
            crop_pan_down: ShortcutBinding::key(ctrl, Key::ArrowDown),
            crop_move_left: ShortcutBinding::plain(Key::ArrowLeft),
            crop_move_right: ShortcutBinding::plain(Key::ArrowRight),
            crop_move_up: ShortcutBinding::plain(Key::ArrowUp),
            crop_move_down: ShortcutBinding::plain(Key::ArrowDown),
            crop_move_left_alternate: ShortcutBinding::plain(Key::A),
            crop_move_right_alternate: ShortcutBinding::plain(Key::D),
            crop_move_up_alternate: ShortcutBinding::plain(Key::W),
            crop_move_down_alternate: ShortcutBinding::plain(Key::S),
            crop_resize_left: ShortcutBinding::key(shift, Key::ArrowLeft),
            crop_resize_right: ShortcutBinding::key(shift, Key::ArrowRight),
            crop_resize_up: ShortcutBinding::key(shift, Key::ArrowUp),
            crop_resize_down: ShortcutBinding::key(shift, Key::ArrowDown),
            crop_resize_left_alternate: ShortcutBinding::key(shift, Key::A),
            crop_resize_right_alternate: ShortcutBinding::key(shift, Key::D),
            crop_resize_up_alternate: ShortcutBinding::key(shift, Key::W),
            crop_resize_down_alternate: ShortcutBinding::key(shift, Key::S),
            crop_draw_tool: ShortcutBinding::plain(Key::C),
            crop_select_tool: ShortcutBinding::plain(Key::V),
            crop_fit: ShortcutBinding::plain(Key::F),
            crop_extract: ShortcutBinding::plain(Key::Enter),
            crop_copy: ShortcutBinding::key(command, Key::C),
            crop_cancel: ShortcutBinding::plain(Key::Escape),

            sprite_delete: ShortcutBinding::plain(Key::Delete),
            sprite_delete_alternate: ShortcutBinding::plain(Key::Backspace),
            sprite_pan_left: ShortcutBinding::key(ctrl, Key::ArrowLeft),
            sprite_pan_right: ShortcutBinding::key(ctrl, Key::ArrowRight),
            sprite_pan_up: ShortcutBinding::key(ctrl, Key::ArrowUp),
            sprite_pan_down: ShortcutBinding::key(ctrl, Key::ArrowDown),
            sprite_erase: ShortcutBinding::plain(Key::E),
            sprite_restore: ShortcutBinding::plain(Key::R),
            sprite_pick_color: ShortcutBinding::plain(Key::C),
            sprite_pick_color_alternate: ShortcutBinding::plain(Key::P),
            sprite_crop_mode: ShortcutBinding::modifier(ModifierKey::Alt),
            sprite_hide_overlays: ShortcutBinding::modifier(ModifierKey::Shift),
            sprite_fit: ShortcutBinding::plain(Key::F),
            sprite_brush_smaller: ShortcutBinding::plain(Key::OpenBracket),
            sprite_brush_larger: ShortcutBinding::plain(Key::CloseBracket),
            sprite_undo: ShortcutBinding::key(command, Key::Z),
            sprite_redo: ShortcutBinding::key(command_shift, Key::Z),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_settings_deserialize_from_an_empty_legacy_object() {
        let settings: ShortcutSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, ShortcutSettings::default());
    }

    #[test]
    fn shortcut_binding_round_trips() {
        let binding = ShortcutBinding::key(Modifiers::CTRL | Modifiers::SHIFT, Key::K);
        let json = serde_json::to_string(&binding).unwrap();
        assert_eq!(
            serde_json::from_str::<ShortcutBinding>(&json).unwrap(),
            binding
        );
    }
}
