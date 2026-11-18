//! Cells Rune's text can't draw the way a terminal needs: box-drawing and block characters, drawn as strokes and fills that
//! meet the next cell's with no gap at 19 px rows (xterm's `customGlyphs`), and the underline styles and strikethrough
//! (13.11). Everything is in the row's points: a cell at column `c` starts at `c * CELL_W`, the row is `CELL_H` high.
use rune::{Color, LineCap, Path, Shape};

use super::fit::{CELL_H, CELL_W};

/// A light line is one pixel wide, a heavy one three: both centre on a pixel at 1×, so neither blurs.
const LIGHT: f32 = 1.0;
const HEAVY: f32 = 3.0;
/// Half the distance between the two lines of a double one.
const GAP: f32 = 2.0;

/// The arms of U+2500..=U+257F as left, right, up, down: 0 none, 1 light, 2 heavy, 3 double. The dashed ones are drawn
/// solid, and the arcs and diagonals are drawn by `special`.
#[rustfmt::skip]
const ARMS: [[u8; 4]; 128] = [
    [1,1,0,0],[2,2,0,0],[0,0,1,1],[0,0,2,2],[1,1,0,0],[2,2,0,0],[0,0,1,1],[0,0,2,2],
    [1,1,0,0],[2,2,0,0],[0,0,1,1],[0,0,2,2],[0,1,0,1],[0,2,0,1],[0,1,0,2],[0,2,0,2],
    [1,0,0,1],[2,0,0,1],[1,0,0,2],[2,0,0,2],[0,1,1,0],[0,2,1,0],[0,1,2,0],[0,2,2,0],
    [1,0,1,0],[2,0,1,0],[1,0,2,0],[2,0,2,0],[0,1,1,1],[0,2,1,1],[0,1,2,1],[0,1,1,2],
    [0,1,2,2],[0,2,2,1],[0,2,1,2],[0,2,2,2],[1,0,1,1],[2,0,1,1],[1,0,2,1],[1,0,1,2],
    [1,0,2,2],[2,0,2,1],[2,0,1,2],[2,0,2,2],[1,1,0,1],[2,1,0,1],[1,2,0,1],[2,2,0,1],
    [1,1,0,2],[2,1,0,2],[1,2,0,2],[2,2,0,2],[1,1,1,0],[2,1,1,0],[1,2,1,0],[2,2,1,0],
    [1,1,2,0],[2,1,2,0],[1,2,2,0],[2,2,2,0],[1,1,1,1],[2,1,1,1],[1,2,1,1],[2,2,1,1],
    [1,1,2,1],[1,1,1,2],[1,1,2,2],[2,1,2,1],[1,2,2,1],[2,1,1,2],[1,2,1,2],[2,2,2,1],
    [2,2,1,2],[2,1,2,2],[1,2,2,2],[2,2,2,2],[1,1,0,0],[2,2,0,0],[0,0,1,1],[0,0,2,2],
    [3,3,0,0],[0,0,3,3],[0,3,0,1],[0,1,0,3],[0,3,0,3],[3,0,0,1],[1,0,0,3],[3,0,0,3],
    [0,3,1,0],[0,1,3,0],[0,3,3,0],[3,0,1,0],[1,0,3,0],[3,0,3,0],[0,3,1,1],[0,1,3,3],
    [0,3,3,3],[3,0,1,1],[1,0,3,3],[3,0,3,3],[3,3,0,1],[1,1,0,3],[3,3,0,3],[3,3,1,0],
    [1,1,3,0],[3,3,3,0],[3,3,1,1],[1,1,3,3],[3,3,3,3],[0,0,0,0],[0,0,0,0],[0,0,0,0],
    [0,0,0,0],[0,0,0,0],[0,0,0,0],[0,0,0,0],[1,0,0,0],[0,0,1,0],[0,1,0,0],[0,0,0,1],[2,0,0,0],[0,0,2,0],
    [0,2,0,0],[0,0,0,2],[1,2,0,0],[0,0,1,2],[2,1,0,0],[0,0,2,1],
];

/// Whether the cell is drawn here and not by the font.
pub fn drawn(c: char) -> bool {
    matches!(c, '\u{2500}'..='\u{259f}')
}

/// A rectangle of the cell, as a polygon. `x` and `y` are the cell's own corner.
fn rect(shape: Shape, x: f32, y: f32, [l, t, r, b]: [f32; 4], color: Color) -> Shape {
    shape.fill(vec![[x + l, y + t], [x + r, y + t], [x + r, y + b], [x + l, y + b]], color)
}

/// What `c` (one for which `drawn` is true) adds to `shape` in the cell at column `col`, in `color`.
pub fn draw(shape: Shape, c: char, col: u16, color: Color) -> Shape {
    let (x, y) = (col as f32 * CELL_W, 0.0);
    match c as u32 {
        0x2500..=0x257f => lines(shape, c, x, y, color),
        n => blocks(shape, n, x, y, color),
    }
}

fn lines(shape: Shape, c: char, x: f32, y: f32, color: Color) -> Shape {
    let n = c as u32;
    if matches!(n, 0x256d..=0x2573) {
        return special(shape, n, x, y, color);
    }
    let [l, r, u, d] = ARMS[(n - 0x2500) as usize];
    let (cx, cy) = (x + CELL_W / 2.0, y + CELL_H / 2.0);
    let width = |w: u8| if w == 2 { HEAVY } else { LIGHT };
    let mut shape = shape;
    // An arm runs from the cell's edge to a little past the middle, so the corner is filled and nothing shows a gap.
    let reach = |a: u8, b: u8| (if a == 3 || b == 3 { GAP } else { 0.0 }).max(width(a).max(width(b)) / 2.0);
    let (hw, vw) = (reach(u, d), reach(l, r));
    let flat = |shape: Shape, w: u8, y: f32, x0: f32, x1: f32| shape.line([x0, y], [x1, y], width(w), LineCap::Butt, color);
    // Horizontal arms.
    for (arm, dir) in [(l, -1.0f32), (r, 1.0)] {
        let edge = if dir < 0.0 { x } else { x + CELL_W };
        match arm {
            0 => {}
            3 => {
                for side in [-1.0f32, 1.0] {
                    // The line on the side a vertical arm leaves by stops short of it; the other runs through.
                    let perp = if side < 0.0 { u } else { d };
                    let end = if perp != 0 { cx + dir * GAP } else { cx - dir * GAP };
                    shape = flat(shape, 1, cy + side * GAP, edge, end);
                }
            }
            w => shape = flat(shape, w, cy, edge, cx - dir * hw),
        }
    }
    // Vertical arms.
    let tall = |shape: Shape, w: u8, x: f32, y0: f32, y1: f32| shape.line([x, y0], [x, y1], width(w), LineCap::Butt, color);
    for (arm, dir) in [(u, -1.0f32), (d, 1.0)] {
        let edge = if dir < 0.0 { y } else { y + CELL_H };
        match arm {
            0 => {}
            3 => {
                for side in [-1.0f32, 1.0] {
                    let perp = if side < 0.0 { l } else { r };
                    let end = if perp != 0 { cy + dir * GAP } else { cy - dir * GAP };
                    shape = tall(shape, 1, cx + side * GAP, edge, end);
                }
            }
            w => shape = tall(shape, w, cx, edge, cy - dir * vw),
        }
    }
    shape
}

/// Rounded corners and the diagonals.
fn special(shape: Shape, n: u32, x: f32, y: f32, color: Color) -> Shape {
    let (cx, cy) = (x + CELL_W / 2.0, y + CELL_H / 2.0);
    let (right, bottom) = (x + CELL_W, y + CELL_H);
    const R: f32 = 4.0;
    // Out along the row to `sx` (-1 left, 1 right), round the corner, and along the column to `sy` (-1 up, 1 down).
    let corner = |sx: f32, sy: f32| {
        let (h, v) = (if sx < 0.0 { x } else { right }, if sy < 0.0 { y } else { bottom });
        let path = Path::new().move_to([h, cy]).line_to([cx + sx * R, cy]).quad_to([cx, cy], [cx, cy + sy * R]).line_to([cx, v]);
        shape.clone().stroke_path(&path, LIGHT, LineCap::Butt, color)
    };
    match n {
        0x256d => corner(1.0, 1.0),
        0x256e => corner(-1.0, 1.0),
        0x256f => corner(-1.0, -1.0),
        0x2570 => corner(1.0, -1.0),
        0x2571 => shape.line([x, bottom], [right, y], LIGHT, LineCap::Butt, color),
        0x2572 => shape.line([x, y], [right, bottom], LIGHT, LineCap::Butt, color),
        _ => shape.line([x, bottom], [right, y], LIGHT, LineCap::Butt, color).line([x, y], [right, bottom], LIGHT, LineCap::Butt, color),
    }
}

/// U+2580..=U+259F: halves, eighths, shades and quadrants, edges rounded to whole pixels so neighbours meet.
fn blocks(shape: Shape, n: u32, x: f32, y: f32, color: Color) -> Shape {
    let (w, h) = (CELL_W, CELL_H);
    let eighth_h = |k: f32| (h * k / 8.0).round();
    let eighth_w = |k: f32| (w * k / 8.0).round();
    let (mw, mh) = ((w / 2.0).ceil(), (h / 2.0).round());
    let r = |shape: Shape, a: [f32; 4]| rect(shape, x, y, a, color);
    match n {
        0x2580 => r(shape, [0.0, 0.0, w, mh]),
        0x2581..=0x2588 => r(shape, [0.0, h - eighth_h((n - 0x2580) as f32), w, h]),
        0x2589..=0x258f => r(shape, [0.0, 0.0, eighth_w((0x2590 - n) as f32), h]),
        0x2590 => r(shape, [mw, 0.0, w, h]),
        0x2591..=0x2593 => rect(shape, x, y, [0.0, 0.0, w, h], color.alpha((n - 0x2590) as f32 * 0.25)),
        0x2594 => r(shape, [0.0, 0.0, w, eighth_h(1.0)]),
        0x2595 => r(shape, [w - eighth_w(1.0), 0.0, w, h]),
        _ => {
            // Quadrants, as upper-left, upper-right, lower-left, lower-right bits.
            let q = match n {
                0x2596 => 0b0010,
                0x2597 => 0b0001,
                0x2598 => 0b1000,
                0x2599 => 0b1011,
                0x259a => 0b1001,
                0x259b => 0b1110,
                0x259c => 0b1101,
                0x259d => 0b0100,
                0x259e => 0b0110,
                _ => 0b0111,
            };
            let mut shape = shape;
            for (bit, a) in [(8, [0.0, 0.0, mw, mh]), (4, [mw, 0.0, w, mh]), (2, [0.0, mh, mw, h]), (1, [mw, mh, w, h])] {
                if q & bit != 0 {
                    shape = r(shape, a);
                }
            }
            shape
        }
    }
}

/// The underline styles the font can't draw, and the colour of any underline that isn't the text's (SGR 58).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Under {
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

/// The row's baseline-ish place for underlines: just under the text, inside the 19 px cell.
const UNDER_Y: f32 = 16.5;
const STRIKE_Y: f32 = 9.5;

/// An underline of `style` over columns `from..to` (cell edges).
pub fn underline(shape: Shape, style: Under, from: u16, to: u16, color: Color) -> Shape {
    let (x0, x1) = (from as f32 * CELL_W, to as f32 * CELL_W);
    match style {
        Under::Single => shape.line([x0, UNDER_Y], [x1, UNDER_Y], LIGHT, LineCap::Butt, color),
        Under::Double => shape.line([x0, UNDER_Y - 1.0], [x1, UNDER_Y - 1.0], LIGHT, LineCap::Butt, color).line(
            [x0, UNDER_Y + 1.0],
            [x1, UNDER_Y + 1.0],
            LIGHT,
            LineCap::Butt,
            color,
        ),
        Under::Curly => {
            // One wave a cell, up and down by a pixel.
            let mut points = Vec::new();
            let mut x = x0;
            while x < x1 {
                points.push([x, UNDER_Y + 1.0]);
                points.push([(x + CELL_W / 2.0).min(x1), UNDER_Y - 1.0]);
                x += CELL_W;
            }
            points.push([x1, UNDER_Y + 1.0]);
            shape.stroke_with(points, false, LIGHT, LineCap::Butt, color)
        }
        Under::Dotted => dashes(shape, x0, x1, 1.0, 1.0, color),
        Under::Dashed => dashes(shape, x0, x1, 3.0, 2.0, color),
    }
}

fn dashes(mut shape: Shape, x0: f32, x1: f32, on: f32, off: f32, color: Color) -> Shape {
    let mut x = x0;
    while x < x1 {
        shape = shape.line([x, UNDER_Y], [(x + on).min(x1), UNDER_Y], LIGHT, LineCap::Butt, color);
        x += on + off;
    }
    shape
}

/// A line through the middle of the cells `from..to`.
pub fn strike(shape: Shape, from: u16, to: u16, color: Color) -> Shape {
    shape.line([from as f32 * CELL_W, STRIKE_Y], [to as f32 * CELL_W, STRIKE_Y], LIGHT, LineCap::Butt, color)
}
