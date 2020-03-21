//! This module defines possible mouse events.

use crate::prelude::*;

use crate::control::io::mouse::button::*;
use crate::system::web::dom::ShapeData;



// =============
// === Event ===
// =============

macro_rules! define_events {
    ( $( $js_event:ident :: $name:ident ),* $(,)? ) => {$(
        /// Mouse event wrapper.
        #[derive(Debug,Clone,From,Shrinkwrap)]
        pub struct $name {
            #[shrinkwrap(main_field)]
            raw   : web_sys::$js_event,
            shape : ShapeData,
        }
        impl $name {

            /// Constructor.
            pub fn new(raw:web_sys::$js_event,shape:ShapeData) -> Self {
                Self {raw,shape}
            }

            pub fn offset_y(&self) -> i32 {
                self.shape.height() as i32 - self.raw.offset_y()
            }

            pub fn client_y(&self) -> i32 {
                self.shape.height() as i32 - self.raw.client_y()
            }

            /// Translation of the button property to Rust `Button` enum.
            pub fn button(&self) -> Button {
                Button::from_code(self.raw.button())
            }
        }
    )*};
}

define_events! {
    MouseEvent::OnDown,
    MouseEvent::OnUp,
    MouseEvent::OnMove,
    MouseEvent::OnLeave,
    WheelEvent::OnWheel,
}
