/*! Module handling mouse events

TODO: Keyboard events
*/

use crate::graphics::Point;
use crate::ComponentRefPin;
use euclid::default::Vector2D;

/// The type of a MouseEvent
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum MouseEventType {
    /// The mouse was pressed
    MousePressed,
    /// The mouse was relased
    MouseReleased,
    /// The mouse position has changed
    MouseMoved,
}

/// Structur representing a mouse event
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    /// The position of the cursor
    pub pos: Point,
    /// The action performed (pressed/released/moced)
    pub what: MouseEventType,
}

pub fn process_mouse_event(component: ComponentRefPin, event: MouseEvent) {
    let offset = Vector2D::new(0., 0.);

    crate::item_tree::visit_items(
        component,
        |_, item, offset| {
            let geom = item.as_ref().geometry();
            let geom = geom.translate(*offset);

            if geom.contains(event.pos) {
                let mut event2 = event.clone();
                event2.pos -= geom.origin.to_vector();
                item.as_ref().input_event(event2);
            }

            geom.origin.to_vector()
        },
        offset,
    );
}
