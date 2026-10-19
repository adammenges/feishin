use crate::{app::Feishin, scroll::VerticalScroll};
use feishin_core::playback_filters::{self, Filter};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, StyledExt,
    checkbox::Checkbox,
    input::InputEvent,
    menu::{DropdownMenu, PopupMenuItem},
};

impl Feishin {
    pub fn playback_filters_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let filters = self.state.settings.playback_filters.clone();
        self.playback_filter_inputs
            .retain(|id, _| filters.iter().any(|f| &f.id == id));
        for filter in &filters {
            if self.playback_filter_inputs.contains_key(&filter.id) {
                continue;
            }
            let input = cx.new(|cx| {
                let mut input = crate::appearance::input_state(window, cx)
                    .aria_label(format!("Filter: {}", filter.id));
                input.set_value(
                    filter
                        .value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| filter.value.to_string()),
                    window,
                    cx,
                );
                input
            });
            let id = filter.id.clone();
            cx.subscribe(&input, move |this, input, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let value = input.read(cx).value().to_string();
                if let Some(filter) = this
                    .state
                    .settings
                    .playback_filters
                    .iter_mut()
                    .find(|f| f.id == id)
                {
                    filter.value = if playback_filters::numeric(&filter.field) {
                        value
                            .parse::<f64>()
                            .ok()
                            .filter(|v| v.is_finite())
                            .map(serde_json::Value::from)
                            .unwrap_or_else(|| value.into())
                    } else {
                        value.into()
                    };
                    this.save();
                    cx.notify();
                }
            })
            .detach();
            self.playback_filter_inputs.insert(filter.id.clone(), input);
        }
        let mut panel = div()
            .id("playback-filters-page")
            .debug_selector(|| "playback-filters-page".into())
            .v_flex()
            .size_full()
            .gap_4()
            .native_vertical_scroll()
            .child(
                self.tr("Omit songs from being added to the queue based on the following criteria"),
            )
            .child(
                crate::styles::button("add-playback-filter", cx)
                    .debug_selector(|| "add-playback-filter".into())
                    .label(self.tr("Add"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.playback_filters.push(Filter::default());
                        this.save();
                        cx.notify();
                    })),
            );
        for (index, filter) in filters.into_iter().enumerate() {
            let enabled_id = filter.id.clone();
            let remove_id = filter.id.clone();
            let field_id = filter.id.clone();
            let operator_id = filter.id.clone();
            let view = cx.weak_entity();
            let operator_view = view.clone();
            let field_label = playback_filters::FIELDS
                .iter()
                .find(|(field, _)| *field == filter.field)
                .map_or(filter.field.as_str(), |(_, label)| *label);
            let operator_label = playback_filters::OPERATORS
                .iter()
                .find(|(op, _)| *op == filter.operator)
                .map_or(filter.operator.as_str(), |(_, label)| *label);
            let mut row = div()
                .id(("playback-filter", index))
                .h_flex()
                .flex_wrap()
                .gap_2()
                .items_center()
                .child(
                    Checkbox::new(("playback-filter-enabled", index))
                        .debug_selector(move || format!("playback-filter-enabled-{index}"))
                        .label(self.tr("Enabled"))
                        .checked(filter.enabled)
                        .on_click(cx.listener(move |this, value: &bool, _, cx| {
                            if let Some(filter) = this
                                .state
                                .settings
                                .playback_filters
                                .iter_mut()
                                .find(|f| f.id == enabled_id)
                            {
                                filter.enabled = *value;
                            }
                            this.save();
                            cx.notify();
                        })),
                )
                .child(
                    crate::styles::button(("playback-filter-field", index), cx)
                        .debug_selector(move || format!("playback-filter-field-{index}"))
                        .label(self.tr(field_label))
                        .disabled(!filter.enabled)
                        .dropdown_menu(move |mut menu, _, cx| {
                            for (field, label) in playback_filters::FIELDS {
                                let view = view.clone();
                                let id = field_id.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(crate::appearance::text(label, cx))
                                        .on_click(move |_, window, cx| {
                                            view.update(cx, |this, cx| {
                                                if let Some(filter) = this
                                                    .state
                                                    .settings
                                                    .playback_filters
                                                    .iter_mut()
                                                    .find(|f| f.id == id)
                                                {
                                                    filter.field = (*field).into();
                                                    filter.operator = "is".into();
                                                    filter.value = if *field == "favorite" {
                                                        false.into()
                                                    } else if playback_filters::numeric(field) {
                                                        0.into()
                                                    } else {
                                                        "".into()
                                                    };
                                                    let value = filter
                                                        .value
                                                        .as_str()
                                                        .map(str::to_owned)
                                                        .unwrap_or_else(|| {
                                                            filter.value.to_string()
                                                        });
                                                    if let Some(input) =
                                                        this.playback_filter_inputs.get(&id)
                                                    {
                                                        input.update(cx, |input, cx| {
                                                            input.set_value(value, window, cx)
                                                        });
                                                    }
                                                }
                                                this.save();
                                                cx.notify();
                                            })
                                            .ok();
                                        }),
                                );
                            }
                            menu
                        }),
                )
                .child(
                    crate::styles::button(("playback-filter-operator", index), cx)
                        .debug_selector(move || format!("playback-filter-operator-{index}"))
                        .label(self.tr(operator_label))
                        .disabled(!filter.enabled)
                        .dropdown_menu({
                            let field = filter.field.clone();
                            move |mut menu, _, cx| {
                                for (operator, label) in playback_filters::operators(&field) {
                                    let view = operator_view.clone();
                                    let id = operator_id.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(crate::appearance::text(label, cx))
                                            .on_click(move |_, _, cx| {
                                                view.update(cx, |this, cx| {
                                                    if let Some(filter) = this
                                                        .state
                                                        .settings
                                                        .playback_filters
                                                        .iter_mut()
                                                        .find(|f| f.id == id)
                                                    {
                                                        filter.operator = operator.into();
                                                    }
                                                    this.save();
                                                    cx.notify();
                                                })
                                                .ok();
                                            }),
                                    );
                                }
                                menu
                            }
                        }),
                );
            if filter.field == "favorite" {
                let id = filter.id.clone();
                row = row.child(
                    crate::styles::button(("playback-filter-value", index), cx)
                        .label(filter.value.to_string())
                        .disabled(!filter.enabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(filter) = this
                                .state
                                .settings
                                .playback_filters
                                .iter_mut()
                                .find(|f| f.id == id)
                            {
                                filter.value = (!filter.value.as_bool().unwrap_or(false)).into();
                            }
                            this.save();
                            cx.notify();
                        })),
                );
            } else {
                row = row.child(
                    div().min_w(px(180.)).flex_1().child(
                        crate::styles::input(&self.playback_filter_inputs[&filter.id], cx)
                            .disabled(!filter.enabled),
                    ),
                );
            }
            panel = panel.child(
                row.child(
                    crate::styles::button(("remove-playback-filter", index), cx)
                        .debug_selector(move || format!("remove-playback-filter-{index}"))
                        .label(self.tr("Remove"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.state
                                .settings
                                .playback_filters
                                .retain(|f| f.id != remove_id);
                            this.playback_filter_inputs.remove(&remove_id);
                            this.save();
                            cx.notify();
                        })),
                ),
            );
        }
        panel.into_any_element()
    }
}
