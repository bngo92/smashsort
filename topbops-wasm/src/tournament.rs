use rand::prelude::SliceRandom;
use std::collections::HashMap;
use topbops::List;
use yew::{html, Callback, Component, Context, Html, Properties};
use yew_router::history::Location;
use yew_router::scope_ext::RouterScopeExt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node<T: Clone> {
    pub item: T,
    pub disabled: bool,
    depth: usize,
    pair: usize,
}

fn interleave(src: &mut Vec<i32>, dst: &mut Vec<i32>) {
    *dst = Interleave::new(src.drain(..).map(|i| -i), std::mem::take(dst).into_iter()).collect();
}

struct Interleave<I: Iterator, J: Iterator<Item = I::Item>> {
    iter1: I,
    iter2: J,
    flag: bool,
}

impl<I: Iterator, J: Iterator<Item = I::Item>> Interleave<I, J> {
    fn new(iter1: I, iter2: J) -> Interleave<I, J> {
        Interleave {
            iter1,
            iter2,
            flag: false,
        }
    }
}

impl<I: Iterator, J: Iterator<Item = I::Item>> Iterator for Interleave<I, J> {
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        self.flag = !self.flag;
        match self.flag {
            true => self.iter1.next(),
            false => self.iter2.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let h1 = self.iter1.size_hint();
        let h2 = self.iter2.size_hint();
        (h1.0 + h2.0, h1.1.zip(h1.1).map(|(h1, h2)| h1 + h2))
    }
}

/// Generate a balanced binary tree with enough leaves for all items.
///
/// Items are ordered such that high seeds are matched with low seeds or have bye rounds.
/// The tree is generated by splitting leaf nodes:
///
///      *
///     / \
///    1   2
///
///      *
///     / \
///   *     *
///  / \   / \
/// 1   4 3   2
///
/// The tree is actually generated by precalculating indexes instead of iteratively splitting leaf nodes.
/// The tree is also represented as a flat vec.
/// Spaces between nodes represent results between children nodes.
///
/// Start:
/// 1
/// *
/// 4
/// *
/// 3
/// *
/// 2
///
/// End:
/// 1
/// 1
/// 4
/// 1
/// 3
/// 2
/// 2
#[derive(Clone, Eq, PartialEq)]
pub struct TournamentData<T: Clone> {
    initial_data: Vec<Option<Node<T>>>,
    pub data: Vec<Option<Node<T>>>,
}

impl<T: Clone> TournamentData<T> {
    pub fn new(items: Vec<T>, default: T) -> TournamentData<T> {
        let depth = (items.len() as f64).log2().ceil() as u32;

        // Build arrays of steps between items with ascending seeds
        // Steps for the next level can be calculated from the previous level
        let mut top = Vec::new();
        let mut next_top = Vec::new();
        let mut bottom = Vec::new();
        let mut next_bottom = Vec::new();
        for d in 0..depth + 1 {
            let len = (2 << d) - 2;
            let mut current = 0;
            interleave(&mut next_top, &mut top);
            for next_i in &top {
                let i = len - 2 * current;
                next_top.push(i);
                current += i + next_i;
            }
            let i = len - 2 * current;
            next_top.push(i);
            current += i - 2;
            interleave(&mut next_bottom, &mut bottom);
            for next_i in &bottom {
                let i = len - 2 * current;
                next_bottom.push(i);
                current += i + next_i;
            }
            let i = len - 2 * current;
            next_bottom.push(i);
        }

        // All nodes with even indexes are leaf nodes
        // The tree is otherwise complete (all other levels are filled) so create nodes at odd
        // indexes
        let mut data: Vec<_> = [
            None,
            Some(Node {
                item: default,
                disabled: true,
                depth: usize::MAX,
                pair: usize::MAX,
            }),
        ]
        .into_iter()
        .cycle()
        .take((2 << depth) - 1)
        .collect();

        // Create leaf nodes in the first two layers
        let len = (1 << depth) - items.len();
        let iter = std::iter::once(0)
            .chain(Interleave::new(next_top.into_iter(), top.into_iter()))
            .chain(std::iter::once(-2))
            .chain(Interleave::new(next_bottom.into_iter(), bottom.into_iter()));
        let mut current = 0;
        for (i, (item, step)) in items.into_iter().zip(iter).enumerate() {
            current += step;
            let index = if len > i {
                if current % 4 == 0 {
                    current + 1
                } else {
                    current - 1
                }
            } else {
                current
            };
            data[index as usize] = Some(Node {
                item,
                disabled: false,
                depth: usize::MAX,
                pair: usize::MAX,
            });
        }

        // Iterate over the final set of nodes and assign depth and pair values
        for i in 0..data.len() {
            if let Some(item) = data[i].clone() {
                // This block is only entered once for each node pair
                if item.depth == usize::MAX {
                    let depth = i.trailing_ones() as usize;
                    data[i].as_mut().unwrap().depth = depth;
                    let pair = i + (2 << depth);
                    if pair < data.len() {
                        data[i].as_mut().unwrap().pair = pair;
                        data[pair].as_mut().unwrap().depth = depth;
                        data[pair].as_mut().unwrap().pair = i;
                    }
                }
            }
        }
        TournamentData {
            initial_data: data.clone(),
            data,
        }
    }

    /// Assign the node with the index i to win their round.
    ///
    /// The current node pair is disabled and the parent node is updated and enabled.
    fn update(&mut self, i: usize) {
        let Some(item) = self.data[i].clone() else { return; };
        if !item.disabled && !self.data[item.pair].as_ref().unwrap().disabled {
            self.data[i].as_mut().unwrap().disabled = true;
            self.data[item.pair].as_mut().unwrap().disabled = true;
            let win = self.data[i].as_ref().unwrap().item.clone();
            let mut parent = self.data[(i + item.pair) / 2].as_mut().unwrap();
            parent.item = win;
            parent.disabled = false;
        }
    }
}

pub enum Msg {
    Load(List, bool),
    Update(usize),
    Reset,
}

#[derive(Eq, PartialEq, Properties)]
pub struct TournamentProps {
    pub id: String,
}

pub struct Tournament {
    title: String,
    state: TournamentData<String>,
    iframe: Option<String>,
}

impl Component for Tournament {
    type Message = Msg;
    type Properties = TournamentProps;

    fn create(ctx: &Context<Self>) -> Self {
        let query = ctx
            .link()
            .location()
            .unwrap()
            .query::<HashMap<String, String>>()
            .unwrap_or_default();
        let random = matches!(query.get("mode").map_or("", String::as_str), "random");
        let id = ctx.props().id.clone();
        ctx.link().send_future(async move {
            let list = crate::fetch_list("demo", &id).await.unwrap();
            Msg::Load(list, random)
        });
        Tournament {
            title: String::new(),
            state: TournamentData {
                initial_data: Vec::new(),
                data: Vec::new(),
            },
            iframe: None,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let onclick = ctx.link().callback(|_| Msg::Reset).clone();
        html! {
            if !self.title.is_empty() {
                <div class="row">
                    <div class="col-10">
                        <h1>{self.title.clone()}</h1>
                    </div>
                    <div class="col-2 align-self-center">
                        <button type="button" class="btn btn-danger w-100 mb-1" {onclick}>{"Reset"}</button>
                    </div>
                </div>
                <TournamentBracket data={self.state.clone()} on_click_select={ctx.link().callback(Msg::Update)}/>
                if let Some(src) = self.iframe.clone() {
                    <div class="row">
                        <div class="col-8 offset-2">
                            <iframe width="100%" height="380" frameborder="0" {src}></iframe>
                        </div>
                    </div>
                }
            }
        }
    }

    fn update(&mut self, _: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Load(list, random) => {
                let mut items: Vec<_> = list.items.into_iter().map(|i| i.name).collect();
                // TODO: order by score
                if random {
                    items.shuffle(&mut rand::thread_rng());
                }
                self.title = if random {
                    format!("{} - Random Tournament", list.name)
                } else {
                    format!("{} - Tournament", list.name)
                };
                self.state = TournamentData::new(items, String::new());
                self.iframe = list.iframe;
            }
            Msg::Update(i) => {
                self.state.update(i);
            }
            Msg::Reset => {
                self.state.data = self.state.initial_data.clone();
            }
        }
        true
    }
}

#[derive(PartialEq, Properties)]
pub struct TournamentBracketProps {
    pub data: TournamentData<String>,
    pub on_click_select: Callback<usize>,
}

pub struct TournamentBracket;

impl Component for TournamentBracket {
    type Message = ();
    type Properties = TournamentBracketProps;

    fn create(_: &Context<Self>) -> Self {
        TournamentBracket
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        props
            .data
            .data
            .iter()
            .enumerate()
            .map(|(i, item)| {
                if let Some(item) = item {
                    let class = match item.depth {
                        5 => "col-2 offset-10",
                        4 => "col-2 offset-8",
                        3 => "col-2 offset-6",
                        2 => "col-2 offset-4",
                        1 => "col-2 offset-2",
                        _ => "col-2",
                    };
                    let onclick = props.on_click_select.clone();
                    let onclick = Callback::from(move |_| onclick.emit(i));
                    let title = item.item.clone();
                    let disabled = item.disabled;
                    html! {
                        <div class="row">
                            <div {class}>
                                <button type="button" class="btn btn-warning truncate w-100" style="height: 38px" {title} {disabled} {onclick}>{item.item.clone()}</button>
                            </div>
                        </div>
                    }
                } else {
                    html! { <div style="height: 38px"></div> }
                }
            })
            .collect()
    }
}
