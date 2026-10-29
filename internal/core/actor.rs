// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Runtime support for `@actor` components: native code (the actor) that the
//! application registers for a component and that is created once per
//! instance of that component.
//!
//! The application registers the actors on a `SlintContext`. For each `@actor`
//! component, the Rust generator emits a facade type and a message enum. The
//! facade gives the actor access to the instance's properties; every callback
//! invocation becomes a message that is delivered synchronously to the actor's
//! message handler.

use crate::api::PlatformError;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::rc::Rc;
use core::any::TypeId;
use core::cell::RefCell;

/// Implemented by the facade types generated for `@actor` components.
pub trait ActorFacade: 'static {
    /// The message enum generated from the component's callbacks.
    type Message: 'static;
}

/// The handler that receives an actor's messages.
type MessageHandler<F> = Box<dyn FnMut(&F, <F as ActorFacade>::Message)>;

/// A registered actor: invoked once per component instance with its facade,
/// it returns the handler for the instance's messages.
pub type ActorFactory<F> = dyn Fn(&F) -> MessageHandler<F>;

/// Connects one component instance to its actor: owns the facade and the
/// actor's message handler, and delivers the messages that the instance's
/// callback handlers send.
///
/// The component instance owns the dispatcher, so the actor's state is
/// dropped together with the instance.
pub struct ActorDispatcher<F: ActorFacade> {
    facade: F,
    handler: RefCell<Option<MessageHandler<F>>>,
    queue: RefCell<VecDeque<F::Message>>,
}

impl<F: ActorFacade> ActorDispatcher<F> {
    pub fn new(facade: F) -> Rc<Self> {
        Rc::new(Self { facade, handler: RefCell::new(None), queue: Default::default() })
    }

    /// Invoke the actor's factory and deliver the messages sent so far.
    pub fn start(&self, factory: &ActorFactory<F>) {
        let handler = factory(&self.facade);
        *self.handler.borrow_mut() = Some(handler);
        self.dispatch();
    }

    /// Deliver `message` to the actor's handler. A message sent while the
    /// handler runs - for example because it set a property whose change
    /// invokes a callback - is delivered after the handler returns.
    pub fn send(&self, message: F::Message) {
        self.queue.borrow_mut().push_back(message);
        self.dispatch();
    }

    fn dispatch(&self) {
        // Already borrowed: we're inside the handler, the outer call delivers.
        let Ok(mut handler) = self.handler.try_borrow_mut() else { return };
        let Some(handler) = handler.as_mut() else { return };
        loop {
            let Some(message) = self.queue.borrow_mut().pop_front() else { break };
            handler(&self.facade, message);
        }
    }
}

/// Check that `ctx` has an actor registered for each of the `required`
/// facade types, given with the component name for the error message.
pub fn actor_validate(
    ctx: &crate::SlintContext,
    required: &[(TypeId, &str)],
) -> Result<(), PlatformError> {
    for (type_id, name) in required {
        if !ctx.has_actor_factory(*type_id) {
            return Err(PlatformError::Other(alloc::format!(
                "No actor is registered on the SlintContext for the @actor component '{name}'. \
                Register one with SlintContext::register_actor() before creating the component"
            )));
        }
    }
    Ok(())
}

/// The error of the constructors that create an instance of `component`, which
/// contains `@actor` components, without an explicit Slint context.
pub fn actors_require_context(component: &str) -> Result<(), PlatformError> {
    Err(PlatformError::Other(alloc::format!(
        "'{component}' contains @actor components and needs a SlintContext to find their actors. \
        Create it with {component}::new_with_context()"
    )))
}
