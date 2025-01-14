use crate::messages::input_mapper::utility_types::input_keyboard::{Key, KeyStates, ModifierKeys};
use crate::messages::input_mapper::utility_types::input_mouse::{MouseKeys, MouseState, ViewportBounds};
use crate::messages::portfolio::document::utility_types::misc::KeyboardPlatformLayout;
use crate::messages::prelude::*;

#[doc(inline)]
pub use graphene::DocumentResponse;

use glam::DVec2;

#[derive(Debug, Default)]
pub struct InputPreprocessorMessageHandler {
	pub keyboard: KeyStates,
	pub mouse: MouseState,
	pub viewport_bounds: ViewportBounds,
}

impl MessageHandler<InputPreprocessorMessage, KeyboardPlatformLayout> for InputPreprocessorMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: InputPreprocessorMessage, data: KeyboardPlatformLayout, responses: &mut VecDeque<Message>) {
		let keyboard_platform = data;

		#[remain::sorted]
		match message {
			InputPreprocessorMessage::BoundsOfViewports { bounds_of_viewports } => {
				assert_eq!(bounds_of_viewports.len(), 1, "Only one viewport is currently supported");

				for bounds in bounds_of_viewports {
					let new_size = bounds.size();
					let existing_size = self.viewport_bounds.size();

					let translation = (new_size - existing_size) / 2.;

					// TODO: Extend this to multiple viewports instead of setting it to the value of this last loop iteration
					self.viewport_bounds = bounds;

					responses.push_back(
						graphene::Operation::TransformLayer {
							path: vec![],
							transform: glam::DAffine2::from_translation(translation).to_cols_array(),
						}
						.into(),
					);
					responses.push_back(
						DocumentMessage::Artboard(
							graphene::Operation::TransformLayer {
								path: vec![],
								transform: glam::DAffine2::from_translation(translation).to_cols_array(),
							}
							.into(),
						)
						.into(),
					);
					responses.push_back(FrontendMessage::TriggerViewportResize.into());
				}
			}
			InputPreprocessorMessage::DoubleClick { editor_mouse_state, modifier_keys } => {
				self.handle_modifier_keys(modifier_keys, keyboard_platform, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;

				responses.push_back(InputMapperMessage::DoubleClick.into());
			}
			InputPreprocessorMessage::KeyDown { key, modifier_keys } => {
				self.handle_modifier_keys(modifier_keys, keyboard_platform, responses);
				self.keyboard.set(key as usize);
				responses.push_back(InputMapperMessage::KeyDown(key).into());
			}
			InputPreprocessorMessage::KeyUp { key, modifier_keys } => {
				self.handle_modifier_keys(modifier_keys, keyboard_platform, responses);
				self.keyboard.unset(key as usize);
				responses.push_back(InputMapperMessage::KeyUp(key).into());
			}
			InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys } => {
				self.handle_modifier_keys(modifier_keys, keyboard_platform, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;

				self.translate_mouse_event(mouse_state, true, responses);
			}
			InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys } => {
				self.handle_modifier_keys(modifier_keys, keyboard_platform, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;

				responses.push_back(InputMapperMessage::PointerMove.into());

				// While any pointer button is already down, additional button down events are not reported, but they are sent as `pointermove` events
				self.translate_mouse_event(mouse_state, false, responses);
			}
			InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys } => {
				self.handle_modifier_keys(modifier_keys, keyboard_platform, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;

				self.translate_mouse_event(mouse_state, false, responses);
			}
			InputPreprocessorMessage::WheelScroll { editor_mouse_state, modifier_keys } => {
				self.handle_modifier_keys(modifier_keys, keyboard_platform, responses);

				let mouse_state = editor_mouse_state.to_mouse_state(&self.viewport_bounds);
				self.mouse.position = mouse_state.position;
				self.mouse.scroll_delta = mouse_state.scroll_delta;

				responses.push_back(InputMapperMessage::WheelScroll.into());
			}
		};
	}

	// Clean user input and if possible reconstruct it.
	// Store the changes in the keyboard if it is a key event.
	// Transform canvas coordinates to document coordinates.
	advertise_actions!();
}

impl InputPreprocessorMessageHandler {
	fn translate_mouse_event(&mut self, mut new_state: MouseState, allow_first_button_down: bool, responses: &mut VecDeque<Message>) {
		for (bit_flag, key) in [(MouseKeys::LEFT, Key::Lmb), (MouseKeys::RIGHT, Key::Rmb), (MouseKeys::MIDDLE, Key::Mmb)] {
			// Calculate the intersection between the two key states
			let old_down = self.mouse.mouse_keys & bit_flag == bit_flag;
			let new_down = new_state.mouse_keys & bit_flag == bit_flag;
			if !old_down && new_down {
				if allow_first_button_down || self.mouse.mouse_keys != MouseKeys::NONE {
					responses.push_back(InputMapperMessage::KeyDown(key).into());
				} else {
					// Required to stop a keyup being emitted for a keydown outside canvas
					new_state.mouse_keys ^= bit_flag;
				}
			}
			if old_down && !new_down {
				responses.push_back(InputMapperMessage::KeyUp(key).into());
			}
		}

		self.mouse = new_state;
	}

	fn handle_modifier_keys(&mut self, modifier_keys: ModifierKeys, keyboard_platform: KeyboardPlatformLayout, responses: &mut VecDeque<Message>) {
		self.handle_modifier_key(Key::KeyShift, modifier_keys.contains(ModifierKeys::SHIFT), responses);
		self.handle_modifier_key(Key::KeyAlt, modifier_keys.contains(ModifierKeys::ALT), responses);
		self.handle_modifier_key(Key::KeyControl, modifier_keys.contains(ModifierKeys::CONTROL), responses);
		let meta_or_command = match keyboard_platform {
			KeyboardPlatformLayout::Mac => Key::KeyCommand,
			KeyboardPlatformLayout::Standard => Key::KeyMeta,
		};
		self.handle_modifier_key(meta_or_command, modifier_keys.contains(ModifierKeys::META_OR_COMMAND), responses);
	}

	fn handle_modifier_key(&mut self, key: Key, key_is_down: bool, responses: &mut VecDeque<Message>) {
		let key_was_down = self.keyboard.get(key as usize);

		if key_was_down && !key_is_down {
			self.keyboard.unset(key as usize);
			responses.push_back(InputMapperMessage::KeyUp(key).into());
		} else if !key_was_down && key_is_down {
			self.keyboard.set(key as usize);
			responses.push_back(InputMapperMessage::KeyDown(key).into());
		}
	}

	pub fn document_bounds(&self) -> [DVec2; 2] {
		// IPP bounds are relative to the entire application
		[(0., 0.).into(), self.viewport_bounds.bottom_right - self.viewport_bounds.top_left]
	}
}

#[cfg(test)]
mod test {
	use crate::messages::input_mapper::utility_types::input_keyboard::{Key, ModifierKeys};
	use crate::messages::input_mapper::utility_types::input_mouse::EditorMouseState;
	use crate::messages::portfolio::document::utility_types::misc::KeyboardPlatformLayout;
	use crate::messages::prelude::*;

	#[test]
	fn process_action_mouse_move_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let editor_mouse_state = EditorMouseState::from_editor_position(4., 809.);
		let modifier_keys = ModifierKeys::ALT;
		let message = InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_message(message, KeyboardPlatformLayout::Standard, &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyAlt as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::KeyAlt).into()));
	}

	#[test]
	fn process_action_mouse_down_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let editor_mouse_state = EditorMouseState::new();
		let modifier_keys = ModifierKeys::CONTROL;
		let message = InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_message(message, KeyboardPlatformLayout::Standard, &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyControl as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::KeyControl).into()));
	}

	#[test]
	fn process_action_mouse_up_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let editor_mouse_state = EditorMouseState::new();
		let modifier_keys = ModifierKeys::SHIFT;
		let message = InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_message(message, KeyboardPlatformLayout::Standard, &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyShift as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyDown(Key::KeyShift).into()));
	}

	#[test]
	fn process_action_key_down_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();
		input_preprocessor.keyboard.set(Key::KeyControl as usize);

		let key = Key::KeyA;
		let modifier_keys = ModifierKeys::empty();
		let message = InputPreprocessorMessage::KeyDown { key, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_message(message, KeyboardPlatformLayout::Standard, &mut responses);

		assert!(!input_preprocessor.keyboard.get(Key::KeyControl as usize));
		assert_eq!(responses.pop_front(), Some(InputMapperMessage::KeyUp(Key::KeyControl).into()));
	}

	#[test]
	fn process_action_key_up_handle_modifier_keys() {
		let mut input_preprocessor = InputPreprocessorMessageHandler::default();

		let key = Key::KeyS;
		let modifier_keys = ModifierKeys::CONTROL | ModifierKeys::SHIFT;
		let message = InputPreprocessorMessage::KeyUp { key, modifier_keys };

		let mut responses = VecDeque::new();

		input_preprocessor.process_message(message, KeyboardPlatformLayout::Standard, &mut responses);

		assert!(input_preprocessor.keyboard.get(Key::KeyControl as usize));
		assert!(input_preprocessor.keyboard.get(Key::KeyShift as usize));
		assert!(responses.contains(&InputMapperMessage::KeyDown(Key::KeyControl).into()));
		assert!(responses.contains(&InputMapperMessage::KeyDown(Key::KeyControl).into()));
	}
}
