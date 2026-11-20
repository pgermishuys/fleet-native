//! The invented TypeScript file and folder listing behind the completion scenes (component 11, piece U): `src/ledger/index.ts` of the
//! scratch Fleet's demo project, importing from its own folder, so the native popup and Fleet's own show the same suggestions.
use crate::composer::files::narrow_file_matches;

pub const TS_PATH: &str = "src/ledger/index.ts";
pub const TS_SOURCE: &str = "import { rows } from \"./format\";\nimport { totals } from \"./t\";\n\nexport function pageOf(ledger: Ledger, page: number) {\n  return rows(ledger).slice((page - 1) * 25, page * 25);\n}\n";

/// The files the project has besides the ones its Files tab lists (so those pictures don't change).
const EXTRA: [&str; 5] = ["src/ledger/format.ts", "src/ledger/index.ts", "src/ledger/totals.ts", "src/ledger/types.d.ts", "src/ledger/ui/"];

/// What the find endpoint answers for `folder` (the path ending in "/"): its files and folders, as the server lists them.
pub fn folder(folder: &str) -> Vec<String> {
    let mut all: Vec<String> = super::files_tab::all_files().into_iter().map(str::to_string).collect();
    all.extend(EXTRA.iter().map(|s| s.to_string()));
    narrow_file_matches(&all, folder)
}
