//! Component snapshots for the debugger's Components view.

use std::{any::type_name, cell::RefCell, rc::Rc};

use ratatui::{layout::Rect, Frame};
use serde::Serialize;
use tui_dispatch::Component;
use tui_dispatch_debug::debug::{ComponentSnapshot, DebugLayer};

use crate::action::Action;

/// Tracks the components rendered in the latest frame. Clones share snapshots.
/// Disabled by default to avoid collecting debug data during normal play.
#[derive(Clone, Default)]
pub struct ComponentRegistry {
    snapshots: Option<Rc<RefCell<Vec<ComponentSnapshot>>>>,
}

impl ComponentRegistry {
    pub fn enabled() -> Self {
        Self {
            snapshots: Some(Rc::default()),
        }
    }

    pub fn debug_layer(&self) -> DebugLayer<Action> {
        let registry = self.clone();
        DebugLayer::simple().with_component_snapshotter(move || registry.snapshots())
    }

    pub fn snapshots(&self) -> Vec<ComponentSnapshot> {
        self.snapshots
            .as_ref()
            .map(|snapshots| snapshots.borrow().clone())
            .unwrap_or_default()
    }

    pub(crate) fn begin_frame(&self) {
        if let Some(snapshots) = &self.snapshots {
            snapshots.borrow_mut().clear();
        }
    }

    pub(crate) fn record<C>(&self, area: Rect, id: impl Into<String>, state: &impl Serialize) {
        let Some(snapshots) = &self.snapshots else {
            return;
        };
        let debug_entries = match serde_json::to_value(state) {
            Ok(serde_json::Value::Object(fields)) => fields
                .into_iter()
                .map(|(name, value)| (name, value.to_string()))
                .collect(),
            Ok(value) => vec![("state".into(), value.to_string())],
            Err(error) => vec![("error".into(), error.to_string())],
        };
        let mut snapshots = snapshots.borrow_mut();
        let raw_id = snapshots.len() as u32;
        snapshots.push(ComponentSnapshot {
            raw_id,
            type_name: type_name::<C>().rsplit("::").next().unwrap().to_owned(),
            type_name_full: type_name::<C>().to_owned(),
            bound_id: Some(id.into()),
            last_area: Some(area),
            debug_entries,
        });
    }

    pub(crate) fn render<C: Component<Action>>(
        &self,
        mut component: C,
        frame: &mut Frame,
        area: Rect,
        props: C::Props<'_>,
        id: impl Into<String>,
        state: &impl Serialize,
    ) {
        self.record::<C>(area, id, state);
        component.render(frame, area, props);
    }
}
