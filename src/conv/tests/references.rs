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
