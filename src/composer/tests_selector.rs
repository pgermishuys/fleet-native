//! Fleet's tests for the selector dropdown that are about its rules (`SelectorDropdown.test.ts`,
//! `SelectorDropdownPlacement.test.ts`), under their names (the Vue sentence in the comment). The ones about keys
//! and clicks are in `fleet/tests/selector.rs`.
use super::choice::ModelOption;
use super::groups::{ModelPicker, model_picker_key};
use super::selector::*;

fn model(id: &str) -> ModelOption {
    let mut m = ModelOption::new("p", "P", id, id);
    m.provider = "P".into();
    m
}

fn models() -> Vec<ModelOption> {
    ["one", "two", "three"].iter().map(|id| model(id)).collect()
}

fn default() -> Item {
    default_item("Default", "Use the session default model", true)
}

fn titles(view: &[Shown]) -> Vec<String> {
    view.iter().filter_map(|s| s.title.clone()).collect()
}

fn rows_of(models: &[ModelOption], harness: Option<&str>, picker: &ModelPicker, query: &str) -> Vec<Shown> {
    let (items, sections) = model_items(default(), models, harness, picker, true);
    shown(&items, sections.as_deref(), query)
}

// selector dropdown: "shows the starred model under Favourites, then All models"
#[test]
fn shows_the_starred_model_under_favourites_then_all_models() {
    let picker = ModelPicker { favourites: vec![model_picker_key("opencode", "p", "two")], ..Default::default() };
    let view = rows_of(&models(), Some("opencode"), &picker, "");
    assert_eq!(titles(&view), ["Favourites", "All models"]);
    let starred = rows(&view).iter().filter(|r| r.item.starred == Some(true)).count();
    assert_eq!(starred, 2, "the favourite stays in All models, starred in both");
}

// selector dropdown: "flattens while typing"
#[test]
fn flattens_while_typing() {
    let view = rows_of(&models(), Some("opencode"), &ModelPicker::default(), "thr");
    assert!(titles(&view).is_empty());
    let all = rows(&view);
    assert_eq!(all.len(), 2, "Default stays first while filtering");
    assert_eq!(all[0].item.id, "");
    // Enter takes the first match, not Default.
    let active = start_active(&view, "thr", "");
    assert_eq!(all[active].item.id, models()[2].selection_key);
}

// selector dropdown: "picks the active row with the arrows and Enter" (the movement; the keys are a view test)
#[test]
fn the_arrows_move_the_highlight_one_row_and_stop_at_the_ends() {
    assert_eq!(move_active(0, true, 4), 1);
    assert_eq!(move_active(3, true, 4), 3, "no wrap past the last row");
    assert_eq!(move_active(0, false, 4), 0, "no wrap past the first row");
    assert_eq!(move_active(2, false, 4), 1);
}

// selector dropdown: "has no stars or headings without a harness"
#[test]
fn has_no_stars_or_headings_without_a_harness() {
    let view = rows_of(&models(), None, &ModelPicker::default(), "");
    assert!(titles(&view).is_empty());
    assert!(rows(&view).iter().all(|r| r.item.starred.is_none()));
}

fn priced() -> Vec<ModelOption> {
    let mut m = models();
    m[0].price_tier = Some("high".into());
    m[0].capability = Some("powerful".into());
    m[1].price_tier = Some("very_high".into());
    m
}

// selector dropdown price chip: "shows the tier on a model that has one, with its capability as the tooltip"
#[test]
fn shows_the_tier_on_a_model_that_has_one_with_its_capability_as_the_tooltip() {
    let view = rows_of(&priced(), None, &ModelPicker::default(), "");
    let chips: Vec<(String, Option<String>)> =
        rows(&view).iter().filter_map(|r| r.item.price.clone().map(|p| (p, r.item.price_note.clone()))).collect();
    assert_eq!(chips, [("High cost".to_string(), Some("Powerful".to_string())), ("Very high cost".to_string(), None)]);
}

// selector dropdown price chip: "puts the chip in the option's name and leaves Default and unpriced models bare"
#[test]
fn leaves_default_and_unpriced_models_bare() {
    let view = rows_of(&priced(), None, &ModelPicker::default(), "");
    let all = rows(&view);
    assert!(all[0].item.price.is_none(), "Default");
    assert_eq!(all[1].item.price.as_deref(), Some("High cost"));
    assert!(all[3].item.price.is_none(), "an unpriced model");
}

// selector dropdown price chip: "draws no chip for an unknown tier, and the closed picker is unchanged"
#[test]
fn draws_no_chip_for_an_unknown_tier() {
    let mut m = model("one");
    m.price_tier = Some("gold".into());
    assert_eq!(model_item(&m, None).price, None);
    assert_eq!(price_label("very_high"), Some("Very high cost"));
}

// selector dropdown price chip: "shows each model's context size at the right, and doesn't repeat the provider when it is the same on every row"
#[test]
fn shows_each_models_context_size_and_hides_a_provider_that_is_the_same_on_every_row() {
    let mut m = models();
    m[0].context_window = Some(200_000);
    let view = rows_of(&m, None, &ModelPicker::default(), "");
    let sizes: Vec<String> = rows(&view).iter().filter_map(|r| r.item.context_size.clone()).collect();
    assert_eq!(sizes, ["200k"]);
    assert!(view[0].hide_meta, "one provider on every row");
    // Two providers: the provider shows on each row.
    let mut other = model("four");
    other.provider = "Q".into();
    m.push(other);
    assert!(!rows_of(&m, None, &ModelPicker::default(), "")[0].hide_meta);
}

// selector dropdown price chip: "with plainDefault the chip says the name but the list keeps the Default row whole"
#[test]
fn the_list_keeps_the_default_row_whole() {
    let default = default_item("Default (Claude Haiku)", "x", true);
    let (items, _) = model_items(default, &models(), None, &ModelPicker::default(), true);
    assert_eq!(items[0].label, "Default (Claude Haiku)");
    assert_eq!(crate::composer::choice::plain_default_label("Default (Claude Haiku)"), "Claude Haiku");
}

// native: a filter keeps rows by label, description, provider and id, and always the Default row
#[test]
fn a_filter_reads_label_description_provider_and_id() {
    let mut m = model("alpha");
    m.description = "Writes poems".into();
    m.provider = "Meadow".into();
    let (items, _) = model_items(default(), &[m, model("beta")], None, &ModelPicker::default(), true);
    for q in ["ALPHA", " poems ", "meadow", "alpha\"]"] {
        let kept = filter(&items, q);
        assert_eq!(kept.len(), 2, "{q:?} keeps Default and the one model");
    }
    assert_eq!(filter(&items, "zzz").len(), 1, "only Default");
}

// SelectorDropdownPlacement: "opens downward, inside the dialog, when auto and there is no room above"
#[test]
fn opens_downward_inside_the_dialog_when_auto_and_there_is_no_room_above() {
    // A dialog spanning 100..700, the chip at 130..158.
    let p = place(true, 130.0, 158.0, 100.0, 700.0, 900.0);
    assert!(p.down);
    assert!(p.max_height.unwrap() <= 320.0);
}

// SelectorDropdownPlacement: "stays upward when auto and there is room above"
#[test]
fn stays_upward_when_auto_and_there_is_room_above() {
    assert!(!place(true, 560.0, 588.0, 100.0, 700.0, 900.0).down);
}

// SelectorDropdownPlacement: "opens upward, capped to the room above, when auto and the selector is near the bottom"
#[test]
fn opens_upward_capped_to_the_room_above_when_auto_and_near_the_bottom() {
    let p = place(true, 660.0, 688.0, 100.0, 700.0, 900.0);
    assert!(!p.down);
    assert!(p.max_height.unwrap() <= 320.0);
    // With only 100 above the cap never goes under 120.
    assert_eq!(place(true, 200.0, 228.0, 100.0, 700.0, 900.0).max_height.map(|h| h >= 120.0), Some(true));
}

// SelectorDropdownPlacement: "opens upward with no inline cap by default, wherever the selector is"
#[test]
fn opens_upward_with_no_cap_by_default_wherever_the_selector_is() {
    assert_eq!(place(false, 130.0, 158.0, 100.0, 700.0, 900.0), Placement { down: false, max_height: None });
}

// row 3.4: the 380 px menu slides left, as far as it must, to stay 8 px inside its pane
#[test]
fn the_menu_slides_left_to_stay_inside_its_pane() {
    assert_eq!(shift_left(100.0, 1000.0), 0.0, "room to spare");
    assert_eq!(shift_left(700.0, 1000.0), 700.0 + 380.0 - 992.0, "slides by what sticks out");
    assert_eq!(shift_left(10.0, 300.0), 2.0, "never past 8 px from the left edge: left - 8");
    assert_eq!(shift_left(4.0, 300.0), 0.0, "a chip nearer the edge than 8 px stays put");
    assert_eq!(menu_width(1360.0), 380.0);
    assert_eq!(menu_width(300.0), 284.0, "a window under 396 px shrinks the menu");
    assert!((menu_height(400.0) - 240.0).abs() < 0.01, "60vh");
    assert_eq!(menu_height(1000.0), 320.0);
}

// row 3.10: N is the rows whose middle is below the visible bottom
#[test]
fn counts_the_rows_that_run_on_below_the_visible_part() {
    let rows = [(0.0, 40.0), (40.0, 40.0), (80.0, 40.0), (120.0, 40.0)];
    assert_eq!(more_below(&rows, 100.0), 1, "the middles are 20, 60, 100 and 140: only the last is below 100");
    assert_eq!(more_below(&rows, 99.0), 2);
    assert_eq!(more_below(&rows, 160.0), 0);
    assert_eq!(more_below(&rows, 60.0), 2);
}
