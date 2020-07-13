/*! Module handling mouse events

TODO: Keyboard events
*/

use crate::abi::datastructures::MouseEvent;
use crate::ComponentRefPin;
use euclid::default::Vector2D;

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
