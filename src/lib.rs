pub mod data;
pub mod matchflow;
pub mod ranking;
pub mod storage;

use data::{fetch_available_lists, load_list, ListInfo, LoadedList};
use matchflow::{random_matchup, Matchup};
use ranking::BradleyTerry;
use storage::{
    align_list_state, load_list_state, load_state as load_storage_state,
    save_state as persist_state, upsert_list_state, StoredListState,
};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[derive(PartialEq, Clone)]
enum FetchStatus {
    Idle,
    Loading,
    Error(String),
}

#[derive(Clone, Copy)]
enum WinnerSide {
    Left,
    Right,
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
                            let previous =
                                previously_selected.or_else(|| persisted_state.selected_list.clone());
                            let default_selection =
                                resolve_selection(&fetched, previous);
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

        use_effect_with_deps(
            move |selected: &Option<String>| {
                match selected {
                    Some(id) => {
                        items_status.set(FetchStatus::Loading);
                        loaded_list.set(None);
                        ranking_state.set(None);
                        current_match.set(None);
                        list_state_handle.set(None);

                        let id = id.clone();
                        let items_status = items_status.clone();
                        let loaded_list = loaded_list.clone();
                        let ranking_state = ranking_state.clone();
                        let current_match = current_match.clone();
                        let list_state_handle = list_state_handle.clone();
                        let persisted_state_handle = persisted_state_handle.clone();
                        let persisted_snapshot = (*persisted_state_handle).clone();

                        spawn_local(async move {
                            match load_list(&id).await {
                                Ok(list) => {
                                    let item_ids: Vec<String> = list
                                        .items
                                        .iter()
                                        .map(|item| item.id.clone())
                                        .collect();

                                    let existing =
                                        load_list_state(&persisted_snapshot, &id).cloned();
                                    let mut stored_state =
                                        align_list_state(existing, &item_ids);

                                    let mut ranking =
                                        BradleyTerry::from_abilities(stored_state.abilities.clone());
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

                                    let next_match =
                                        random_matchup(item_ids.len(), None);

                                    list_state_handle.set(Some(stored_state));
                                    ranking_state.set(Some(ranking));
                                    current_match.set(next_match);
                                    loaded_list.set(Some(list));
                                    items_status.set(FetchStatus::Idle);
                                }
                                Err(err) => {
                                    items_status.set(FetchStatus::Error(err.to_string()));
                                    loaded_list.set(None);
                                    ranking_state.set(None);
                                    current_match.set(None);
                                    list_state_handle.set(None);
                                }
                            }
                        });
                    }
                    None => {
                        loaded_list.set(None);
                        ranking_state.set(None);
                        current_match.set(None);
                        list_state_handle.set(None);
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

            let next_match =
                random_matchup(list.items.len(), Some(&prev_match));
            current_match.set(next_match);
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

        Callback::from(move |_| {
            let Some(list_id) = (*selected_list).clone() else {
                return;
            };

            let Some(list) = (&*loaded_list).as_ref() else {
                return;
            };

            let item_ids: Vec<String> = list
                .items
                .iter()
                .map(|item| item.id.clone())
                .collect();

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
        })
    };

    html! {
        <div class="app-container">
            <header class="top-bar">
                <h1>{ "Ranking Lists" }</h1>
                <button class="reset-button" onclick={on_reset}>{ "Reset Rankings" }</button>
            </header>
            <main class="content">
                <section class="lists-panel">
                    { render_lists_panel(&list_status, &lists, &selected_list) }
                </section>
                <section class="list-details">
                    { render_list_details(
                        &items_status,
                        &loaded_list,
                        &ranking_state,
                        &list_state,
                        &current_match,
                        &on_match_result
                    ) }
                </section>
            </main>
        </div>
    }
}

fn render_lists_panel(
    status: &UseStateHandle<FetchStatus>,
    lists: &UseStateHandle<Option<Vec<ListInfo>>>,
    selected_list: &UseStateHandle<Option<String>>,
) -> Html {
    match &**status {
        FetchStatus::Loading => html! { <p>{ "Loading lists…" }</p> },
        FetchStatus::Error(message) => html! { <p class="error">{ message }</p> },
        FetchStatus::Idle => {
            let Some(list_vec) = &**lists else {
                return html! { <p>{ "No lists available." }</p> };
            };

            if list_vec.is_empty() {
                return html! { <p>{ "No lists available." }</p> };
            }

            let current_selection = (*selected_list).clone();

            html! {
                <div class="list-selector">
                    <h2>{ "Available Lists" }</h2>
                    <div class="list-buttons">
                        { for list_vec.iter().map(|info| render_list_button(info, &current_selection, selected_list)) }
                    </div>
                </div>
            }
        }
    }
}

fn render_list_button(
    info: &ListInfo,
    current_selection: &Option<String>,
    selected_list: &UseStateHandle<Option<String>>,
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
        let selected_list = selected_list.clone();
        Callback::from(move |_| {
            selected_list.set(Some(id.clone()));
        })
    };

    html! {
        <button class={class} onclick={on_click}>{ label }</button>
    }
}

fn render_list_details(
    status: &UseStateHandle<FetchStatus>,
    loaded: &UseStateHandle<Option<LoadedList>>,
    ranking_state: &UseStateHandle<Option<BradleyTerry>>,
    list_state: &UseStateHandle<Option<StoredListState>>,
    current_match: &UseStateHandle<Option<Matchup>>,
    on_select_winner: &Callback<WinnerSide>,
) -> Html {
    match &**status {
        FetchStatus::Loading => html! { <p>{ "Loading list…" }</p> },
        FetchStatus::Error(message) => html! { <p class="error">{ message }</p> },
        FetchStatus::Idle => {
            let Some(list) = (&**loaded).as_ref() else {
                return html! { <p>{ "Select a list to begin." }</p> };
            };

            let Some(state) = (&**list_state).as_ref() else {
                return html! { <p>{ "Preparing list data…" }</p> };
            };

            let ranking = (&**ranking_state).as_ref();

            let mut items_with_scores: Vec<_> = list
                .items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let score = ranking
                        .map(|system| system.log_score(index))
                        .unwrap_or_default();
                    (index, item, score)
                })
                .collect();

            items_with_scores.sort_by(|a, b| {
                b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal)
            });

            let total_matches = state.total_matches();

            let matchup_panel = match (&**current_match).as_ref() {
                Some(matchup) if matchup.left_index < list.items.len()
                    && matchup.right_index < list.items.len() =>
                {
                    let left_item = &list.items[matchup.left_index];
                    let right_item = &list.items[matchup.right_index];

                    let left_callback = {
                        let callback = on_select_winner.clone();
                        Callback::from(move |_| callback.emit(WinnerSide::Left))
                    };

                    let right_callback = {
                        let callback = on_select_winner.clone();
                        Callback::from(move |_| callback.emit(WinnerSide::Right))
                    };

                    html! {
                        <div class="matchup">
                            <div class="card">
                                <p class="card-title">{ &left_item.label }</p>
                                <button class="win-button" onclick={left_callback}>{ "Wins" }</button>
                            </div>
                            <span class="vs-label">{ "vs" }</span>
                            <div class="card">
                                <p class="card-title">{ &right_item.label }</p>
                                <button class="win-button" onclick={right_callback}>{ "Wins" }</button>
                            </div>
                        </div>
                    }
                }
                _ => html! { <p>{ "Not enough unique items to create a matchup." }</p> },
            };

            html! {
                <div class="list-preview">
                    <div class="matchup-panel">
                        { matchup_panel }
                    </div>
                    <div class="ranking-summary">
                        <div class="summary-header">
                            <h2>{ format!("Items in {}", list.info.label) }</h2>
                            <p>{ format!("Matches recorded: {total_matches}") }</p>
                        </div>
                        <ul>
                            { for items_with_scores.into_iter().map(|(_index, item, score)| {
                                html! {
                                    <li key={item.id.clone()}>
                                        <span class="item-label">{ &item.label }</span>
                                        <span class="item-rating">{ format!("{score:.2}") }</span>
                                    </li>
                                }
                            }) }
                        </ul>
                    </div>
                </div>
            }
        }
    }
}

fn resolve_selection(
    lists: &[ListInfo],
    previous: Option<String>,
) -> Option<String> {
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
