//! Counts, token counts, sizes and money as Fleet writes them (`lib/format/numbers.ts`): "12,500", "17.1k",
//! "3.4 MB", "$1.85". Fleet formats in the user's locale; the native app has no locale setting yet, so these
//! write what `en-US` writes (component 24, Settings, owns a locale if it gets one). A number that isn't a
//! number (NaN, infinity) is an empty string, as in Fleet.

/// Round the decimal text of a non-negative number to `digits` places, halves up, the way `toFixed` and
/// `Intl.NumberFormat` do (Rust's own `{:.1}` rounds an exact half to even: 1.25 would be "1.2", not "1.3").
/// `text` has no exponent and no sign. Returns the integer part and the fraction, exactly `digits` long.
fn round_half_up(text: &str, digits: usize) -> (String, String) {
    let (int, frac) = text.split_once('.').unwrap_or((text, ""));
    let mut frac_digits: Vec<u8> = frac.bytes().map(|b| b - b'0').collect();
    let up = frac_digits.get(digits).is_some_and(|d| *d >= 5);
    frac_digits.resize(digits, 0);
    let mut all: Vec<u8> = int.bytes().map(|b| b - b'0').chain(frac_digits).collect();
    if up {
        let mut i = all.len();
        loop {
            if i == 0 {
                all.insert(0, 1);
                break;
            }
            i -= 1;
            if all[i] == 9 {
                all[i] = 0;
            } else {
                all[i] += 1;
                break;
            }
        }
    }
    let split = all.len() - digits;
    let text = |part: &[u8]| part.iter().map(|d| (d + b'0') as char).collect::<String>();
    (text(&all[..split]), text(&all[split..]))
}

fn group_thousands(int: &str) -> String {
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `Intl.NumberFormat` with the given fraction digits, for a non-negative number.
fn grouped(x: f64, min_frac: usize, max_frac: usize) -> String {
    let (int, mut frac) = round_half_up(&format!("{x}"), max_frac);
    while frac.len() > min_frac && frac.ends_with('0') {
        frac.pop();
    }
    let int = group_thousands(&int);
    if frac.is_empty() { int } else { format!("{int}.{frac}") }
}

/// JavaScript's `Number(value.toFixed(digits))` for a non-negative number, as a number again.
fn to_fixed(x: f64, digits: usize) -> f64 {
    let (int, frac) = round_half_up(&format!("{x:.40}"), digits);
    format!("{int}.{frac}").parse().unwrap_or(x)
}

fn finite(n: Option<f64>) -> Option<f64> {
    n.filter(|n| n.is_finite())
}

/// `formatCount`: a plain count with grouping, "12,500"; rounded to a whole number.
pub fn format_count(n: Option<f64>) -> String {
    format_count_digits(n, 0)
}

/// `formatCount` with `fractionDigits`: up to this many decimals, "1.25".
pub fn format_count_digits(n: Option<f64>, fraction_digits: usize) -> String {
    let Some(n) = finite(n) else { return String::new() };
    let sign = if n < 0.0 && grouped(-n, 0, fraction_digits).chars().any(|c| c != '0' && c.is_ascii_digit()) { "-" } else { "" };
    format!("{sign}{}", grouped(n.abs(), 0, fraction_digits))
}

/// `formatTokens`: a token count, short: "950", "17.1k", "200k", "1.2M", "3.4B". One decimal under 100 of a unit,
/// none from 100; a count that would round up to the next unit moves to it ("1M", never "1,000k").
pub fn format_tokens(n: Option<f64>) -> String {
    let Some(n) = finite(n) else { return String::new() };
    let sign = if n < 0.0 { "-" } else { "" };
    let abs = n.abs();
    if abs.round() < 1000.0 {
        return format!("{sign}{}", grouped(abs, 0, 0));
    }
    for (size, suffix) in [(1e3, "k"), (1e6, "M"), (1e9, "B")] {
        let value = abs / size;
        let digits = if value < 99.95 { 1 } else { 0 };
        let rounded = to_fixed(value, digits);
        if rounded < 1000.0 || suffix == "B" {
            return format!("{sign}{}{suffix}", grouped(rounded, 0, digits));
        }
    }
    String::new()
}

/// `formatMoney` in US dollars: "$1.85", "$1,234.50"; "<$0.01" for a positive amount that rounds to nothing.
pub fn format_money(amount: Option<f64>) -> String {
    let Some(amount) = finite(amount) else { return String::new() };
    if amount > 0.0 && amount < 0.005 {
        return "<$0.01".into();
    }
    let text = grouped(amount.abs(), 2, 2);
    let negative = amount < 0.0 && text.chars().any(|c| c != '0' && c.is_ascii_digit());
    format!("{}${text}", if negative { "-" } else { "" })
}

/// `formatBytes`: "812 B", "34 KB", "3.4 MB", "340 MB", "2.1 GB". One decimal under 10 from a megabyte up.
pub fn format_bytes(bytes: Option<f64>) -> String {
    let Some(bytes) = finite(bytes) else { return String::new() };
    let abs = bytes.max(0.0);
    if abs < 1024.0 {
        return format!("{} B", grouped(abs, 0, 0));
    }
    if abs < 1024.0 * 1024.0 {
        return format!("{} KB", grouped(abs / 1024.0, 0, 0));
    }
    let (value, unit) = if abs < 1024.0f64.powi(3) { (abs / 1024.0f64.powi(2), "MB") } else { (abs / 1024.0f64.powi(3), "GB") };
    let digits = if value < 10.0 { 1 } else { 0 };
    format!("{} {unit}", grouped(value, digits, digits))
}
