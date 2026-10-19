use crate::app::Feishin;
use crate::scroll::VerticalScroll;
use feishin_core::{
    model::Item,
    smart::{self, Data, Node},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, StyledExt,
    calendar::{Calendar, CalendarEvent, CalendarState, Date},
    input::InputState,
    menu::{DropdownMenu, PopupMenuItem},
    popover::{Popover, PopoverState},
    select::{SearchableVec, SelectEvent, SelectGroup, SelectItem, SelectState},
};
use serde_json::{Value, json};
use std::collections::HashMap;
use uuid::Uuid;

pub struct Editor {
    pub root: Node,
    initial_root: Node,
    pub fields: Vec<smart::Field>,
    pub values: HashMap<Uuid, Vec<Entity<InputState>>>,
    pub limit: Entity<InputState>,
    other_limit: Entity<InputState>,
    pub percent: bool,
    pub sorts: Vec<(String, bool)>,
    field_choices: HashMap<Uuid, Entity<RuleSelect>>,
    operator_choices: HashMap<Uuid, Entity<RuleSelect>>,
    sort_choices: Vec<Entity<RuleSelect>>,
    choice_language: String,
    custom_fields: Vec<smart::Field>,
    _subscriptions: Vec<Subscription>,
}
#[derive(Clone)]
pub enum Change {
    Reset,
    Clear,
    AddRule,
    AddGroup,
    Remove,
    ToggleGroup,
    Field(String),
    Operator(String),
    LimitMode,
    SortField(usize, String),
    SortDirection(usize),
    AddSort,
    RemoveSort(usize),
}
#[derive(Clone)]
struct Choice {
    label: SharedString,
    value: String,
}
impl SelectItem for Choice {
    type Value = String;
    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &String {
        &self.value
    }
}
type Choices = SearchableVec<SelectGroup<Choice>>;
type RuleSelect = SelectState<Choices>;

fn field_choice(field: &smart::Field) -> Choice {
    Choice {
        label: field.label.clone().into(),
        value: field.value.clone(),
    }
}

fn sort_options(cx: &App) -> Vec<Choice> {
    std::iter::once(Choice {
        label: crate::appearance::text("filter.random", cx).into(),
        value: "random".into(),
    })
    .chain(smart::fields().iter().map(field_choice))
    .collect()
}

fn field_options(custom: &[smart::Field], selected: &str, cx: &App) -> Vec<SelectGroup<Choice>> {
    let mut groups = vec![];
    if !custom.is_empty() {
        groups.push(SelectGroup {
            title: crate::appearance::text("queryBuilder.customTags", cx).into(),
            items: custom.iter().map(field_choice).collect(),
        });
    }
    groups.push(SelectGroup {
        title: if custom.is_empty() {
            "".into()
        } else {
            crate::appearance::text("queryBuilder.standardTags", cx).into()
        },
        items: smart::fields()
            .iter()
            .filter(|field| !custom.iter().any(|custom| custom.value == field.value))
            .map(field_choice)
            .collect(),
    });
    if !selected.is_empty()
        && !groups
            .iter()
            .flat_map(|group| &group.items)
            .any(|item| item.value == selected)
    {
        groups[0].items.push(Choice {
            label: selected.to_owned().into(),
            value: selected.into(),
        });
    }
    groups
}

fn operator_options(
    fields: &[smart::Field],
    field: &str,
    selected: &str,
    language: &str,
) -> Vec<SelectGroup<Choice>> {
    let mut items = smart::operators_for_kind(smart::field_kind(field, fields))
        .iter()
        .map(|value| Choice {
            label: smart::operator_label(language, value).into(),
            value: (*value).into(),
        })
        .collect::<Vec<_>>();
    if !selected.is_empty() && !items.iter().any(|item| item.value == selected) {
        items.push(Choice {
            label: smart::operator_label(language, selected).into(),
            value: selected.into(),
        });
    }
    vec![SelectGroup {
        title: "".into(),
        items,
    }]
}

fn sort_groups(selected: &str, cx: &App) -> Vec<SelectGroup<Choice>> {
    let mut items = sort_options(cx);
    if !selected.is_empty() && !items.iter().any(|item| item.value == selected) {
        items.push(Choice {
            label: selected.to_owned().into(),
            value: selected.into(),
        });
    }
    vec![SelectGroup {
        title: "".into(),
        items,
    }]
}

fn rules(node: &Node, output: &mut Vec<(Uuid, String, String)>) {
    match &node.data {
        Data::Group { children, .. } => children.iter().for_each(|node| rules(node, output)),
        Data::Rule {
            field, operator, ..
        } => output.push((node.id, field.clone(), operator.clone())),
    }
}

fn blank_rule() -> Node {
    Node {
        id: Uuid::new_v4(),
        data: Data::Rule {
            field: String::new(),
            operator: String::new(),
            value: "".into(),
        },
    }
}

fn blank_group(any: bool) -> Node {
    Node {
        id: Uuid::new_v4(),
        data: Data::Group {
            any,
            children: vec![blank_rule()],
            extra: Default::default(),
        },
    }
}

fn remove_incomplete_rules(node: &mut Node) {
    if let Data::Group { children, .. } = &mut node.data {
        children.retain(|node| !matches!(&node.data, Data::Rule { field, operator, .. } if field.is_empty() || operator.is_empty()));
        children.iter_mut().for_each(remove_incomplete_rules);
    }
}

fn select(
    id: Uuid,
    groups: Vec<SelectGroup<Choice>>,
    value: &str,
    change: impl Fn(String) -> Change + 'static,
    subscriptions: &mut Vec<Subscription>,
    window: &mut Window,
    cx: &mut Context<Feishin>,
) -> Entity<RuleSelect> {
    let state = cx.new(|cx| {
        let mut state =
            SelectState::new(SearchableVec::new(groups), None, window, cx).searchable(true);
        state.set_selected_value(&value.to_owned(), window, cx);
        state
    });
    subscriptions.push(cx.subscribe_in(
        &state,
        window,
        move |view, _, event: &SelectEvent<Choices>, window, cx| {
            if let SelectEvent::Confirm(Some(value)) = event {
                view.change_smart(id, change(value.clone()), window, cx);
            }
        },
    ));
    state
}

struct RuleCalendar {
    state: Entity<CalendarState>,
    focus: FocusHandle,
    _subscription: Subscription,
}
impl RuleCalendar {
    fn new(
        input: Entity<InputState>,
        popover: WeakEntity<PopoverState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let date =
            chrono::NaiveDate::parse_from_str(input.read(cx).value().trim(), "%Y-%m-%d").ok();
        let state = cx.new(|cx| {
            let mut state = CalendarState::new(window, cx);
            state.set_date(Date::Single(date), window, cx);
            state
                .year_range((1, 10_000))
                .first_day_of_week(chrono::Weekday::Mon)
                .month_picker()
        });
        let subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &CalendarEvent, window, cx| {
                let CalendarEvent::Selected(date) = event;
                input.update(cx, |input, cx| {
                    input.set_value(
                        date.start()
                            .map(|date| date.to_string())
                            .unwrap_or_default(),
                        window,
                        cx,
                    );
                    input.focus(window, cx);
                });
                popover
                    .update(cx, |state, cx| state.dismiss(window, cx))
                    .ok();
            },
        );
        let focus = cx.focus_handle();
        focus.focus(window);
        Self {
            state,
            focus,
            _subscription: subscription,
        }
    }
}
impl Render for RuleCalendar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.state.update(cx, |state, cx| {
                    state.navigate(&event.keystroke.key, window, cx)
                }) {
                    cx.stop_propagation();
                }
            }))
            .child(Calendar::new(&self.state))
    }
}

impl Editor {
    pub fn refresh_choices(
        &mut self,
        custom: &[smart::Field],
        language: &str,
        window: &mut Window,
        cx: &mut Context<Feishin>,
    ) {
        if self.choice_language == language && self.custom_fields == custom {
            return;
        }
        self.choice_language = language.into();
        self.custom_fields = custom.to_vec();
        self.fields = smart::combined_fields(custom);
        let mut nodes = vec![];
        rules(&self.root, &mut nodes);
        for (id, field, operator) in nodes {
            let fields = SearchableVec::new(field_options(custom, &field, cx));
            self.field_choices[&id].update(cx, |state, cx| state.set_items(fields, window, cx));
            let operators =
                SearchableVec::new(operator_options(&self.fields, &field, &operator, language));
            self.operator_choices[&id]
                .update(cx, |state, cx| state.set_items(operators, window, cx));
        }
        for ((value, _), state) in self.sorts.iter().zip(&self.sort_choices) {
            let choices = SearchableVec::new(sort_groups(value, cx));
            state.update(cx, |state, cx| state.set_items(choices, window, cx));
        }
    }

    pub fn new(
        mut root: Node,
        custom: &[smart::Field],
        window: &mut Window,
        cx: &mut Context<Feishin>,
    ) -> anyhow::Result<Self> {
        let Data::Group { extra, .. } = &root.data else {
            anyhow::bail!("Smart playlists need a group of rules");
        };
        let percent = extra
            .get("limitPercent")
            .is_some_and(|value| !value.is_null());
        let [limit, other_limit] = std::array::from_fn(|index| {
            let value = extra
                .get(if percent != (index == 1) {
                    "limitPercent"
                } else {
                    "limit"
                })
                .filter(|value| !value.is_null())
                .map(ToString::to_string)
                .unwrap_or_default();
            cx.new(|cx| {
                let mut input =
                    crate::appearance::input_state(window, cx).aria_label("common.limit");
                input.set_value(value, window, cx);
                input
            })
        });
        let sort = extra
            .get("sort")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or("+dateAdded");
        let inline_order = sort.contains(',') || sort.starts_with(['+', '-']);
        let sorts: Vec<(String, bool)> = sort
            .split(',')
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                let value = value.trim();
                (
                    value.trim_start_matches(['+', '-']).to_owned(),
                    !value.starts_with('-')
                        && (inline_order
                            || extra.get("order").and_then(Value::as_str) != Some("desc")),
                )
            })
            .collect();
        let mut values = HashMap::new();
        Self::inputs(&mut root, &mut values, window, cx);
        let fields = smart::combined_fields(custom);
        let language = cx
            .try_global::<crate::appearance::Language>()
            .map_or("en", |language| language.0.as_str())
            .to_owned();
        let mut nodes = vec![];
        rules(&root, &mut nodes);
        let mut subscriptions = vec![];
        let mut field_choices = HashMap::new();
        let mut operator_choices = HashMap::new();
        for (id, field, operator) in nodes {
            field_choices.insert(
                id,
                select(
                    id,
                    field_options(custom, &field, cx),
                    &field,
                    Change::Field,
                    &mut subscriptions,
                    window,
                    cx,
                ),
            );
            operator_choices.insert(
                id,
                select(
                    id,
                    operator_options(&fields, &field, &operator, &language),
                    &operator,
                    Change::Operator,
                    &mut subscriptions,
                    window,
                    cx,
                ),
            );
        }
        let sort_choices = sorts
            .iter()
            .enumerate()
            .map(|(index, (value, _))| {
                select(
                    root.id,
                    sort_groups(value, cx),
                    value,
                    move |value| Change::SortField(index, value),
                    &mut subscriptions,
                    window,
                    cx,
                )
            })
            .collect();
        Ok(Self {
            initial_root: root.clone(),
            root,
            fields,
            values,
            limit,
            other_limit,
            percent,
            sorts,
            field_choices,
            operator_choices,
            sort_choices,
            choice_language: language,
            custom_fields: custom.to_vec(),
            _subscriptions: subscriptions,
        })
    }
    fn inputs(
        node: &mut Node,
        values: &mut HashMap<Uuid, Vec<Entity<InputState>>>,
        window: &mut Window,
        cx: &mut Context<Feishin>,
    ) {
        match &mut node.data {
            Data::Group { children, .. } => {
                for child in children {
                    Self::inputs(child, values, window, cx);
                }
            }
            Data::Rule {
                operator, value, ..
            } => {
                *operator = smart::display_operator(operator, value).into();
                let inputs = if matches!(operator.as_str(), "inTheRange" | "inTheRangeDate") {
                    value
                        .as_array()
                        .cloned()
                        .unwrap_or_else(|| vec![value.clone(), "".into()])
                } else {
                    vec![value.clone()]
                };
                values.insert(
                    node.id,
                    inputs
                        .into_iter()
                        .map(|value| {
                            cx.new(|cx| {
                                let mut input =
                                    crate::appearance::input_state(window, cx).placeholder("Value");
                                input.set_value(
                                    if value.is_null() && operator == "inTheRangeDate" {
                                        String::new()
                                    } else {
                                        value
                                            .as_str()
                                            .map(str::to_owned)
                                            .unwrap_or_else(|| value.to_string())
                                    },
                                    window,
                                    cx,
                                );
                                input
                            })
                        })
                        .collect(),
                );
            }
        }
    }
    pub fn snapshot(&self, cx: &App) -> anyhow::Result<Node> {
        let mut root = self.root.clone();
        remove_incomplete_rules(&mut root);
        for (id, inputs) in &self.values {
            let values = inputs
                .iter()
                .map(|input| input.read(cx).value().to_string())
                .collect::<Vec<_>>();
            if let Some(node) = root.find_mut(*id) {
                node.apply_text_with_fields(&values, &self.fields)?;
            }
        }
        if let Data::Group { extra, .. } = &mut root.data {
            let limit = self.limit.read(cx).value();
            extra.remove("limit");
            extra.remove("limitPercent");
            if !limit.trim().is_empty() {
                let value = limit
                    .parse::<f64>()
                    .map_err(|_| anyhow::anyhow!("Enter a valid playlist limit"))?;
                anyhow::ensure!(
                    value.is_finite() && (!self.percent || (0.0..=100.0).contains(&value)),
                    "Invalid playlist limit"
                );
                extra.insert(
                    if self.percent {
                        "limitPercent"
                    } else {
                        "limit"
                    }
                    .into(),
                    serde_json::from_str(&value.to_string())?,
                );
            }
            let sort = self
                .sorts
                .iter()
                .filter(|(field, _)| !field.is_empty())
                .map(|(field, ascending)| format!("{}{field}", if *ascending { "+" } else { "-" }))
                .collect::<Vec<_>>()
                .join(",");
            extra.insert(
                "sort".into(),
                if sort.is_empty() {
                    "+dateAdded".into()
                } else {
                    sort.into()
                },
            );
        }
        Node::parse(&root.json())?;
        Ok(root)
    }
    fn draft(&self, cx: &App) -> Node {
        let mut root = self.root.clone();
        for (id, inputs) in &self.values {
            if let Some(node) = root.find_mut(*id) {
                let values = inputs
                    .iter()
                    .map(|input| input.read(cx).value().to_string())
                    .collect::<Vec<_>>();
                // Unfinished text stays in its existing input; structural edits remain available.
                let _ = node.apply_text_with_fields(&values, &self.fields);
            }
        }
        root
    }
    pub fn render(
        &self,
        playlists: &[Item],
        current_playlist: Option<&str>,
        cx: &mut Context<Feishin>,
    ) -> AnyElement {
        let root_id = self.root.id;
        div()
            .v_flex()
            .gap_3()
            .child(self.node_view(&self.root, 0, playlists, current_playlist, cx))
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .child(
                        crate::styles::button("smart-limit-mode", cx)
                            .label(format!(
                                "{} {}",
                                crate::appearance::text("common.limit", cx),
                                if self.percent { "%" } else { "#" }
                            ))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.change_smart(root_id, Change::LimitMode, window, cx)
                            })),
                    )
                    .child(crate::styles::input(&self.limit, cx)),
            )
            .child(
                div()
                    .text_sm()
                    .child(crate::appearance::text("common.sort", cx)),
            )
            .children(
                self.sorts
                    .iter()
                    .enumerate()
                    .map(|(index, (_, ascending))| {
                        div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .gap_2()
                            .child(
                                div()
                                    .debug_selector(move || format!("smart-sort-{index}"))
                                    .child(
                                        crate::styles::select(&self.sort_choices[index])
                                            .w(px(200.))
                                            .search_placeholder(crate::appearance::text(
                                                "common.search",
                                                cx,
                                            )),
                                    ),
                            )
                            .child(
                                crate::styles::button(("smart-sort-direction", index), cx)
                                    .label(crate::appearance::text(
                                        if *ascending {
                                            "common.ascending"
                                        } else {
                                            "common.descending"
                                        },
                                        cx,
                                    ))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.change_smart(
                                            root_id,
                                            Change::SortDirection(index),
                                            window,
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                crate::styles::button(("smart-sort-remove", index), cx)
                                    .disabled(self.sorts.len() <= 1)
                                    .label(crate::appearance::text("Remove", cx))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.change_smart(
                                            root_id,
                                            Change::RemoveSort(index),
                                            window,
                                            cx,
                                        )
                                    })),
                            )
                    }),
            )
            .child(
                crate::styles::button("smart-sort-add", cx)
                    .label(crate::appearance::text("Add sort field", cx))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.change_smart(root_id, Change::AddSort, window, cx)
                    })),
            )
            .into_any_element()
    }
    fn node_view(
        &self,
        node: &Node,
        depth: usize,
        playlists: &[Item],
        current_playlist: Option<&str>,
        cx: &mut Context<Feishin>,
    ) -> AnyElement {
        let id = node.id;
        let element_id = id.as_u128() as usize;
        match &node.data {
            Data::Group { any, children, .. } => {
                div()
                    .id(("smart-group", element_id))
                    .v_flex()
                    .gap_2()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .h_flex()
                            .flex_wrap()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .gap_2()
                            .child(
                                crate::styles::button(("smart-group-type", element_id), cx)
                                    .label(crate::appearance::text(
                                        if *any {
                                            "form.queryEditor.input_optionMatchAny"
                                        } else {
                                            "form.queryEditor.input_optionMatchAll"
                                        },
                                        cx,
                                    ))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.change_smart(id, Change::ToggleGroup, window, cx)
                                    })),
                            )
                            .child(
                                crate::styles::button(("smart-add-rule", element_id), cx)
                                    .label(crate::appearance::text("Add rule", cx))
                                    .disabled(depth >= 8)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.change_smart(id, Change::AddRule, window, cx)
                                    })),
                            )
                            .child(
                                crate::styles::button(("smart-add-group", element_id), cx)
                                    .label(crate::appearance::text("Add group", cx))
                                    .disabled(depth >= 8)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.change_smart(id, Change::AddGroup, window, cx)
                                    })),
                            )
                            .when(depth > 0, |div| {
                                div.child(
                                    crate::styles::button(("smart-remove-group", element_id), cx)
                                        .label(crate::appearance::text("Remove group", cx))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.change_smart(id, Change::Remove, window, cx)
                                        })),
                                )
                            })
                            .when(depth == 0, |div| {
                                div.child(
                                    crate::styles::button("smart-reset-rules", cx)
                                        .debug_selector(|| "smart-reset-rules".into())
                                        .label(crate::appearance::text(
                                            "form.queryEditor.resetToDefault",
                                            cx,
                                        ))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.change_smart(id, Change::Reset, window, cx)
                                        })),
                                )
                                .child(
                                    crate::styles::button("smart-clear-rules", cx)
                                        .debug_selector(|| "smart-clear-rules".into())
                                        .label(crate::appearance::text(
                                            "form.queryEditor.clearFilters",
                                            cx,
                                        ))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.change_smart(id, Change::Clear, window, cx)
                                        })),
                                )
                            }),
                    )
                    .children(children.iter().map(|child| {
                        self.node_view(child, depth + 1, playlists, current_playlist, cx)
                    }))
                    .into_any_element()
            }
            Data::Rule {
                field, operator, ..
            } => {
                let boolean = smart::field_kind(field, &self.fields) == "boolean"
                    || matches!(operator.as_str(), "isMissing" | "isPresent");
                div()
                    .id(("smart-rule", element_id))
                    .debug_selector(move || format!("smart-rule-{element_id}"))
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .flex_wrap()
                    .gap_2()
                    .items_center()
                    .child(
                        div().debug_selector(move || format!("smart-field-{element_id}"))
                            .child(crate::styles::select(&self.field_choices[&id]).small().w(px(170.)).search_placeholder(crate::appearance::text("common.search", cx))),
                    )
                    .child(
                        div().debug_selector(move || format!("smart-operator-{element_id}"))
                            .child(crate::styles::select(&self.operator_choices[&id]).small().w(px(170.)).disabled(field.is_empty()).search_placeholder(crate::appearance::text("common.search", cx))),
                    )
                    .children(self.values.get(&id).into_iter().flatten().enumerate().map(
                        |(index, input)| {
                            if boolean {
                                let value = input.read(cx).value().as_str() == "true";
                                let input = input.clone();
                                crate::styles::button(("smart-boolean", element_id), cx)
                                    .small()
                                    .label(if value { "True" } else { "False" })
                                    .on_click(cx.listener(move |_, _, window, cx| {
                                        input.update(cx, |input, cx| {
                                            input.set_value(
                                                if value { "false" } else { "true" },
                                                window,
                                                cx,
                                            )
                                        });
                                        cx.notify();
                                    }))
                                    .into_any_element()
                            } else if smart::field_kind(field, &self.fields) == "playlist" {
                                let input = input.clone();
                                let value = input.read(cx).value();
                                let label = playlists
                                    .iter()
                                    .find(|item| item.id == value.as_str())
                                    .map_or("Choose playlist".into(), |item| item.name.clone());
                                let view = cx.weak_entity();
                                let choices = playlists.iter().filter(|playlist| Some(playlist.id.as_str()) != current_playlist).cloned().collect::<Vec<_>>();
                                crate::styles::button(("smart-playlist-value", element_id), cx)
                                    .debug_selector(move || format!("smart-playlist-value-{element_id}"))
                                    .small()
                                    .label(label)
                                    .dropdown_menu(move |mut menu, _, _| {
                                        for playlist in &choices {
                                            let input = input.clone();
                                            let id = playlist.id.clone();
                                            let view = view.clone();
                                            menu = menu.item(
                                                PopupMenuItem::new(playlist.name.clone()).on_click(
                                                    move |_, window, cx| {
                                                        input.update(cx, |input, cx| {
                                                            input.set_value(id.clone(), window, cx)
                                                        });
                                                        view.update(cx, |_, cx| cx.notify()).ok();
                                                    },
                                                ),
                                            );
                                        }
                                        menu
                                    })
                                    .into_any_element()
                            } else {
                                let calendar = matches!(operator.as_str(), "beforeDate" | "afterDate" | "inTheRangeDate");
                                div()
                                    .w(px(190.0))
                                    .when(calendar, |div| div.w_auto().flex_none())
                                    .when(matches!(operator.as_str(), "inTheRange" | "inTheRangeDate"), |div| {
                                        div.child(if index == 0 { "From" } else { "To" })
                                    })
                                    .child(div().h_flex().gap_1()
                                        .child(crate::styles::input(input, cx).disabled(field.is_empty()).flex_1().min_w(px(0.)).cleanable(calendar)
                                            .when(calendar, |input| input.w(rems(10.625)).min_w(rems(10.625)).flex_none()))
                                        .when(calendar, |row| {
                                            let input = input.clone();
                                            row.child(Popover::new(SharedString::from(format!("smart-calendar-{element_id}-{index}")))
                                                .trigger(crate::styles::button(SharedString::from(format!("smart-calendar-button-{element_id}-{index}")), cx)
                                                    .small().label(crate::appearance::text("filter.date", cx))
                                                    .debug_selector(move || format!("smart-calendar-{element_id}-{index}")))
                                                .content(move |_, window, cx| {
                                                    let popover = cx.weak_entity();
                                                    window.use_keyed_state("smart-date-calendar", cx, |window, cx| RuleCalendar::new(input.clone(), popover, window, cx))
                                                }))
                                        }))
                                    .into_any_element()
                            }
                        },
                    ))
                    .child(
                        crate::styles::button(("smart-remove-rule", element_id), cx)
                            .small()
                            .label(crate::appearance::text("Remove", cx))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.change_smart(id, Change::Remove, window, cx)
                            })),
                    )
                    .into_any_element()
            }
        }
    }
}
impl Feishin {
    pub fn open_query_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.query_field_inputs.clear();
        self.query_field_types.clear();
        for field in self.state.settings.query_fields.clone() {
            self.add_query_field(field, window, cx);
        }
        self.navigate(crate::app::Route::QueryFields, cx);
    }
    fn add_query_field(
        &mut self,
        field: smart::Field,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.query_field_types.push(field.kind);
        self.query_field_inputs.push(
            [
                (field.label, "setting.queryBuilderCustomFields_inputLabel"),
                (field.value, "setting.queryBuilderCustomFields_inputTag"),
            ]
            .map(|(value, label)| {
                cx.new(|cx| {
                    crate::appearance::input_state(window, cx)
                        .aria_label(label)
                        .placeholder(label)
                        .default_value(value)
                })
            }),
        );
        cx.notify();
    }
    pub fn query_fields_view(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("query-fields-settings")
            .v_flex()
            .gap_3()
            .size_full()
            .native_vertical_scroll()
            .child(
                crate::styles::button("query-fields-back", cx)
                    .label(self.tr("Back"))
                    .on_click(
                        cx.listener(|this, _, _, cx| {
                            this.navigate(crate::app::Route::Settings, cx)
                        }),
                    ),
            )
            .child(self.tr("Custom query builder fields"))
            .children(
                self.query_field_inputs
                    .iter()
                    .enumerate()
                    .map(|(index, inputs)| {
                        let view = cx.weak_entity();
                        div()
                            .h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .child(self.tr("setting.queryBuilderCustomFields_inputLabel"))
                                    .child(crate::styles::input(&inputs[0], cx)),
                            )
                            .child(
                                crate::styles::button(("query-field-type", index), cx)
                                    .label(self.query_field_types[index].clone())
                                    .dropdown_menu(move |mut menu, _, _| {
                                        for (kind, label) in [
                                            ("boolean", "Boolean"),
                                            ("date", "Date"),
                                            ("dateRange", "Date range"),
                                            ("number", "Number"),
                                            ("playlist", "Playlist"),
                                            ("string", "String"),
                                        ] {
                                            let view = view.clone();
                                            menu = menu.item(PopupMenuItem::new(label).on_click(
                                                move |_, _, cx| {
                                                    view.update(cx, |this, cx| {
                                                        if let Some(old) =
                                                            this.query_field_types.get_mut(index)
                                                        {
                                                            *old = kind.into();
                                                        }
                                                        cx.notify();
                                                    })
                                                    .ok();
                                                },
                                            ));
                                        }
                                        menu
                                    }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(self.tr("setting.queryBuilderCustomFields_inputTag"))
                                    .child(crate::styles::input(&inputs[1], cx)),
                            )
                            .child(
                                crate::styles::button(("query-field-remove", index), cx)
                                    .label(self.tr("Remove"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if index < this.query_field_inputs.len() {
                                            this.query_field_inputs.remove(index);
                                            this.query_field_types.remove(index);
                                        }
                                        cx.notify();
                                    })),
                            )
                    }),
            )
            .child(
                crate::styles::button("query-field-add", cx)
                    .debug_selector(|| "query-field-add".into())
                    .label(self.tr("Add"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this.query_field_inputs.len() < 256 {
                            this.add_query_field(
                                smart::Field {
                                    label: String::new(),
                                    value: String::new(),
                                    kind: "string".into(),
                                },
                                window,
                                cx,
                            );
                        }
                    })),
            )
            .child(
                crate::styles::button("query-fields-apply", cx)
                    .debug_selector(|| "query-fields-apply".into())
                    .label(self.tr("common.save"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let fields = this
                            .query_field_inputs
                            .iter()
                            .zip(&this.query_field_types)
                            .map(|(inputs, kind)| smart::Field {
                                label: inputs[0].read(cx).value().trim().into(),
                                value: inputs[1].read(cx).value().trim().into(),
                                kind: kind.clone(),
                            })
                            .collect::<Vec<_>>();
                        match smart::validate_fields(&fields) {
                            Ok(()) => {
                                this.state.settings.query_fields = fields;
                                this.save();
                            }
                            Err(error) => this.message = error.to_string(),
                        }
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
    pub fn enable_smart(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.smart_editor.is_some() {
            return;
        }
        self.load_playlist_options(cx);
        let raw = self.form_rules.read(cx).value();
        let result = if raw.trim().is_empty() {
            Ok(blank_group(false))
        } else {
            serde_json::from_str(&raw)
                .map_err(anyhow::Error::from)
                .and_then(|value| Node::parse(&value))
        };
        match result
            .and_then(|node| Editor::new(node, &self.state.settings.query_fields, window, cx))
        {
            Ok(editor) => self.smart_editor = Some(editor),
            Err(error) => self.message = error.to_string(),
        }
        cx.notify();
    }
    pub fn raw_smart(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &self.smart_editor {
            match editor.snapshot(cx) {
                Ok(node) => {
                    self.form_rules.update(cx, |input, cx| {
                        input.set_value(
                            serde_json::to_string_pretty(&node.json()).unwrap(),
                            window,
                            cx,
                        )
                    });
                    self.smart_editor = None;
                }
                Err(error) => self.message = error.to_string(),
            }
        }
        cx.notify();
    }
    pub fn change_smart(
        &mut self,
        id: Uuid,
        change: Change,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(mut editor) = self.smart_editor.take() else {
            return;
        };
        if matches!(change, Change::LimitMode) {
            editor.percent = !editor.percent;
            std::mem::swap(&mut editor.limit, &mut editor.other_limit);
            self.smart_editor = Some(editor);
            cx.notify();
            return;
        }
        let discard_drafts = matches!(change, Change::Reset | Change::Clear);
        let reset_value = matches!(&change, Change::Field(_))
            || matches!(&change, Change::Operator(operator) if matches!(operator.as_str(), "isMissing" | "isPresent"));
        let mut root = match change {
            Change::Reset => editor.initial_root.clone(),
            Change::Clear => blank_group(false),
            _ => editor.draft(cx),
        };
        if discard_drafts
            && let (
                Data::Group { extra, .. },
                Data::Group {
                    extra: previous, ..
                },
            ) = (&mut root.data, &editor.root.data)
        {
            *extra = previous.clone();
        }
        match change {
            Change::Reset | Change::Clear => {}
            Change::Remove => root.remove(id),
            Change::LimitMode => {
                unreachable!("Limit mode changes preserve both input drafts above")
            }
            Change::SortField(index, field) => {
                if let Some(sort) = editor.sorts.get_mut(index) {
                    sort.0 = field;
                }
            }
            Change::SortDirection(index) => {
                if let Some(sort) = editor.sorts.get_mut(index) {
                    sort.1 = !sort.1;
                }
            }
            Change::AddSort => editor.sorts.push((String::new(), true)),
            Change::RemoveSort(index) => {
                if editor.sorts.len() > 1 && index < editor.sorts.len() {
                    editor.sorts.remove(index);
                }
            }
            change => {
                if let Some(node) = root.find_mut(id) {
                    match (&mut node.data, change) {
                        (Data::Group { children, .. }, Change::AddRule) => {
                            children.push(blank_rule())
                        }
                        (Data::Group { children, .. }, Change::AddGroup) => {
                            children.push(blank_group(true))
                        }
                        (Data::Group { any, .. }, Change::ToggleGroup) => *any = !*any,
                        (
                            Data::Rule {
                                field,
                                operator,
                                value,
                            },
                            Change::Field(new),
                        ) => {
                            *field = new;
                            operator.clear();
                            *value = if smart::field_kind(field, &self.state.settings.query_fields)
                                == "boolean"
                            {
                                false.into()
                            } else {
                                "".into()
                            };
                        }
                        (
                            Data::Rule {
                                operator, value, ..
                            },
                            Change::Operator(new),
                        ) => {
                            *operator = new;
                            if matches!(operator.as_str(), "inTheRange" | "inTheRangeDate")
                                && !value.is_array()
                            {
                                *value = json!([value, value]);
                            } else if matches!(operator.as_str(), "isMissing" | "isPresent") {
                                *value = true.into();
                            } else if !matches!(operator.as_str(), "inTheRange" | "inTheRangeDate")
                                && value.is_array()
                            {
                                *value = value[0].clone();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        if let Data::Group { extra, .. } = &mut root.data {
            extra.insert(
                "sort".into(),
                editor
                    .sorts
                    .iter()
                    .map(|(field, ascending)| {
                        format!("{}{field}", if *ascending { "+" } else { "-" })
                    })
                    .collect::<Vec<_>>()
                    .join(",")
                    .into(),
            );
        }
        let result = Node::parse(&root.json())
            .and_then(|_| Editor::new(root, &self.state.settings.query_fields, window, cx));
        match result {
            Ok(mut updated) => {
                updated.initial_root = editor.initial_root;
                updated.percent = editor.percent;
                updated.limit = editor.limit;
                updated.other_limit = editor.other_limit;
                if !discard_drafts {
                    for (node_id, inputs) in &mut updated.values {
                        if (!reset_value || *node_id != id)
                            && let Some(previous) = editor.values.get(node_id)
                            && inputs.len() == previous.len()
                        {
                            *inputs = previous.clone();
                        }
                    }
                }
                self.smart_editor = Some(updated);
            }
            Err(error) => {
                self.message = error.to_string();
                self.smart_editor = Some(editor);
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::sort_options;
    use gpui::TestAppContext;
    use serde_json::Value;

    #[gpui::test]
    fn sort_choices_match_all_original_fields(cx: &mut TestAppContext) {
        let reference: Value = serde_json::from_str(include_str!(
            "../../feishin-core/assets/smart-editor-reference.json"
        ))
        .unwrap();
        let choices = cx.update(|cx| sort_options(cx));
        let expected = reference["sorts"].as_array().unwrap();
        assert_eq!(choices.len(), expected.len());
        for (choice, expected) in choices.iter().zip(expected) {
            assert_eq!(choice.value, expected["value"]);
            assert_eq!(choice.label.as_ref(), expected["label"].as_str().unwrap());
        }
    }

    #[gpui::test]
    fn calendar_grids_and_month_end_selection_match_original(cx: &mut TestAppContext) {
        use super::*;
        use chrono::Datelike;
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc};
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let reference: Value =
            serde_json::from_str(include_str!("../assets/calendar-reference.json")).unwrap();
        for (index, case) in reference.as_array().unwrap().iter().enumerate() {
            let captured = Rc::new(RefCell::new(None));
            let (_, cx) = cx.add_window_view(|window, cx| {
                window.activate_window();
                window.activate_accessibility_for_test();
                let input = cx.new(|cx| {
                    let mut input = InputState::new(window, cx);
                    input.set_value(case["date"].as_str().unwrap().to_owned(), window, cx);
                    input
                });
                let popover = cx.new(|cx| PopoverState::new(false, cx));
                let calendar = cx.new(|cx| {
                    super::RuleCalendar::new(input.clone(), popover.downgrade(), window, cx)
                });
                captured.replace(Some((calendar.clone(), input, popover)));
                Root::new(calendar, window, cx)
            });
            let (calendar, input, _popover) = captured.borrow_mut().take().unwrap();
            cx.simulate_resize(size(px(800.), px(700.)));
            cx.run_until_parked();
            let click = |selector: &str, cx: &mut VisualTestContext| {
                let bounds = cx
                    .debug_bounds(selector)
                    .unwrap_or_else(|| panic!("Missing {selector}"));
                cx.simulate_click(bounds.center(), Modifiers::none());
                cx.run_until_parked();
            };
            for month in 1..=case["months"].as_array().unwrap().len() {
                assert!(
                    cx.debug_bounds(&format!("calendar-month-{month}"))
                        .is_some()
                );
            }
            let date =
                chrono::NaiveDate::parse_from_str(case["date"].as_str().unwrap(), "%Y-%m-%d")
                    .unwrap();
            if index == 0 {
                click("calendar-prev", cx);
                click("calendar-month-2", cx);
                assert!(cx.debug_bounds("calendar-day-2020-02-29").is_some());
                click("calendar-month-view", cx);
                click("calendar-next", cx);
            }
            click(&format!("calendar-month-{}", date.month()), cx);
            let weeks = case["weeks"].as_array().unwrap();
            let expected: Vec<_> = weeks
                .iter()
                .flat_map(|week| week.as_array().unwrap())
                .map(|date| date.as_str().unwrap())
                .collect();
            cx.update(|window, _| {
                let actual: Vec<_> = window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .filter(|(_, node)| node.role() == Role::Button)
                    .filter_map(|(_, node)| node.label())
                    .filter(|label| chrono::NaiveDate::parse_from_str(label, "%Y-%m-%d").is_ok())
                    .collect();
                assert_eq!(actual, expected, "{}", case["date"]);
            });
            let first = cx
                .debug_bounds(&format!("calendar-day-{}", expected[0]))
                .unwrap();
            for (row, week) in weeks.iter().enumerate() {
                for (column, date) in week.as_array().unwrap().iter().enumerate() {
                    let bounds = cx
                        .debug_bounds(&format!("calendar-day-{}", date.as_str().unwrap()))
                        .unwrap();
                    assert_eq!(
                        bounds.origin.x,
                        first.origin.x + (first.size.width + px(2.)) * column as f32
                    );
                    assert_eq!(
                        bounds.origin.y,
                        first.origin.y + (first.size.height + px(2.)) * row as f32
                    );
                }
            }
            click(
                &format!("calendar-day-{}", case["saved"].as_str().unwrap()),
                cx,
            );
            assert_eq!(
                input.read_with(cx, |input, _| input.value().to_string()),
                case["saved"]
            );
            if index == 0 {
                cx.update(|window, cx| calendar.read(cx).focus.focus(window));
                click("calendar-year-view", cx);
                cx.simulate_keystrokes("right enter right enter");
                cx.run_until_parked();
                assert_eq!(
                    input.read_with(cx, |input, _| input.value().to_string()),
                    case["saved"],
                    "Choosing a year/month must not commit a day"
                );
                assert!(cx.debug_bounds("calendar-day-2022-06-30").is_some());
                cx.simulate_keystrokes("right enter");
                cx.run_until_parked();
                assert_eq!(
                    input.read_with(cx, |input, _| input.value().to_string()),
                    "2022-07-01"
                );
                click("calendar-year-view", cx);
                click("calendar-year-2023", cx);
                click("calendar-month-2", cx);
                click("calendar-day-2023-02-28", cx);
                assert_eq!(
                    input.read_with(cx, |input, _| input.value().to_string()),
                    "2023-02-28"
                );
            }
        }
    }
}
