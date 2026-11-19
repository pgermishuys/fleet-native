//! `lib/__tests__/file-references.test.ts` (`parseFileReference`'s two tables; the Markdown ones need Rune's inline
//! code to be markable, see the checklist) and `formatSlashCommand` of `lib/slash-command-utils.ts`.
use crate::conv::references::*;

fn file(path: &str, line: Option<u32>) -> Option<FileReference> {
    Some(FileReference { path: path.into(), line })
}

#[test]
fn reads_text_as_a_file() {
    for (text, expected) in [
        ("src/billing/tax.ts", file("src/billing/tax.ts", None)),
        ("src/billing/tax.ts:13", file("src/billing/tax.ts", Some(13))),
        ("src/billing/tax.ts:13:7", file("src/billing/tax.ts", Some(13))),
        ("src/billing/tax.ts:13-20", file("src/billing/tax.ts", Some(13))),
        ("src/billing/tax.ts#L18", file("src/billing/tax.ts", Some(18))),
        ("src/billing/tax.ts#L18-L24", file("src/billing/tax.ts", Some(18))),
        ("test/billing/invoice-service.test.ts", file("test/billing/invoice-service.test.ts", None)),
        ("README.md", file("README.md", None)),
        (".gitignore", file(".gitignore", None)),
        ("./src/main.rs", file("./src/main.rs", None)),
        ("docker/Dockerfile", file("docker/Dockerfile", None)),
        ("src\\Billing\\Tax.cs:28", file("src\\Billing\\Tax.cs", Some(28))),
        ("C:\\dev\\harbor-api\\src\\billing\\tax.ts", file("C:\\dev\\harbor-api\\src\\billing\\tax.ts", None)),
        ("/home/sam/dev/harbor-api/src/billing/tax.ts:17", file("/home/sam/dev/harbor-api/src/billing/tax.ts", Some(17))),
    ] {
        assert_eq!(parse_file_reference(text), expected, "{text}");
    }
}

#[test]
fn leaves_other_text_alone() {
    for text in [
        "lineTotal",
        "this.lineTotal",
        "HttpContext.Items",
        "1.2.3",
        "roundCents(net)",
        "a + b",
        "https://example.com/a.ts",
        "--force",
        "Dockerfile",
        "..",
        "x = 1",
    ] {
        assert_eq!(parse_file_reference(text), None, "{text}");
    }
}

#[test]
fn formats_a_slash_command_as_typed() {
    assert_eq!(format_slash_command("tidy", Some("src/auth")), "/tidy src/auth");
    assert_eq!(format_slash_command("init", None), "/init");
    assert_eq!(format_slash_command("init", Some("")), "/init");
}

// "file references in rendered Markdown" of `file-references.test.ts`: what a message names, before the server says which are files.
#[test]
fn collects_the_files_inline_code_names_with_their_lines() {
    assert_eq!(
        markdown_file_paths("See `/home/sam/dev/harbor-api/src/billing/invoice-service.ts:17`."),
        ["/home/sam/dev/harbor-api/src/billing/invoice-service.ts"]
    );
}

#[test]
fn collects_a_markdown_link_to_a_file() {
    assert_eq!(markdown_file_paths("The rule is in [tax.ts](src/billing/tax.ts#L18)."), ["src/billing/tax.ts"]);
}

#[test]
fn collects_every_path_named_in_order_and_skips_what_is_not_a_path() {
    assert_eq!(
        markdown_file_paths("`src/billing/tax.ts:13` and `src/billing/legacy-rates.ts`, not `lineTotal`, and `src/billing/tax.ts` again."),
        ["src/billing/tax.ts", "src/billing/legacy-rates.ts"]
    );
}

#[test]
fn leaves_paths_in_code_blocks_and_web_links_alone() {
    assert!(markdown_file_paths("```\nsrc/billing/tax.ts\n```\n\n[docs](https://example.com/tax.ts)").is_empty());
    assert!(markdown_file_paths("~~~ts\n`src/billing/tax.ts`\n~~~\n").is_empty());
    assert_eq!(markdown_file_paths("```\nx\n```\nafter `a/b.ts`"), ["a/b.ts"]);
}

#[test]
fn double_backticks_hold_a_single_one_and_an_unclosed_run_names_nothing() {
    assert_eq!(markdown_file_paths("``src/a.ts``"), ["src/a.ts"]);
    assert!(markdown_file_paths("a ` b src/a.ts").is_empty());
}
