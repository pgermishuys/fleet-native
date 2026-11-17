//! Two versions of a skill's SKILL.md as a diff. Ports `components/skills/SkillDiff.vue`: a 1px box whose header holds the
//! file's name and "+N −N" in 11.5px mono, then Fleet's diff with the unchanged runs folded away, at most 340px tall, or
//! "No differences." Improve a skill's proposal uses it, and so will Settings' skill history, which is why it takes the
//! two texts and a label and nothing from the dialog.
//!
//! Fleet wraps a long line to the box's width; the diff here (`view::diff`) keeps one line per row and scrolls sideways,
//! because its number column and its code are two columns of rows of one fixed height (docs/RUNE.md).
use super::diff::bare_diff;
use crate::State;
use crate::conv::diff::{count_changes, parse_diff_lines};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use rune::{El, ScrollSpec, col, el, row, text};

/// `.skill-diff .diff-view { max-height: 340px }`.
const MAX_H: f32 = 340.0;

/// `SkillDiff`. `key` names the box; the diff inside is `<key>-diff`.
pub fn skill_diff(t: &Theme, s: &State, key: &str, before: &str, after: &str, label: &str) -> El {
    let lines = parse_diff_lines(before, after);
    let (adds, removes) = count_changes(&lines);
    let changed = adds + removes > 0;
    let mut head = row().key(format!("{key}-head")).w_full().gap(8.0).px(10.0).py(6.0).child(
        text(label.to_string()).key(format!("{key}-label")).font(mono(11.5, 400.0, 1.4)).color(t.muted).truncate().grow(1.0).min_w(0.0),
    );
    if changed {
        let count = mono(11.5, 400.0, 1.4).features("'tnum' 1");
        head = head.child(
            row()
                .key(format!("{key}-stats"))
                .gap(6.0)
                .child(text(format!("+{adds}")).font(count).color(t.diff_add_text()))
                .child(text(format!("\u{2212}{removes}")).font(count).color(t.diff_del_text())),
        );
    }
    let body = if changed {
        // The diff keeps its own border; the box's is the only one Fleet draws here, so the inner one is the diff's alone.
        el().key(format!("{key}-scroll")).w_full().max_h(MAX_H).scroll(ScrollSpec { stick_bottom: false }).child(bare_diff(
            t,
            s,
            &format!("{key}-diff"),
            &lines,
        ))
    } else {
        text("No differences.").key(format!("{key}-same")).font(sans(13.0, 400.0, 1.4)).color(t.muted).px(10.0).py(10.0)
    };
    col()
        .key(key.to_string())
        .w_full()
        .min_w(0.0)
        .clip_rounded(RADIUS_BTN)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(head)
        .child(el().w_full().h(1.0).fill(t.border))
        .child(body)
}

pub const BEFORE: &str = "---\nname: tidy-docs\ndescription: Keep the docs in step with the code\n---\n\nWhen the code changes, check the docs that name it.\n\n1. Search the docs for the names that changed.\n2. Update each mention.\n3. Run the link check.\n4. Report what you changed.\n5. Leave the changelog alone.\n6. Do not touch generated files.\n7. Keep each edit small.\n8. Ask before moving a page.\n\nEnd with one line saying what you changed.\n";
pub const AFTER: &str = "---\nname: tidy-docs\ndescription: Keep the docs in step with the code\n---\n\nWhen the code changes, check the docs that name it, and the examples that use it.\n\n1. Search the docs for the names that changed.\n2. Update each mention.\n3. Run the link check.\n4. Report what you changed.\n5. Leave the changelog alone.\n6. Do not touch generated files.\n7. Keep each edit small.\n8. Ask before moving a page.\n\nEnd with one line saying what you changed, and which pages.\nIf nothing needed changing, say so.\n";

/// Scene only: a change, and two texts that match.
pub fn demo(s: &State, t: &Theme) -> El {
    col().w_full().gap(14.0).child(skill_diff(t, s, "skill-diff-changed", BEFORE, AFTER, "tidy-docs/SKILL.md")).child(skill_diff(
        t,
        s,
        "skill-diff-same",
        AFTER,
        AFTER,
        "tidy-docs/SKILL.md",
    ))
}
