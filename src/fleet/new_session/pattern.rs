//! The small slice of JavaScript regular expressions that a worktree naming `capture` pattern uses
//! (`[A-Z]{2,}-\d+`), so the plan line's preview can run Fleet's own patterns without a `regex` crate
//! (a new crate costs build time on three systems, every time; `AGENTS.md`).
//!
//! Supported: literals, `.`, classes (`[a-z]`, `[^x]`, `\d \w \s` inside), `\d \D \w \W \s \S \b \B`, escaped
//! punctuation, groups (`( )`, `(?: )`), alternation, `* + ? {n} {n,} {n,m}` (greedy, or lazy with a trailing `?`),
//! `^` and `$`. Anything else (look-around, back-references, `\p{..}`) makes the pattern unusable, which is what Fleet's
//! preview does with a pattern that throws: naming carries on without the capture.

#[derive(Debug)]
enum Node {
    Char(char),
    Any,
    Class { negated: bool, items: Vec<ClassItem> },
    Group { index: Option<usize>, alternatives: Vec<Vec<Node>> },
    Repeat { node: Box<Node>, min: usize, max: usize, greedy: bool },
    Start,
    End,
    WordBoundary(bool),
}

#[derive(Debug)]
enum ClassItem {
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}

/// A compiled pattern.
#[derive(Debug)]
pub struct Pattern {
    alternatives: Vec<Vec<Node>>,
    groups: usize,
}

struct Parser {
    chars: Vec<char>,
    at: usize,
    groups: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn alternatives(&mut self) -> Option<Vec<Vec<Node>>> {
        let mut all = vec![self.sequence()?];
        while self.eat('|') {
            all.push(self.sequence()?);
        }
        Some(all)
    }

    fn sequence(&mut self) -> Option<Vec<Node>> {
        let mut nodes = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            nodes.push(self.quantified(atom)?);
        }
        Some(nodes)
    }

    fn quantified(&mut self, atom: Node) -> Option<Node> {
        let (min, max) = match self.peek() {
            Some('*') => (0, usize::MAX),
            Some('+') => (1, usize::MAX),
            Some('?') => (0, 1),
            // `{` with no count is a literal brace in JavaScript's sloppy mode; keep it simple and refuse.
            Some('{') => {
                let bounds = self.braces()?;
                return self.lazy(atom, bounds);
            }
            _ => return Some(atom),
        };
        self.at += 1;
        self.lazy(atom, (min, max))
    }

    fn lazy(&mut self, atom: Node, (min, max): (usize, usize)) -> Option<Node> {
        if matches!(atom, Node::Start | Node::End | Node::WordBoundary(_)) {
            return None;
        }
        let greedy = !self.eat('?');
        Some(Node::Repeat { node: Box::new(atom), min, max, greedy })
    }

    /// `{n}`, `{n,}` or `{n,m}`; leaves `at` after the `}`.
    fn braces(&mut self) -> Option<(usize, usize)> {
        let close = self.chars[self.at..].iter().position(|&c| c == '}')? + self.at;
        let inner: String = self.chars[self.at + 1..close].iter().collect();
        let (low, high) = match inner.split_once(',') {
            Some((low, high)) => (low, Some(high)),
            None => (inner.as_str(), None),
        };
        let min: usize = low.parse().ok()?;
        let max = match high {
            None => min,
            Some("") => usize::MAX,
            Some(h) => h.parse().ok()?,
        };
        if max < min {
            return None;
        }
        self.at = close + 1;
        Some((min, max))
    }

    fn atom(&mut self) -> Option<Node> {
        let c = self.peek()?;
        self.at += 1;
        match c {
            '.' => Some(Node::Any),
            '^' => Some(Node::Start),
            '$' => Some(Node::End),
            '(' => {
                let index = if self.eat('?') {
                    if !self.eat(':') {
                        return None;
                    }
                    None
                } else {
                    self.groups += 1;
                    Some(self.groups)
                };
                let alternatives = self.alternatives()?;
                if !self.eat(')') {
                    return None;
                }
                Some(Node::Group { index, alternatives })
            }
            '[' => self.class(),
            '\\' => self.escape(),
            '*' | '+' | '?' | '{' | '}' | ')' | ']' => None,
            c => Some(Node::Char(c)),
        }
    }

    fn escape(&mut self) -> Option<Node> {
        let c = self.peek()?;
        self.at += 1;
        let class = |item| Node::Class { negated: false, items: vec![item] };
        Some(match c {
            'd' => class(ClassItem::Digit(true)),
            'D' => class(ClassItem::Digit(false)),
            'w' => class(ClassItem::Word(true)),
            'W' => class(ClassItem::Word(false)),
            's' => class(ClassItem::Space(true)),
            'S' => class(ClassItem::Space(false)),
            'b' => Node::WordBoundary(true),
            'B' => Node::WordBoundary(false),
            'n' => Node::Char('\n'),
            'r' => Node::Char('\r'),
            't' => Node::Char('\t'),
            c if c.is_ascii_alphanumeric() => return None,
            c => Node::Char(c),
        })
    }

    fn class(&mut self) -> Option<Node> {
        let negated = self.eat('^');
        let mut items = Vec::new();
        loop {
            let c = self.peek()?;
            self.at += 1;
            if c == ']' {
                break;
            }
            let low = if c == '\\' {
                let e = self.peek()?;
                self.at += 1;
                match e {
                    'd' => {
                        items.push(ClassItem::Digit(true));
                        continue;
                    }
                    'D' => {
                        items.push(ClassItem::Digit(false));
                        continue;
                    }
                    'w' => {
                        items.push(ClassItem::Word(true));
                        continue;
                    }
                    'W' => {
                        items.push(ClassItem::Word(false));
                        continue;
                    }
                    's' => {
                        items.push(ClassItem::Space(true));
                        continue;
                    }
                    'S' => {
                        items.push(ClassItem::Space(false));
                        continue;
                    }
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    e if e.is_ascii_alphanumeric() => return None,
                    e => e,
                }
            } else {
                c
            };
            // A range, unless the `-` is the last thing before `]`.
            if self.peek() == Some('-') && self.chars.get(self.at + 1).is_some_and(|&n| n != ']') {
                self.at += 1;
                let mut high = self.peek()?;
                self.at += 1;
                if high == '\\' {
                    high = self.peek()?;
                    self.at += 1;
                    if high.is_ascii_alphanumeric() {
                        return None;
                    }
                }
                if high < low {
                    return None;
                }
                items.push(ClassItem::Range(low, high));
            } else {
                items.push(ClassItem::Range(low, low));
            }
        }
        Some(Node::Class { negated, items })
    }
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

impl ClassItem {
    fn matches(&self, c: char) -> bool {
        match *self {
            ClassItem::Range(low, high) => (low..=high).contains(&c),
            ClassItem::Digit(yes) => c.is_ascii_digit() == yes,
            ClassItem::Word(yes) => is_word(c) == yes,
            ClassItem::Space(yes) => c.is_whitespace() == yes,
        }
    }
}

type Spans = Vec<Option<(usize, usize)>>;

struct Run<'a> {
    text: &'a [char],
    spans: Spans,
    /// A cap on the work one match may do, so a pattern like `(a+)+b` can't freeze the window.
    steps: usize,
}

const STEP_LIMIT: usize = 200_000;

impl Run<'_> {
    /// Matches `nodes` from `at`, then `next`. The continuation is how a later failure backs out of an earlier choice.
    fn seq(&mut self, nodes: &[Node], at: usize, next: &mut dyn FnMut(&mut Run, usize) -> bool) -> bool {
        self.steps += 1;
        if self.steps > STEP_LIMIT {
            return false;
        }
        let Some((first, rest)) = nodes.split_first() else { return next(self, at) };
        match first {
            Node::Char(c) => self.text.get(at) == Some(c) && self.seq(rest, at + 1, next),
            Node::Any => {
                self.text.get(at).is_some_and(|c| !matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')) && self.seq(rest, at + 1, next)
            }
            Node::Class { negated, items } => {
                self.text.get(at).is_some_and(|&c| items.iter().any(|i| i.matches(c)) != *negated) && self.seq(rest, at + 1, next)
            }
            Node::Start => at == 0 && self.seq(rest, at, next),
            Node::End => at == self.text.len() && self.seq(rest, at, next),
            Node::WordBoundary(want) => {
                let before = at > 0 && is_word(self.text[at - 1]);
                let after = self.text.get(at).is_some_and(|&c| is_word(c));
                ((before != after) == *want) && self.seq(rest, at, next)
            }
            Node::Group { index, alternatives } => {
                for alternative in alternatives {
                    let saved = self.spans.clone();
                    let matched = self.seq(alternative, at, &mut |run, end| {
                        let before = index.map(|i| run.spans[i]);
                        if let Some(i) = *index {
                            run.spans[i] = Some((at, end));
                        }
                        if run.seq(rest, end, next) {
                            return true;
                        }
                        if let (Some(i), Some(old)) = (*index, before) {
                            run.spans[i] = old;
                        }
                        false
                    });
                    if matched {
                        return true;
                    }
                    self.spans = saved;
                }
                false
            }
            Node::Repeat { node, min, max, greedy } => self.repeat(node, *min, *max, *greedy, 0, at, rest, next),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn repeat(
        &mut self,
        node: &Node,
        min: usize,
        max: usize,
        greedy: bool,
        count: usize,
        at: usize,
        rest: &[Node],
        next: &mut dyn FnMut(&mut Run, usize) -> bool,
    ) -> bool {
        let one = std::slice::from_ref(node);
        let more = |run: &mut Run, next: &mut dyn FnMut(&mut Run, usize) -> bool| {
            count < max
                && run.seq(one, at, &mut |run, end| {
                    // An empty repeat would loop forever.
                    end != at && run.repeat(node, min, max, greedy, count + 1, end, rest, next)
                })
        };
        let stop = |run: &mut Run, next: &mut dyn FnMut(&mut Run, usize) -> bool| count >= min && run.seq(rest, at, next);
        // Greedy tries one more first; lazy tries to stop first.
        if greedy {
            if more(self, next) {
                return true;
            }
            stop(self, next)
        } else {
            if stop(self, next) {
                return true;
            }
            more(self, next)
        }
    }
}

impl Pattern {
    /// `None` for a pattern outside the supported slice.
    pub fn new(source: &str) -> Option<Pattern> {
        let mut parser = Parser { chars: source.chars().collect(), at: 0, groups: 0 };
        let alternatives = parser.alternatives()?;
        (parser.at == parser.chars.len()).then_some(Pattern { alternatives, groups: parser.groups })
    }

    /// `RegExp.exec(text)` read the way naming reads it: the first group's text when the pattern has one that took
    /// part, else the whole match. `None` when nothing matches.
    pub fn capture(&self, text: &str) -> Option<String> {
        let chars: Vec<char> = text.chars().collect();
        for start in 0..=chars.len() {
            let mut run = Run { text: &chars, spans: vec![None; self.groups + 1], steps: 0 };
            let mut end_at = None;
            for alternative in &self.alternatives {
                if run.seq(alternative, start, &mut |_, end| {
                    end_at = Some(end);
                    true
                }) {
                    break;
                }
            }
            if let Some(end) = end_at {
                let (from, to) = if self.groups > 0 { run.spans[1].unwrap_or((start, end)) } else { (start, end) };
                return Some(chars[from..to].iter().collect());
            }
        }
        None
    }
}
