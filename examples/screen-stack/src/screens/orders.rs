// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint::{ModelRc, SharedString, VecModel};
use slint_interpreter::{ComponentInstance, Struct, Value};
use std::rc::Rc;

pub struct Order {
    pub id: String,
    pub icon: String,
    pub title: String,
    pub subtitle: String,
}

pub struct OrdersScreen {
    store: Rc<AppStore>,
    orders: Vec<Order>,
}

impl OrdersScreen {
    pub fn create(store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[OrdersScreen] Controller CREATED");

        let orders = vec![
            Order {
                id: "ord-001".into(),
                icon: "📦".into(),
                title: "Order #001".into(),
                subtitle: "Delivered - Jan 15".into(),
            },
            Order {
                id: "ord-002".into(),
                icon: "🚚".into(),
                title: "Order #002".into(),
                subtitle: "In Transit - Jan 18".into(),
            },
            Order {
                id: "ord-003".into(),
                icon: "⏳".into(),
                title: "Order #003".into(),
                subtitle: "Processing - Jan 20".into(),
            },
            Order {
                id: "ord-004".into(),
                icon: "📦".into(),
                title: "Order #004".into(),
                subtitle: "Delivered - Jan 10".into(),
            },
        ];

        Box::new(Self { store, orders })
    }

    fn to_list_item(order: &Order) -> Value {
        let mut item = Struct::default();
        item.set_field("id".into(), Value::String(SharedString::from(&order.id)));
        item.set_field("icon".into(), Value::String(SharedString::from(&order.icon)));
        item.set_field("title".into(), Value::String(SharedString::from(&order.title)));
        item.set_field("subtitle".into(), Value::String(SharedString::from(&order.subtitle)));
        item.set_field("has-arrow".into(), Value::Bool(true));
        Value::Struct(item)
    }
}

impl Drop for OrdersScreen {
    fn drop(&mut self) {
        println!("[OrdersScreen] Controller DESTROYED");
    }
}

impl Screen for OrdersScreen {
    fn name(&self) -> &'static str {
        "orders"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        ScreenCallbackHelper::setup_navigate(instance, ctx);

        // Set orders from Rust
        let items: Vec<Value> = self.orders.iter().map(Self::to_list_item).collect();
        let model = ModelRc::new(VecModel::from(items));
        let _ = instance.set_property("orders", Value::Model(model));

        // Handle item clicks - save ID and navigate to detail
        let store = self.store.clone();
        let ctx = ctx.clone();
        let _ = instance.set_callback("item-clicked", move |args| {
            if let Some(Value::String(id)) = args.first() {
                println!("[OrdersScreen] Order clicked: {}", id);
                store.set("selected_order_id", id.to_string());
                ctx.push("order-detail");
            }
            Value::Void
        });

        println!("[OrdersScreen] on_loaded with {} orders", self.orders.len());
    }
}

impl ScreenWithHeader for OrdersScreen {
    fn title(&self) -> &str {
        "Orders"
    }
}
