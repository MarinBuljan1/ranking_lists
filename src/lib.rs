pub mod data;
pub mod matchflow;
pub mod ranking;
pub mod storage;

use data::{fetch_available_lists, load_list, ListInfo, LoadedList};
use gloo_timers::callback::Timeout;
use matchflow::{random_matchup, Matchup};
use ranking::BradleyTerry;
use std::ops::Deref;
use storage::{
    align_list_state, load_list_state, load_state as load_storage_state,
    save_state as persist_state, upsert_list_state, StoredListState,
};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, Element};
use yew::prelude::*;

const SWIPE_THRESHOLD: f64 = 80.0;
const FLASH_CLEAR_DELAY_MS: u32 = 1000;
const MATCH_RESOLVE_DELAY_MS: u32 = 260;
const ENTER_ANIMATION_DURATION_MS: u32 = 1200;
const ENTER_ANIMATION_BUFFER_MS: u32 = 80;

#[derive(Clone, PartialEq)]
struct DragState {
    pointer_id: i32,
    start_x: f64,
    current_x: f64,
}

struct SwipeBackground {
    start_rgb: (u8, u8, u8),
    end_rgb: (u8, u8, u8),
    start_alpha: f64,
    end_alpha: f64,
}

impl SwipeBackground {
    fn start_color_value(&self) -> String {
        let (r, g, b) = self.start_rgb;
        format!("{r} {g} {b}")
    }

    fn end_color_value(&self) -> String {
        let (r, g, b) = self.end_rgb;
        format!("{r} {g} {b}")
    }

    fn start_alpha_value(&self) -> String {
        format!("{:.4}", self.start_alpha.clamp(0.0, 1.0))
    }

    fn end_alpha_value(&self) -> String {
        format!("{:.4}", self.end_alpha.clamp(0.0, 1.0))
    }
}

#[derive(Clone)]
struct PointerCallbacks {
    down: Callback<web_sys::PointerEvent>,
    move_cb: Callback<web_sys::PointerEvent>,
    up: Callback<web_sys::PointerEvent>,
    cancel: Callback<web_sys::PointerEvent>,
}

impl PointerCallbacks {
    fn noop() -> Self {
        let noop = Callback::from(|_: web_sys::PointerEvent| {});
        Self {
            down: noop.clone(),
            move_cb: noop.clone(),
            up: noop.clone(),
            cancel: noop,
        }
    }
}

#[derive(PartialEq, Clone)]
enum FetchStatus {
    Idle,
    Loading,
    Error(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WinnerSide {
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq)]
enum CardTransition {
    Idle,
    Exiting { side: WinnerSide, offset: f64 },
    Entering { side: WinnerSide },
}

#[function_component(App)]
fn app() -> Html {
    let list_status = use_state(|| FetchStatus::Loading);
    let lists = use_state(|| None::<Vec<ListInfo>>);
    let persisted_state = use_state(load_storage_state);

    let initial_selection = (*persisted_state).selected_list.clone();
    let selected_list = use_state(move || initial_selection);

    let items_status = use_state(|| FetchStatus::Idle);
    let loaded_list = use_state(|| None::<LoadedList>);
    let ranking_state = use_state(|| None::<BradleyTerry>);
    let current_match = use_state(|| None::<Matchup>);
    let list_state = use_state(|| None::<StoredListState>);
    let drag_state = use_state(|| None::<DragState>);
    let card_transition = use_state(|| CardTransition::Idle);
    let flash_side = use_state(|| None::<WinnerSide>);
    let menu_open = use_state(|| false);
    let lists_expanded = use_state(|| false);
    let show_reset_confirm = use_state(|| false);

    {
        let list_status = list_status.clone();
        let lists = lists.clone();
        let selected_list = selected_list.clone();
        let persisted_state = persisted_state.clone();

        use_effect_with_deps(
            move |_| {
                list_status.set(FetchStatus::Loading);

                let list_status = list_status.clone();
                let lists = lists.clone();
                let selected_list = selected_list.clone();
                let previously_selected = (*selected_list).clone();
                let persisted_state = persisted_state.clone();

                spawn_local(async move {
                    match fetch_available_lists().await {
                        Ok(fetched) => {
                            let previous = previously_selected
                                .or_else(|| persisted_state.selected_list.clone());
                            let default_selection = resolve_selection(&fetched, previous);
                            lists.set(Some(fetched));
                            if let Some(selection) = default_selection {
                                selected_list.set(Some(selection));
                            }
                            list_status.set(FetchStatus::Idle);
                        }
                        Err(err) => {
                            list_status.set(FetchStatus::Error(err.to_string()));
                            lists.set(None);
                            selected_list.set(None);
                        }
                    }
                });

                || ()
            },
            (),
        );
    }

    {
        let selected_list = selected_list.clone();
        let items_status = items_status.clone();
        let loaded_list = loaded_list.clone();
        let ranking_state = ranking_state.clone();
        let current_match = current_match.clone();
        let list_state_handle = list_state.clone();
        let persisted_state_handle = persisted_state.clone();
        let drag_state_handle = drag_state.clone();

        use_effect_with_deps(
            move |selected: &Option<String>| {
                match selected {
                    Some(id) => {
                        items_status.set(FetchStatus::Loading);
                        loaded_list.set(None);
                        ranking_state.set(None);
                        current_match.set(None);
                        list_state_handle.set(None);
                        drag_state_handle.set(None);

                        let id = id.clone();
                        let items_status = items_status.clone();
                        let loaded_list = loaded_list.clone();
                        let ranking_state = ranking_state.clone();
                        let current_match = current_match.clone();
                        let list_state_handle = list_state_handle.clone();
                        let persisted_state_handle = persisted_state_handle.clone();
                        let persisted_snapshot = (*persisted_state_handle).clone();
                        let drag_state_handle = drag_state_handle.clone();

                        spawn_local(async move {
                            match load_list(&id).await {
                                Ok(list) => {
                                    let item_ids: Vec<String> =
                                        list.items.iter().map(|item| item.id.clone()).collect();

                                    let existing =
                                        load_list_state(&persisted_snapshot, &id).cloned();
                                    let mut stored_state = align_list_state(existing, &item_ids);

                                    let mut ranking = BradleyTerry::from_abilities(
                                        stored_state.abilities.clone(),
                                    );
                                    ranking.ensure_len(item_ids.len());
                                    ranking.run_iterations(&stored_state.win_matrix, 8);
                                    stored_state.abilities = ranking.to_vec();

                                    let mut updated_app_state = persisted_snapshot.clone();
                                    upsert_list_state(
                                        &mut updated_app_state,
                                        &id,
                                        stored_state.clone(),
                                    );
                                    persist_state(&updated_app_state);
                                    persisted_state_handle.set(updated_app_state);

                                    let next_match = random_matchup(item_ids.len(), None);

                                    list_state_handle.set(Some(stored_state));
                                    ranking_state.set(Some(ranking));
                                    current_match.set(next_match);
                                    loaded_list.set(Some(list));
                                    drag_state_handle.set(None);
                                    items_status.set(FetchStatus::Idle);
                                }
                                Err(err) => {
                                    items_status.set(FetchStatus::Error(err.to_string()));
                                    loaded_list.set(None);
                                    ranking_state.set(None);
                                    current_match.set(None);
                                    list_state_handle.set(None);
                                    drag_state_handle.set(None);
                                }
                            }
                        });
                    }
                    None => {
                        loaded_list.set(None);
                        ranking_state.set(None);
                        current_match.set(None);
                        list_state_handle.set(None);
                        drag_state_handle.set(None);
                        items_status.set(FetchStatus::Idle);
                    }
                };

                || ()
            },
            (*selected_list).clone(),
        );
    }

    {
        let selected_list = selected_list.clone();
        let persisted_state = persisted_state.clone();

        use_effect_with_deps(
            move |current: &Option<String>| {
                let mut next_state = (*persisted_state).clone();
                if next_state.selected_list != *current {
                    next_state.selected_list = current.clone();
                    persist_state(&next_state);
                    persisted_state.set(next_state);
                }
                || ()
            },
            (*selected_list).clone(),
        );
    }

    let on_match_result = {
        let ranking_state = ranking_state.clone();
        let current_match = current_match.clone();
        let loaded_list = loaded_list.clone();
        let list_state_handle = list_state.clone();
        let selected_list = selected_list.clone();
        let persisted_state_handle = persisted_state.clone();
        let card_transition_handle = card_transition.clone();
        let drag_state_handle = drag_state.clone();

        Callback::from(move |side: WinnerSide| {
            let Some(list_id) = (*selected_list).clone() else {
                return;
            };

            let Some(prev_match) = (*current_match).clone() else {
                return;
            };

            let Some(mut ranking) = (*ranking_state).clone() else {
                return;
            };

            let Some(list) = (&*loaded_list).as_ref() else {
                return;
            };

            let Some(mut stored_state) = (*list_state_handle).clone() else {
                return;
            };

            let winner_index;
            let loser_index;

            match side {
                WinnerSide::Left => {
                    winner_index = prev_match.left_index;
                    loser_index = prev_match.right_index;
                }
                WinnerSide::Right => {
                    winner_index = prev_match.right_index;
                    loser_index = prev_match.left_index;
                }
            }

            if winner_index >= stored_state.win_matrix.len()
                || loser_index >= stored_state.win_matrix.len()
            {
                return;
            }

            stored_state.win_matrix[winner_index][loser_index] =
                stored_state.win_matrix[winner_index][loser_index].saturating_add(1);

            ranking.ensure_len(stored_state.win_matrix.len());
            ranking.run_iterations(&stored_state.win_matrix, 6);
            stored_state.abilities = ranking.to_vec();

            list_state_handle.set(Some(stored_state.clone()));
            ranking_state.set(Some(ranking.clone()));

            let mut updated_app_state = (*persisted_state_handle).clone();
            upsert_list_state(&mut updated_app_state, &list_id, stored_state);
            persist_state(&updated_app_state);
            persisted_state_handle.set(updated_app_state);

            let next_match = random_matchup(list.items.len(), Some(&prev_match));
            current_match.set(next_match.clone());
            drag_state_handle.set(None);

            if next_match.is_some() {
                card_transition_handle.set(CardTransition::Entering { side });
                let card_transition_for_idle = card_transition_handle.clone();
                Timeout::new(
                    ENTER_ANIMATION_DURATION_MS + ENTER_ANIMATION_BUFFER_MS,
                    move || {
                        card_transition_for_idle.set(CardTransition::Idle);
                    },
                )
                .forget();
            } else {
                card_transition_handle.set(CardTransition::Idle);
            }
        })
    };

    let on_reset = {
        let selected_list = selected_list.clone();
        let persisted_state_handle = persisted_state.clone();
        let list_state_handle = list_state.clone();
        let ranking_state = ranking_state.clone();
        let current_match = current_match.clone();
        let loaded_list = loaded_list.clone();
        let items_status = items_status.clone();
        let drag_state_handle = drag_state.clone();
        let show_reset_confirm_handle = show_reset_confirm.clone();

        Callback::from(move |_| {
            let Some(list_id) = (*selected_list).clone() else {
                return;
            };

            let Some(list) = (&*loaded_list).as_ref() else {
                return;
            };

            let item_ids: Vec<String> = list.items.iter().map(|item| item.id.clone()).collect();

            let mut new_state = StoredListState::new(&item_ids);

            let mut ranking = BradleyTerry::from_abilities(new_state.abilities.clone());
            ranking.ensure_len(item_ids.len());
            ranking.run_iterations(&new_state.win_matrix, 4);
            new_state.abilities = ranking.to_vec();

            list_state_handle.set(Some(new_state.clone()));
            ranking_state.set(Some(ranking));

            let mut updated_app_state = (*persisted_state_handle).clone();
            upsert_list_state(&mut updated_app_state, &list_id, new_state);
            persist_state(&updated_app_state);
            persisted_state_handle.set(updated_app_state);

            items_status.set(FetchStatus::Idle);
            let next_match = random_matchup(item_ids.len(), None);
            current_match.set(next_match);
            drag_state_handle.set(None);
            show_reset_confirm_handle.set(false);
        })
    };

    let toggle_menu_button = {
        let menu_open = menu_open.clone();
        let show_reset_confirm = show_reset_confirm.clone();
        Callback::from(move |_: yew::MouseEvent| {
            let next = !*menu_open;
            menu_open.set(next);
            if !next {
                show_reset_confirm.set(false);
            }
        })
    };

    let menu_close_callback = {
        let menu_open = menu_open.clone();
        let show_reset_confirm = show_reset_confirm.clone();
        Callback::from(move |_| {
            if *menu_open {
                menu_open.set(false);
                show_reset_confirm.set(false);
            }
        })
    };

    {
        let drag_state = drag_state.clone();
        use_effect_with_deps(
            move |state: &Option<DragState>| {
                let background = state
                    .as_ref()
                    .and_then(|drag| swipe_background_for_delta(drag.current_x - drag.start_x));
                if let Some(window) = window() {
                    if let Some(document) = window.document() {
                        if let Some(body) = document.body() {
                            let style = body.style();
                            let transition = if background.is_some() {
                                "--swipe-alpha-start 0.05s ease, --swipe-alpha-end 0.05s ease"
                            } else {
                                "--swipe-alpha-start 1s ease, --swipe-alpha-end 1s ease"
                            };
                            let _ = style.set_property("transition", transition);
                            match background {
                                Some(bg) => {
                                    let _ = style.set_property(
                                        "--swipe-color-start",
                                        &bg.start_color_value(),
                                    );
                                    let _ = style
                                        .set_property("--swipe-color-end", &bg.end_color_value());
                                    let _ = style.set_property(
                                        "--swipe-alpha-start",
                                        &bg.start_alpha_value(),
                                    );
                                    let _ = style
                                        .set_property("--swipe-alpha-end", &bg.end_alpha_value());
                                }
                                None => {
                                    let _ = style.set_property("--swipe-alpha-start", "0");
                                    let _ = style.set_property("--swipe-alpha-end", "0");
                                }
                            }
                        }
                    }
                }
                || ()
            },
            (*drag_state).clone(),
        );
    }

    let toggle_lists = {
        let lists_expanded = lists_expanded.clone();
        Callback::from(move |_| {
            let next = !*lists_expanded;
            lists_expanded.set(next);
        })
    };

    let request_reset = {
        let show_reset_confirm = show_reset_confirm.clone();
        Callback::from(move |_| {
            show_reset_confirm.set(true);
        })
    };

    let cancel_reset = {
        let show_reset_confirm = show_reset_confirm.clone();
        Callback::from(move |_| {
            show_reset_confirm.set(false);
        })
    };

    let confirm_reset = {
        let on_reset = on_reset.clone();
        Callback::from(move |_| {
            on_reset.emit(());
        })
    };

    let on_select_list = {
        let selected_list = selected_list.clone();
        let menu_open = menu_open.clone();
        let show_reset_confirm = show_reset_confirm.clone();
        Callback::from(move |list_id: String| {
            selected_list.set(Some(list_id.clone()));
            show_reset_confirm.set(false);
            menu_open.set(false);
        })
    };

    let menu_markup = render_menu(
        *menu_open,
        *lists_expanded,
        *show_reset_confirm,
        &list_status,
        &lists,
        &selected_list,
        &loaded_list,
        &ranking_state,
        &list_state,
        menu_close_callback.clone(),
        on_select_list,
        toggle_lists.clone(),
        request_reset.clone(),
        cancel_reset.clone(),
        confirm_reset.clone(),
    );

    let flash_overlay = match *flash_side {
        Some(WinnerSide::Left) => html! { <div class="decision-flash left"></div> },
        Some(WinnerSide::Right) => html! { <div class="decision-flash right"></div> },
        None => html! {},
    };

    let (matchup_markup, pointer_callbacks) = render_matchup_area(
        &items_status,
        &loaded_list,
        &current_match,
        &drag_state,
        &card_transition,
        &flash_side,
        &on_match_result,
    );

    html! {
        <>
            { flash_overlay }
            <div class="app-container"
                onpointerdown={pointer_callbacks.down.clone()}
                onpointermove={pointer_callbacks.move_cb.clone()}
                onpointerup={pointer_callbacks.up.clone()}
                onpointercancel={pointer_callbacks.cancel.clone()}>
                <button class={classes!("hamburger-button", if *menu_open { "open" } else { "" })}
                    data-swipe-ignore="true"
                    onclick={toggle_menu_button.clone()}>
                    <span></span>
                    <span></span>
                    <span></span>
                </button>
                { menu_markup }
                <main class="content single-column">
                    { matchup_markup }
                </main>
            </div>
        </>
    }
}

fn render_menu(
    menu_open: bool,
    lists_expanded: bool,
    show_reset_confirm: bool,
    status: &UseStateHandle<FetchStatus>,
    lists: &UseStateHandle<Option<Vec<ListInfo>>>,
    selected_list: &UseStateHandle<Option<String>>,
    loaded: &UseStateHandle<Option<LoadedList>>,
    ranking_state: &UseStateHandle<Option<BradleyTerry>>,
    list_state: &UseStateHandle<Option<StoredListState>>,
    on_close: Callback<()>,
    on_select_list: Callback<String>,
    on_toggle_lists: Callback<()>,
    on_request_reset: Callback<()>,
    on_cancel_reset: Callback<()>,
    on_confirm_reset: Callback<()>,
) -> Html {
    let overlay_classes = classes!("menu-overlay", if menu_open { Some("open") } else { None });
    let panel_classes = classes!("menu-panel", if menu_open { Some("open") } else { None });
    let stop_click = Callback::from(|event: web_sys::MouseEvent| event.stop_propagation());
    let close_click = {
        let on_close = on_close.clone();
        Callback::from(move |_| on_close.emit(()))
    };
    let toggle_lists_click = {
        let on_toggle_lists = on_toggle_lists.clone();
        Callback::from(move |_| on_toggle_lists.emit(()))
    };
    let request_reset_click = {
        let on_request_reset = on_request_reset.clone();
        Callback::from(move |_| on_request_reset.emit(()))
    };
    let cancel_reset_click = {
        let on_cancel_reset = on_cancel_reset.clone();
        Callback::from(move |_| on_cancel_reset.emit(()))
    };
    let confirm_reset_click = {
        let on_confirm_reset = on_confirm_reset.clone();
        Callback::from(move |_| on_confirm_reset.emit(()))
    };

    let current_selection = (*selected_list).clone();

    let lists_section = match &**status {
        FetchStatus::Loading => html! { <p class="menu-placeholder">{ "Loading listsâ€¦" }</p> },
        FetchStatus::Error(message) => html! { <p class="menu-error">{ message }</p> },
        FetchStatus::Idle => {
            let Some(list_vec) = &**lists else {
                return html! { <p class="menu-placeholder">{ "No lists available." }</p> };
            };

            if list_vec.is_empty() {
                html! { <p class="menu-placeholder">{ "No lists available." }</p> }
            } else {
                html! {
                    <div class="menu-list-buttons">
                        { for list_vec.iter().map(|info| render_list_button(info, &current_selection, &on_select_list)) }
                    </div>
                }
            }
        }
    };

    let total_matches = list_state
        .deref()
        .as_ref()
        .map(|state| state.total_matches())
        .unwrap_or(0);

    let rankings = if let (Some(list), Some(ranking)) =
        ((&**loaded).as_ref(), (&**ranking_state).as_ref())
    {
        let mut items_with_scores: Vec<_> = list
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let rating = ranking.display_rating(index);
                (item.id.clone(), item.label.clone(), rating)
            })
            .collect();

        items_with_scores
            .sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        html! {
            <ul class="menu-ranking-list">
                { for items_with_scores.into_iter().map(|(id, label, rating)| {
                    html! {
                        <li key={id}>
                            <span class="item-label">{ label }</span>
                            <span class="item-rating">{ format!("{rating:.0}") }</span>
                        </li>
                    }
                }) }
            </ul>
        }
    } else {
        html! { <p class="menu-placeholder">{ "Rankings will appear once a list is loaded." }</p> }
    };

    html! {
        <div class={overlay_classes} data-swipe-ignore="true" onclick={close_click.clone()}>
            <aside class={panel_classes} data-swipe-ignore="true" onclick={stop_click}>
                <div class="menu-header">
                    <h2>{ "Menu" }</h2>
                    <button class="menu-close" onclick={close_click}>{ "Ã—" }</button>
                </div>

                <div class="menu-section">
                    <button class={classes!("menu-toggle", if lists_expanded { "expanded" } else { "" })}
                        onclick={toggle_lists_click}>
                        <span>{ "Lists" }</span>
                        <span class="chevron">{ if lists_expanded { "â–¾" } else { "â–¸" } }</span>
                    </button>
                    {
                        if lists_expanded {
                            lists_section
                        } else {
                            html! {}
                        }
                    }
                </div>

                <div class="menu-section">
                    {
                        if show_reset_confirm {
                            html! {
                                <div class="reset-confirm">
                                    <p>{ "Are you sure you want to reset the rankings?" }</p>
                                    <div class="confirm-actions">
                                        <button class="confirm-yes" onclick={confirm_reset_click.clone()}>{ "Yes" }</button>
                                        <button class="confirm-no" onclick={cancel_reset_click.clone()}>{ "No" }</button>
                                    </div>
                                </div>
                            }
                        } else {
                            html! {
                                <button class="menu-action reset" onclick={request_reset_click}>{ "Reset Rankings" }</button>
                            }
                        }
                    }
                </div>

                <div class="menu-section rankings">
                    <div class="menu-section-header">
                        <h3>{ "Current Rankings" }</h3>
                        <span class="matches-count">{ format!("Matches recorded: {total_matches}") }</span>
                    </div>
                    <div class="ranking-scroll">
                        { rankings }
                    </div>
                </div>
            </aside>
        </div>
    }
}

fn render_list_button(
    info: &ListInfo,
    current_selection: &Option<String>,
    on_select_list: &Callback<String>,
) -> Html {
    let id = info.id.clone();
    let label = info.label.clone();
    let is_active = current_selection
        .as_ref()
        .map(|selected| selected == &info.id)
        .unwrap_or(false);

    let class = if is_active {
        "list-button active"
    } else {
        "list-button"
    };

    let on_click = {
        let on_select_list = on_select_list.clone();
        Callback::from(move |_| {
            on_select_list.emit(id.clone());
        })
    };

    html! {
        <button class={class} onclick={on_click}>{ label }</button>
    }
}

fn render_matchup_area(
    status: &UseStateHandle<FetchStatus>,
    loaded: &UseStateHandle<Option<LoadedList>>,
    current_match: &UseStateHandle<Option<Matchup>>,
    drag_state: &UseStateHandle<Option<DragState>>,
    card_transition: &UseStateHandle<CardTransition>,
    flash_side: &UseStateHandle<Option<WinnerSide>>,
    on_select_winner: &Callback<WinnerSide>,
) -> (Html, PointerCallbacks) {
    match &**status {
        FetchStatus::Loading => (
            html! { <p>{ "Loading listâ€¦" }</p> },
            PointerCallbacks::noop(),
        ),
        FetchStatus::Error(message) => (
            html! { <p class="error">{ message }</p> },
            PointerCallbacks::noop(),
        ),
        FetchStatus::Idle => {
            let Some(list) = (&**loaded).as_ref() else {
                return (
                    html! { <p>{ "Select a list to begin." }</p> },
                    PointerCallbacks::noop(),
                );
            };
            let transition_state = *card_transition.deref();
            let active_drag = if matches!(transition_state, CardTransition::Idle) {
                drag_state.deref().clone()
            } else {
                None
            };
            let drag_delta = active_drag
                .as_ref()
                .map(|d| d.current_x - d.start_x)
                .unwrap_or(0.0);
            let is_dragging = active_drag.is_some();
            let mut style_parts: Vec<String> = Vec::new();
            if is_dragging {
                let background_position =
                    50.0 + ((drag_delta / (SWIPE_THRESHOLD * 3.0)).clamp(-1.0, 1.0) * 50.0);
                style_parts.push(format!(
                    "transform: translateX({:.1}px) rotate({:.2}deg); background-position-x: {:.2}%;",
                    drag_delta,
                    drag_delta * 0.05,
                    background_position
                ));
            }
            let matchup_classes = classes!(
                "matchup",
                "swipe-enabled",
                if is_dragging { Some("dragging") } else { None },
                match transition_state {
                    CardTransition::Exiting { side, .. } => match side {
                        WinnerSide::Left => Some("exiting-left"),
                        WinnerSide::Right => Some("exiting-right"),
                    },
                    CardTransition::Entering { side } => match side {
                        WinnerSide::Left => Some("entering-from-right"),
                        WinnerSide::Right => Some("entering-from-left"),
                    },
                    CardTransition::Idle => None,
                }
            );
            if let CardTransition::Exiting { side, offset } = transition_state {
                let rotation = offset * 0.05;
                let (exit_shift, rotation_shift) = match side {
                    WinnerSide::Left => ("-150vw", "-12deg"),
                    WinnerSide::Right => ("150vw", "12deg"),
                };
                style_parts.push(format!(
                    "transform: translateX({:.1}px) rotate({:.2}deg); --start-x: {:.1}px; --start-rot: {:.2}deg; --exit-shift: {}; --rotation-shift: {};",
                    offset,
                    rotation,
                    offset,
                    rotation,
                    exit_shift,
                    rotation_shift
                ));
            }
            let style = style_parts.join(" ");
            let pointer_down = {
                let drag_state = drag_state.clone();
                let card_transition = card_transition.clone();
                Callback::from(move |event: web_sys::PointerEvent| {
                    if should_ignore_swipe(event.target()) {
                        return;
                    }
                    if !matches!(*card_transition, CardTransition::Idle)
                        || drag_state.deref().is_some()
                    {
                        return;
                    }
                    event.prevent_default();
                    let pointer_id = event.pointer_id();
                    if let Some(target) = event
                        .target()
                        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                    {
                        let _ = target.set_pointer_capture(pointer_id);
                    } else if let Some(window) = web_sys::window() {
                        if let Some(document) = window.document() {
                            if let Some(body) = document.body() {
                                let _ = body.set_pointer_capture(pointer_id);
                            }
                        }
                    }

                    drag_state.set(Some(DragState {
                        pointer_id: event.pointer_id(),
                        start_x: event.client_x() as f64,
                        current_x: event.client_x() as f64,
                    }));
                })
            };

            let pointer_move = {
                let drag_state = drag_state.clone();
                let card_transition = card_transition.clone();
                Callback::from(move |event: web_sys::PointerEvent| {
                    if !matches!(*card_transition, CardTransition::Idle) {
                        return;
                    }
                    if let Some(mut state) = drag_state.deref().clone() {
                        if state.pointer_id == event.pointer_id() {
                            event.prevent_default();
                            state.current_x = event.client_x() as f64;
                            drag_state.set(Some(state));
                        }
                    }
                })
            };

            let pointer_end = {
                let drag_state = drag_state.clone();
                let on_select_winner = on_select_winner.clone();
                let card_transition = card_transition.clone();
                let flash_side = flash_side.clone();
                Callback::from(move |event: web_sys::PointerEvent| {
                    if let Some(state) = drag_state.deref().clone() {
                        if state.pointer_id == event.pointer_id() {
                            if let Some(target) = event
                                .target()
                                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                            {
                                let _ = target.release_pointer_capture(event.pointer_id());
                            }
                            let delta = state.current_x - state.start_x;
                            if delta.abs() > SWIPE_THRESHOLD
                                && matches!(*card_transition, CardTransition::Idle)
                            {
                                let side = if delta > 0.0 {
                                    WinnerSide::Right
                                } else {
                                    WinnerSide::Left
                                };
                                card_transition.set(CardTransition::Exiting {
                                    side,
                                    offset: delta,
                                });
                                flash_side.set(Some(side));

                                {
                                    let flash_side = flash_side.clone();
                                    Timeout::new(FLASH_CLEAR_DELAY_MS, move || {
                                        flash_side.set(None);
                                    })
                                    .forget();
                                }

                                {
                                    let on_select_winner = on_select_winner.clone();
                                    Timeout::new(MATCH_RESOLVE_DELAY_MS, move || {
                                        on_select_winner.emit(side);
                                    })
                                    .forget();
                                }
                            }
                            drag_state.set(None);
                        }
                    }
                })
            };

            let pointer_cancel = {
                let drag_state = drag_state.clone();
                Callback::from(move |event: web_sys::PointerEvent| {
                    if let Some(state) = drag_state.deref().clone() {
                        if state.pointer_id == event.pointer_id() {
                            if let Some(target) = event
                                .target()
                                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                            {
                                let _ = target.release_pointer_capture(event.pointer_id());
                            }
                            drag_state.set(None);
                        }
                    }
                })
            };

            let down_for_html = pointer_down.clone();
            let move_for_html = pointer_move.clone();
            let end_for_html = pointer_end.clone();
            let cancel_for_html = pointer_cancel.clone();

            let matchup_panel = match (&**current_match).as_ref() {
                Some(matchup)
                    if matchup.left_index < list.items.len()
                        && matchup.right_index < list.items.len() =>
                {
                    let left_item = &list.items[matchup.left_index];
                    let right_item = &list.items[matchup.right_index];

                    html! {
                        <div class="card-container">
                            <div class={matchup_classes}
                                style={style}
                                onpointerdown={down_for_html.clone()}
                                onpointermove={move_for_html.clone()}
                                onpointerup={end_for_html.clone()}
                                onpointercancel={cancel_for_html.clone()}>
                                <div class="card left-card">
                                    <p class="card-title">{ &left_item.label }</p>
                                    <p class="swipe-hint">{ "Swipe left" }</p>
                                </div>
                                <span class="vs-label">{ "vs" }</span>
                                <div class="card right-card">
                                    <p class="card-title">{ &right_item.label }</p>
                                    <p class="swipe-hint">{ "Swipe right" }</p>
                                </div>
                            </div>
                        </div>
                    }
                }
                _ => html! { <p>{ "Not enough unique items to create a matchup." }</p> },
            };

            let html_output = html! {
                <div class="matchup-wrapper">
                    { matchup_panel }
                </div>
            };

            let callbacks = PointerCallbacks {
                down: pointer_down,
                move_cb: pointer_move,
                up: pointer_end,
                cancel: pointer_cancel,
            };

            (html_output, callbacks)
        }
    }
}

fn resolve_selection(lists: &[ListInfo], previous: Option<String>) -> Option<String> {
    match previous {
        Some(current) => {
            if lists.iter().any(|info| info.id == current) {
                Some(current)
            } else {
                lists.first().map(|info| info.id.clone())
            }
        }
        None => lists.first().map(|info| info.id.clone()),
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    yew::Renderer::<App>::new().render();
}

fn should_ignore_swipe(target: Option<web_sys::EventTarget>) -> bool {
    target
        .and_then(|t| t.dyn_into::<Element>().ok())
        .and_then(|element| {
            element
                .closest("[data-swipe-ignore=\"true\"]")
                .ok()
                .flatten()
        })
        .is_some()
}

fn swipe_background_for_delta(delta: f64) -> Option<SwipeBackground> {
    let normalized = (delta / SWIPE_THRESHOLD).clamp(-1.0, 1.0);
    if normalized.abs() < 0.01 {
        return None;
    }

    let strength = normalized.abs();
    let start_alpha = (0.18 * strength).min(1.0);
    let end_alpha = (0.38 * strength + 0.02).min(1.0);

    if normalized < 0.0 {
        Some(SwipeBackground {
            start_rgb: (0, 88, 196),
            end_rgb: (4, 21, 64),
            start_alpha,
            end_alpha,
        })
    } else {
        Some(SwipeBackground {
            start_rgb: (255, 62, 62),
            end_rgb: (112, 8, 18),
            start_alpha,
            end_alpha,
        })
    }
}
