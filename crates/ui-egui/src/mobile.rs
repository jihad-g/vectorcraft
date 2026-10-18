//! Phone layout: the window narrower than [`PHONE_WIDTH`] (or Window › Phone Layout › On) swaps the
//! desktop chrome (menu bar, control bar, Tools panel, dock, status bar, document tabs) for a
//! touch layout built from the same parts:
//!
//! - a top bar: Menu (every menu as a drill-down list), Home, the document's name, Undo, Redo and
//!   Panels;
//! - a bottom tool bar: the fill/stroke proxy and the toolbar's slots as big buttons that scroll
//!   sideways; a tap on the active slot (or a long press on any) shows the rest of its group in a
//!   strip above the bar;
//! - a bottom sheet: one panel at a time (Properties, Layers, Color, Swatches… or any panel from
//!   More), its contents the dock's own panel bodies.
//!
//! Commands still run through `app.run` / `menus::invoke`; this module only lays them out.
//! Canvas gestures (pinch zoom, two-finger pan, long press for the context menu) are the canvas's
//! own (`touch.rs`, the web shell's `pencil.rs`).

use egui::{Color32, CornerRadius, Sense, Stroke, Ui, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vectorcraft_tools::tool_info;

use crate::menus::{self, Entry, Item};
use crate::theme::Tokens;
use crate::{VectorcraftApp, icons, widgets};

/// Below this window width (points) the Auto setting uses the phone layout.
pub const PHONE_WIDTH: f32 = 600.0;
const TOP_BAR: f32 = 48.0;
const TOOL_BAR: f32 = 62.0;
const BUTTON: f32 = 44.0;
const ROW: f32 = 46.0;
/// Height of the tool group strip (icon and a two-line name).
const STRIP: f32 = 70.0;

/// Window › Phone Layout: when the touch layout replaces the desktop one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhoneLayout {
    /// Windows narrower than [`PHONE_WIDTH`].
    #[default]
    Auto,
    On,
    Off,
}

impl PhoneLayout {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "on" | "true" => Some(Self::On),
            "off" | "false" => Some(Self::Off),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

/// What the phone layout keeps between frames (not saved).
#[derive(Clone, Debug, Default)]
struct Phone {
    /// The panel in the bottom sheet, when it is up.
    sheet: Option<String>,
    /// The menu list is up; the path of menu and submenu indices it shows (empty: the menu titles).
    menu: Option<Vec<usize>>,
    /// The toolbar slot whose group shows in the strip above the tool bar.
    group: Option<usize>,
}

fn state_id() -> egui::Id {
    egui::Id::new("phone-layout")
}

fn load(ctx: &egui::Context) -> Phone {
    ctx.data(|d| d.get_temp::<Phone>(state_id())).unwrap_or_default()
}

fn store(ctx: &egui::Context, p: Phone) {
    ctx.data_mut(|d| d.insert_temp(state_id(), p));
}

/// Whether this frame uses the phone layout.
pub fn active(app: &VectorcraftApp, ctx: &egui::Context) -> bool {
    match app.ui.phone_layout {
        PhoneLayout::On => true,
        PhoneLayout::Off => false,
        PhoneLayout::Auto => ctx.content_rect().width() < PHONE_WIDTH,
    }
}

/// Panels one tap away in the sheet's tab row, in this order; the rest are under More.
const QUICK: &[(&str, &str, &str)] = &[
    ("properties", "Properties", "dc-options"),
    ("layers", "Layers", "layers"),
    ("color", "Color", "palette"),
    ("swatches", "Swatches", "swatch-book"),
    ("stroke", "Stroke", "dc-stroke"),
    ("transform", "Transform", "dc-transform-panel"),
    ("align", "Align", "dc-align"),
    ("pathfinder", "Pathfinder", "dc-pathfinder"),
    ("appearance", "Appearance", "dc-appearance"),
    ("transparency", "Transparency", "dc-transparency"),
    ("gradient", "Gradient", "dc-gradient"),
    ("character", "Character", "type"),
    ("artboards", "Artboards", "dc-artboards"),
    ("history", "History", "history"),
];

/// The phone chrome around the canvas: call in place of the desktop bars, before the central panel.
pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let mut p = load(&ctx);
    // A panel opened from a menu (Window › Color…) or by a command shows in the sheet.
    if let Some(id) = app.ui.open_panel.take() {
        p.sheet = Some(id);
    }
    // Escape (Android's Back sends it) puts away the topmost: the menu, the tool group, the sheet.
    if app.ui.dialog.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        if p.menu.is_some() {
            p.menu = None;
        } else if p.group.is_some() {
            p.group = None;
        } else {
            p.sheet = None;
        }
    }
    top_bar(app, ui, &mut p);
    tool_bar(app, ui, &mut p);
    if p.sheet.is_some() {
        sheet(app, ui, &mut p);
    }
    group_strip(app, &ctx, &mut p);
    if p.menu.is_some() {
        menu_list(app, &ctx, &mut p);
    }
    store(&ctx, p);
}

fn top_bar(app: &mut VectorcraftApp, ui: &mut Ui, p: &mut Phone) {
    let t = Tokens::get(ui.ctx());
    let frame = egui::Frame::NONE.fill(t.app_bar).inner_margin(egui::Margin::symmetric(4, 0)).stroke(Stroke::new(1.0, t.border));
    egui::Panel::top("phone_top_bar").exact_size(TOP_BAR).resizable(false).frame(frame).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if widgets::icon_button(ui, "menu", "Menu", p.menu.is_some(), BUTTON).clicked() {
                p.menu = if p.menu.is_some() { None } else { Some(vec![]) };
                p.group = None;
            }
            if widgets::icon_button(ui, "house", "Home", menus::home_showing(app), BUTTON).clicked() {
                p.menu = None;
                app.run("app.home", json!({})).ok();
            }
            // Right side first, so the title takes what is left.
            let rest = ui.available_rect_before_wrap();
            let mut right = ui.new_child(egui::UiBuilder::new().max_rect(rest).layout(egui::Layout::right_to_left(egui::Align::Center)));
            let r = &mut right;
            r.spacing_mut().item_spacing.x = 2.0;
            let open = p.sheet.is_some();
            if widgets::icon_button(r, "dc-options", "Panels", open, BUTTON).clicked() {
                p.sheet = if open { None } else { Some("properties".into()) };
                p.menu = None;
            }
            for (icon, tip, cmd) in [("redo-2", "Redo", "edit.redo"), ("undo-2", "Undo", "edit.undo")] {
                let on = menus::enabled(app, cmd);
                if widgets::icon_button_enabled(r, icon, tip, false, on, BUTTON).clicked() {
                    menus::invoke(app, cmd, json!({}));
                }
            }
            let title_rect = egui::Rect::from_min_max(rest.min + vec2(6.0, 0.0), pos2(r.min_rect().left() - 6.0, rest.bottom()));
            let name = app.session.active().map(|d| d.title()).unwrap_or_else(|| "VectorCraft".to_string());
            let dirty = app.session.active().is_some_and(|d| d.is_dirty());
            let text = if dirty { format!("{name} •") } else { name };
            if title_rect.width() > 24.0 {
                let galley = ui.painter().layout(text, egui::FontId::proportional(14.0), t.text, title_rect.width());
                let at = pos2(title_rect.left(), title_rect.center().y - galley.size().y / 2.0);
                ui.painter().with_clip_rect(title_rect).galley(at, galley, t.text);
            }
        });
    });
}

fn tool_bar(app: &mut VectorcraftApp, ui: &mut Ui, p: &mut Phone) {
    let t = Tokens::get(ui.ctx());
    let frame = egui::Frame::NONE.fill(t.app_bar).inner_margin(egui::Margin::symmetric(6, 0)).stroke(Stroke::new(1.0, t.border));
    egui::Panel::bottom("phone_tool_bar").exact_size(TOOL_BAR).resizable(false).frame(frame).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            // Fill and stroke: a tap brings the swatch under it to the front, as on the desktop.
            crate::panels::proxy(app, ui, 40.0);
            ui.add_space(6.0);
            let (sep, _) = ui.allocate_exact_size(vec2(1.0, 36.0), Sense::hover());
            ui.painter().rect_filled(sep, 0.0, t.border);
            ui.add_space(4.0);
            egui::ScrollArea::horizontal().id_salt("phone-tools").scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(
                ui,
                |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let active = app.session.tool_id().to_string();
                    for (k, (_, slot)) in crate::toolbar::slots(app).into_iter().enumerate() {
                        let Some(&first) = slot.first() else { continue };
                        let is_active = slot.iter().any(|s| *s == active);
                        let shown_id =
                            if is_active { active.clone() } else { app.ui.slot_tool.get(first).cloned().unwrap_or_else(|| first.to_string()) };
                        let Some(info) = tool_info(&shown_id).or_else(|| tool_info(first)) else { continue };
                        let (rect, resp) = ui.allocate_exact_size(vec2(BUTTON, BUTTON), Sense::click());
                        let bg = if is_active || p.group == Some(k) {
                            t.tool_active
                        } else if resp.hovered() {
                            t.hover
                        } else {
                            Color32::TRANSPARENT
                        };
                        ui.painter().rect_filled(rect, CornerRadius::same(8), bg);
                        let tint = if is_active { t.text_strong } else { t.icon };
                        icons::paint(ui, icons::tool_icon(info.icon), egui::Rect::from_center_size(rect.center(), vec2(24.0, 24.0)), tint);
                        if slot.len() > 1 {
                            let c = rect.right_bottom() + vec2(-6.0, -6.0);
                            ui.painter().add(egui::Shape::convex_polygon(vec![c, c + vec2(-5.0, 0.0), c + vec2(0.0, -5.0)], t.icon, Stroke::NONE));
                        }
                        let label = crate::toolbar::tip(info);
                        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, is_active, &label));
                        if resp.long_touched() || resp.secondary_clicked() {
                            if slot.len() > 1 {
                                p.group = Some(k);
                            }
                        } else if resp.clicked() {
                            if is_active && slot.len() > 1 {
                                p.group = if p.group == Some(k) { None } else { Some(k) };
                            } else {
                                p.group = None;
                                app.select_tool(info.id);
                            }
                            p.menu = None;
                        }
                    }
                },
            );
        });
    });
}

/// The rest of a tool group, in a strip just above the tool bar.
fn group_strip(app: &mut VectorcraftApp, ctx: &egui::Context, p: &mut Phone) {
    let Some(k) = p.group else { return };
    let slots = crate::toolbar::slots(app);
    let Some((_, slot)) = slots.get(k) else {
        p.group = None;
        return;
    };
    let t = Tokens::get(ctx);
    let screen = ctx.content_rect();
    let active = app.session.tool_id().to_string();
    let bottom = screen.bottom() - TOOL_BAR - 6.0;
    let mut chosen = None;
    let area = egui::Area::new(egui::Id::new("phone-group"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(screen.left() + 8.0, bottom - STRIP))
        .show(ctx, |ui| {
            crate::toolbar::flyout_frame(&t, false).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
                ui.set_max_width(screen.width() - 16.0);
                egui::ScrollArea::horizontal().id_salt("phone-group-scroll").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for id in slot.iter() {
                            let Some(info) = tool_info(id) else { continue };
                            let (rect, resp) = ui.allocate_exact_size(vec2(BUTTON + 14.0, STRIP - 8.0), Sense::click());
                            let on = *id == active;
                            if on {
                                ui.painter().rect_filled(rect, CornerRadius::same(8), t.tool_active);
                            }
                            icons::paint(
                                ui,
                                icons::tool_icon(info.icon),
                                egui::Rect::from_center_size(rect.center_top() + vec2(0.0, 18.0), vec2(22.0, 22.0)),
                                if on { t.text_strong } else { t.icon },
                            );
                            // "Direct Selection Tool" → "Direct Selection": the strip has no room for the suffix.
                            let name = tl!(info.label).trim_end_matches(" Tool");
                            let galley = ui.painter().layout(name.to_string(), egui::FontId::proportional(9.5), t.text, rect.width() - 2.0);
                            ui.painter().galley(pos2(rect.center().x - galley.size().x / 2.0, rect.top() + 32.0), galley, t.text);
                            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, tl!(info.label)));
                            if resp.clicked() {
                                chosen = Some(info.id);
                            }
                        }
                    });
                });
            });
        });
    if let Some(id) = chosen {
        app.select_tool(id);
        p.group = None;
    } else if ctx.input(|i| i.pointer.any_pressed())
        && ctx.pointer_interact_pos().is_some_and(|at| !area.response.rect.contains(at) && at.y < screen.bottom() - TOOL_BAR)
    {
        // A press anywhere else above the tool bar (the canvas, the sheet) puts the strip away.
        p.group = None;
    }
}

fn sheet(app: &mut VectorcraftApp, ui: &mut Ui, p: &mut Phone) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let Some(current) = p.sheet.clone() else { return };
    let screen = ctx.content_rect();
    // Leaves the canvas at least a third of the screen.
    let tall = ((screen.height() - TOP_BAR - TOOL_BAR) * 0.55).clamp(160.0, 520.0);
    let frame = egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius { nw: 12, ne: 12, sw: 0, se: 0 });
    egui::Panel::bottom("phone_sheet").exact_size(tall).resizable(false).show_separator_line(false).frame(frame).show(ui, |ui| {
        // The tab row: quick panels, More, close.
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let row = ui.available_rect_before_wrap();
            let tabs_rect = egui::Rect::from_min_max(row.min, pos2(row.right() - BUTTON - 4.0, row.top() + BUTTON));
            let mut tabs = ui.new_child(egui::UiBuilder::new().max_rect(tabs_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
            egui::ScrollArea::horizontal()
                .id_salt("phone-sheet-tabs")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(&mut tabs, |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (id, label, icon) in QUICK {
                        let on = current == *id;
                        if widgets::icon_button(ui, icon, label, on, BUTTON - 4.0).clicked() {
                            p.sheet = Some((*id).to_string());
                        }
                    }
                    let more = widgets::icon_button(ui, "ellipsis", "More panels", !QUICK.iter().any(|q| q.0 == current), BUTTON - 4.0);
                    egui::Popup::menu(&more).show(|ui| {
                        widgets::menu_scroll(ui, |ui| {
                            ui.set_min_width(200.0);
                            for (id, label) in crate::state::all_panels() {
                                if ui.add(egui::Button::new(tl!(label)).min_size(vec2(200.0, 34.0))).clicked() {
                                    p.sheet = Some(id.to_string());
                                    ui.close();
                                }
                            }
                        });
                    });
                });
            let close = egui::Rect::from_min_size(pos2(row.right() - BUTTON, row.top()), vec2(BUTTON, BUTTON));
            let resp = ui.interact(close, ui.id().with("phone-sheet-close"), Sense::click());
            if widgets::paint_icon_button(ui, resp, "x", "Close", false).clicked() {
                p.sheet = None;
            }
            ui.allocate_space(vec2(0.0, BUTTON));
        });
        // The panel's name, then its body.
        let name = crate::state::all_panels().find(|(id, _)| *id == current).map_or(current.as_str(), |(_, l)| l);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(egui::RichText::new(tl!(name)).size(13.0).strong().color(t.text_strong));
        });
        let width = ui.available_width();
        let body = (ui.available_height() - 4.0).max(60.0);
        match current.as_str() {
            // These two size or scroll themselves.
            "properties" | "layers" => crate::dock::panel_body(app, ui, &current, width, body),
            _ => {
                egui::ScrollArea::vertical().id_salt(("phone-sheet-body", current.as_str())).auto_shrink([false, false]).show(ui, |ui| {
                    crate::dock::panel_body(app, ui, &current, width, body);
                });
            }
        }
    });
}

/// The menus as a full-width list that drills into submenus, over the canvas.
fn menu_list(app: &mut VectorcraftApp, ctx: &egui::Context, p: &mut Phone) {
    let t = Tokens::get(ctx);
    let screen = ctx.content_rect();
    let tree: Vec<(&str, Vec<Item>)> =
        menus::menu_tree_named(app.session.prefs.font_names_in_english).into_iter().filter(|(title, _)| *title != "VectorCraft").collect();
    let path = p.menu.clone().unwrap_or_default();
    // The list the path leads to, and its title.
    let mut title: String = tl!("Menu").to_string();
    let mut items: Option<&[Item]> = None;
    if let Some((&first, rest)) = path.split_first()
        && let Some((name, list)) = tree.get(first)
    {
        title = tl!(name).to_string();
        let mut cur: &[Item] = list.as_slice();
        for &i in rest {
            match cur.get(i) {
                Some(Item::Sub(label, children)) => {
                    title = tl!(label).to_string();
                    cur = children.as_slice();
                }
                _ => break,
            }
        }
        items = Some(cur);
    }
    let mut go: Option<Vec<usize>> = None;
    let mut run: Option<(String, Value)> = None;
    let mut close = false;
    let rect = egui::Rect::from_min_max(pos2(screen.left(), screen.top() + TOP_BAR), screen.max);
    egui::Area::new(egui::Id::new("phone-menu")).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
        ui.set_min_size(rect.size());
        ui.set_max_size(rect.size());
        ui.painter().rect_filled(rect, 0.0, t.panel);
        ui.horizontal(|ui| {
            ui.set_height(ROW);
            if !path.is_empty() && widgets::icon_button(ui, "chevron-left", "Back", false, BUTTON).clicked() {
                let mut up = path.clone();
                up.pop();
                go = Some(up);
            }
            ui.add_space(6.0);
            ui.label(egui::RichText::new(&title).size(16.0).strong().color(t.text_strong));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, "x", "Close", false, BUTTON).clicked() {
                    close = true;
                }
            });
        });
        widgets::divider(ui);
        egui::ScrollArea::vertical().id_salt(("phone-menu-scroll", path.clone())).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            match items {
                None => {
                    for (i, (name, _)) in tree.iter().enumerate() {
                        if row(ui, &t, tl!(name), None, true, true).clicked() {
                            go = Some(vec![i]);
                        }
                    }
                }
                Some(list) => {
                    for (i, it) in list.iter().enumerate() {
                        let Some(e) = menus::entry(app, it) else { continue };
                        match e {
                            Entry::Sep => widgets::divider(ui),
                            Entry::Header(h) => {
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(14.0);
                                    ui.label(egui::RichText::new(h).size(11.0).color(t.text_dim));
                                });
                            }
                            Entry::Sub(label, _) => {
                                if row(ui, &t, label, None, true, true).clicked() {
                                    let mut down = path.clone();
                                    down.push(i);
                                    go = Some(down);
                                }
                            }
                            Entry::Item(r) => {
                                let mark = r.checked.map(|on| if on { "✓" } else { "" });
                                if row(ui, &t, &r.label, mark, r.enabled && r.command.is_some(), false).clicked() {
                                    run = r.target();
                                }
                            }
                        }
                    }
                }
            }
            ui.add_space(24.0);
        });
    });
    if let Some((id, params)) = run {
        p.menu = None;
        menus::invoke(app, &id, params);
    } else if close {
        p.menu = None;
    } else if let Some(g) = go {
        p.menu = Some(g);
    }
}

/// One menu row, [`ROW`] tall: a check mark (or its room), the label, and › for a submenu.
fn row(ui: &mut Ui, t: &Tokens, label: &str, mark: Option<&str>, enabled: bool, sub: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), if enabled { Sense::click() } else { Sense::hover() });
    if enabled && (resp.hovered() || resp.is_pointer_button_down_on()) {
        ui.painter().rect_filled(rect, 0.0, t.hover);
    }
    let color = if enabled { t.text } else { t.text_disabled };
    if let Some(m) = mark {
        ui.painter().text(rect.left_center() + vec2(16.0, 0.0), egui::Align2::CENTER_CENTER, m, egui::FontId::proportional(14.0), color);
    }
    let text_rect = egui::Rect::from_min_max(rect.min + vec2(30.0, 0.0), rect.max - vec2(30.0, 0.0));
    let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::proportional(15.0), color);
    ui.painter().with_clip_rect(text_rect).galley(pos2(text_rect.left(), rect.center().y - galley.size().y / 2.0), galley, color);
    if sub {
        icons::paint(ui, "chevron-right", egui::Rect::from_center_size(rect.right_center() - vec2(18.0, 0.0), vec2(16.0, 16.0)), t.text_dim);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    resp
}

/// `window.phoneLayout {layout?: auto|on|off}`: set (or, without `layout`, cycle) the setting.
pub fn set(app: &mut VectorcraftApp, p: &Value) -> Result<Value, String> {
    let next = match p.get("layout") {
        None | Some(Value::Null) => match app.ui.phone_layout {
            PhoneLayout::Auto => PhoneLayout::On,
            PhoneLayout::On => PhoneLayout::Off,
            PhoneLayout::Off => PhoneLayout::Auto,
        },
        Some(Value::String(s)) => PhoneLayout::parse(s).ok_or_else(|| format!("layout must be auto, on or off, not {s:?}"))?,
        Some(Value::Bool(b)) => {
            if *b {
                PhoneLayout::On
            } else {
                PhoneLayout::Off
            }
        }
        Some(other) => return Err(format!("layout must be auto, on or off, not {other}")),
    };
    app.ui.phone_layout = next;
    Ok(json!(next.id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setting_parses_and_names_itself() {
        for l in [PhoneLayout::Auto, PhoneLayout::On, PhoneLayout::Off] {
            assert_eq!(PhoneLayout::parse(l.id()), Some(l));
        }
        assert_eq!(PhoneLayout::parse("sideways"), None);
    }
}
