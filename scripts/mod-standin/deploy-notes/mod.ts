import type { Register } from "fleet-mods";

// Invented for the fleet-native port's pictures: one pane that shows most of the elements a mod can draw.
export const register: Register = (on) => {
  on("ui.render", { component: "ToolUse", props: { tool: ["bash", "shell"] } }, async ($, e, next) => {
    if (e.component !== "ToolUse") return next(e);
    const { Box, Pill, Icon } = $.ui.resolve(e);
    return Box({ gap: 1, alignItems: "center", children: [
      Pill({ tone: "good", label: "42 passed", icon: "check" }),
      Pill({ tone: "neutral", label: "0 skipped" }),
      Icon({ name: "zap", color: "accent", label: "Fast" }),
    ] });
  });
  on("ui.render", { component: "Pane" }, async ($, e, next) => {
    if (e.component !== "Pane" || e.requestId !== "notes") return next(e);
    const { Box, Text, Pill, Icon, Button, Input, Select, Markdown, Code } = $.ui.resolve(e);
    const target = $.state.get<string>("target") ?? "staging";
    return Box({
      flexDirection: "column",
      gap: 3,
      children: [
        Box({ flexDirection: "row", gap: 2, alignItems: "center", children: [
          Icon({ name: "gauge", color: "accent" }),
          Text({ bold: true, children: ["Deploy 1.8 to ", Text({ code: true, children: [target] })] }),
        ] }),
        Box({ flexDirection: "row", gap: 2, flexWrap: "wrap", children: [
          Pill({ tone: "good", label: "Build passed", icon: "check" }),
          Pill({ tone: "warn", label: "2 checks waiting", icon: "clock" }),
          Pill({ tone: "bad", label: "1 failed", icon: "x" }),
          Pill({ tone: "neutral", label: "Draft" }),
        ] }),
        Box({ borderStyle: "quote", borderColor: "accent", paddingX: 3, children: [
          Markdown({ text: "Ship after the **smoke test**. See [the runbook](https://example.com/runbook)." }),
        ] }),
        Select({ key: "target", label: "Target", value: target, options: [
          { value: "staging", label: "Staging" }, { value: "production", label: "Production" },
        ], onSelect: (v) => $.state.set("target", v) }),
        Input({ key: "note", label: "Note for the team", placeholder: "What changed?", submitLabel: "Add", onSubmit: () => {} }),
        Code({ source: "@@ -12,3 +12,3 @@\n-  timeout: 30\n+  timeout: 60\n   retries: 3", format: "diff", path: "deploy.yaml", startLine: 12 }),
        Box({ flexDirection: "row", gap: 2, children: [
          Button({ key: "go", label: "Mark ready", tone: "primary", icon: "check", onPress: () => {} }),
          Button({ key: "stop", label: "Hold", tone: "danger", onPress: () => {} }),
          Button({ key: "skip", label: "Skip", onPress: () => {} }),
        ] }),
      ],
    });
  });
};
