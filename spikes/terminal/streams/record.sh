#!/usr/bin/env bash
# Records the byte streams the spike's scenes replay, from an invented project in a scratch
# folder, at 100 x 30. Needs util-linux `script`, git, vim and less.
#
#   bash streams/record.sh /some/empty/scratch/dir
#
# Nothing in the output may come from a real machine: files, names, dates and authors are
# made up here, `ls` leaves out owner and group, and no program that lists the machine's own
# processes (htop, top) is recorded.
set -euo pipefail
out="$(cd "$(dirname "$0")" && pwd)"
work="${1:?scratch dir}"
mkdir -p "$work/lantern" && cd "$work/lantern"
export HOME="$work" LANG=C.UTF-8 TERM=xterm-256color COLORTERM=truecolor
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null

mkdir -p src/lamp docs scripts assets
cat > src/lamp/wick.rs <<'EOF'
//! How long a wick burns, from its length and the oil left.
use std::time::Duration;

/// A wick of `length_mm`, soaking in `oil_ml` of oil.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wick {
    pub length_mm: f32,
    pub oil_ml: f32,
}

impl Wick {
    pub const BURN_ML_PER_HOUR: f32 = 4.5;

    /// Burn time left; a wick shorter than 2 mm won't stay lit.
    pub fn burn_time(&self) -> Option<Duration> {
        if self.length_mm < 2.0 || self.oil_ml <= 0.0 {
            return None;
        }
        let hours = self.oil_ml / Self::BURN_ML_PER_HOUR;
        Some(Duration::from_secs_f32(hours * 3600.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_wick_goes_out() {
        let w = Wick { length_mm: 1.5, oil_ml: 90.0 };
        assert_eq!(w.burn_time(), None);
    }
}
EOF
printf '# Lantern\n\nA small lamp controller.\n' > README.md
printf 'fn main() {}\n' > src/main.rs
printf 'pub mod wick;\n' > src/lamp/mod.rs
printf '#!/bin/sh\necho trimming\n' > scripts/trim.sh && chmod +x scripts/trim.sh
printf 'notes\n' > docs/notes.md
head -c 2048 /dev/zero > assets/glow.png
ln -sf README.md LATEST
for f in $(find . -not -path './.git*'); do touch -h -d '2026-03-14 09:26' "$f"; done

git init -q -b main .
c() { GIT_AUTHOR_DATE="$1" GIT_COMMITTER_DATE="$1" git -c user.name="$2" -c user.email="$3@example.com" commit -q --allow-empty -m "$4"; }
git add -A && c '2026-03-01T10:00:00' 'Ada Fenwick' ada 'Start the lantern controller'
c '2026-03-02T11:20:00' 'Ada Fenwick' ada 'Measure oil left in millilitres'
git checkout -q -b wick-length
c '2026-03-03T09:05:00' 'Juno Marsh' juno 'A wick shorter than 2 mm goes out'
c '2026-03-04T16:40:00' 'Juno Marsh' juno 'Test the short-wick case'
git checkout -q main
c '2026-03-05T08:15:00' 'Ada Fenwick' ada 'Trim script prints what it trims'
git merge -q --no-ff -m "Merge branch 'wick-length'" wick-length --no-edit 2>/dev/null || true
GIT_COMMITTER_DATE='2026-03-06T12:00:00' GIT_AUTHOR_DATE='2026-03-06T12:00:00' \
  git -c user.name='Ada Fenwick' -c user.email=ada@example.com commit -q --allow-empty -m 'Release 0.2' && git tag v0.2

rec() { # name, command: run at 100 x 30 under a pty, keep the raw bytes
  # A killed full-screen program can leave `script` waiting, so it gets a deadline too.
  timeout 8 script -q -e -c "stty cols 100 rows 30; $2" "$out/$1.raw" > /dev/null < /dev/null || true
  # util-linux writes a "Script started/done" line at each end; keep only what the program printed.
  python3 -I - "$out/$1.raw" "$out/$1.bin" <<'PY'
import sys
raw = open(sys.argv[1], 'rb').read()
if raw.startswith(b'Script started'):
    raw = raw.split(b'\n', 1)[1]
cut = raw.rfind(b'\nScript done')
if cut >= 0:
    raw = raw[:cut]
# A full-screen program's last screen, before it hands the normal screen back.
leave = raw.rfind(b'\x1b[?1049l')
if leave >= 0:
    raw = raw[:leave]
open(sys.argv[2], 'wb').write(raw)
PY
  rm -f "$out/$1.raw"
}

rec ls "ls --color=always -l -g -o -h --time-style=long-iso; ls --color=always -F src/lamp scripts assets"
rec git-log "git --no-pager log --graph --color=always --all --date=short --format='%C(yellow)%h%C(reset)%C(auto)%d%C(reset) %s %C(dim)(%an, %ad)%C(reset)'"
rec vim "timeout -s KILL 3 vim -u NONE -i NONE -N -c 'syntax on' -c 'set number laststatus=2 termguicolors' -c 'colorscheme habamax' -c 'vsplit README.md' -c 'wincmd l' -c '12' src/lamp/wick.rs"
rec less "timeout -s KILL 2 less -R -N +8 src/lamp/wick.rs"
# Wide and combining characters, emoji, 256 colours, true colour, attributes and box drawing.
rec unicode "printf '%b\n' \
  'Wide:  日本語のテキスト | 中文字符 | 한국어 텍스트 | ｆｕｌｌｗｉｄｔｈ' \
  'Emoji: 🦀 crab  🚀 rocket  👩‍💻 coder  ✅ done  🏳️‍🌈 flag  ⚠️ warn' \
  'Marks: é ä ñ  (combining)   Arrows: ← ↑ → ↓ ⇄  Math: ∑ ∫ √ ≠ ≤' \
  '\e[1mbold\e[0m \e[2mdim\e[0m \e[3mitalic\e[0m \e[4munderline\e[0m \e[9mstrike\e[0m \e[7minverse\e[0m \e[1;3mbold italic\e[0m \e[4:3mcurly\e[0m'; \
  for i in \$(seq 0 15); do printf '\e[48;5;%sm  ' \$i; done; printf '\e[0m\n'; \
  for i in \$(seq 16 231); do printf '\e[48;5;%sm ' \$i; [ \$(( (i-15) % 72 )) = 0 ] && printf '\e[0m\n'; done; printf '\e[0m\n'; \
  for i in \$(seq 0 99); do printf '\e[48;2;%d;%d;%dm ' \$((i*255/99)) \$((128-i)) \$((255-i*255/99)); done; printf '\e[0m\n'; \
  printf '┌──────────┬──────────┐\n│ \e[32mpassing\e[0m  │ \e[31mfailing\e[0m  │\n├──────────┼──────────┤\n│ 42       │ 0        │\n└──────────┴──────────┘\n'; \
  printf '▁▂▃▄▅▆▇█ ░▒▓ ⣿⡇ \e[38;2;255;140;0mtrue colour orange\e[0m \e[38;5;45mpalette 45\e[0m\n'"
ls -la "$out"/*.bin
